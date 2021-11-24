use crate::protocol::Packet;
use async_trait::async_trait;
use std::io::{Cursor, Read, Write};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub struct Varint21 {
	pub ans: i32,
	pub size: u8,
}
impl Varint21 {
	
	#[allow(dead_code)]
	fn read(mut read: impl Read) -> io::Result<Self> {
		let mut buf = [0];
		let mut ans = 0;
		let mut size = 0;
		for i in 0..=4 {
			read.read_exact(&mut buf)?;
			size += 1;
			ans |= ((buf[0] & 0b0111_1111) as i32) << (7 * i);
			if buf[0] & 0b1000_0000 == 0 {
				break;
			}
		}
		Ok(Self { ans, size })
	}
}

fn ensure_capacity(vec: &mut Vec<u8>, capacity: usize) {
	if vec.capacity() < capacity {
		vec.reserve(capacity - vec.capacity());
		// Safety: По факту нам без разницы, каким мусором там забито,
		// нам нужен лишь кусок этого буфера, и он гарантированно будет заполнен
		unsafe { vec.set_len(vec.capacity()) };
	}
}

pub struct MinecraftPacket<'t> {
	pub packet_id: i32,
	pub data: &'t [u8],
}

impl<'t> MinecraftPacket<'t> {
	pub fn decode<T: Packet>(&mut self) -> io::Result<T> {
		T::read(&mut self.data)
	}

	#[allow(dead_code)]
	pub async fn write<W: AsyncWrite + Unpin + Send>(
		self,
		buf: &mut W,
	) -> io::Result<()> {
		let id_len = varint_size(self.packet_id);
		buf.write_varint(self.data.len() as i32 + id_len as i32)
			.await?;

		buf.write_varint(self.packet_id as i32).await?;
		buf.write_all(self.data).await?;

		Ok(())
	}
}

#[async_trait]
pub trait MinecraftAsyncReadExt: AsyncRead + Unpin + Send {
	async fn read_packet<'t>(
		&mut self,
		buf: &'t mut Vec<u8>,
	) -> io::Result<MinecraftPacket<'t>> {
		let packet_length = self.read_varint().await?.ans;
		assert!(packet_length >= 1);
		let Varint21 {
			ans: packet_id,
			size: packet_id_length,
		} = self.read_varint().await?;
		assert!(packet_id >= 0);
		let packet_length = packet_length - packet_id_length as i32;
		assert!(packet_length >= 0);

		ensure_capacity(buf, packet_length as usize);
		let buf = &mut buf[0..packet_length as usize];
		self.read_exact(buf).await?;
		let buf = &*buf;

		Ok(MinecraftPacket {
			packet_id,
			data: buf,
		})
	}
	async fn read_varint(&mut self) -> io::Result<Varint21> {
		let mut buf = [0];
		let mut ans = 0;
		let mut size = 0;
		for i in 0..=4 {
			self.read_exact(&mut buf).await?;
			size += 1;
			ans |= ((buf[0] & 0b0111_1111) as i32) << (7 * i);
			if buf[0] & 0b1000_0000 == 0 {
				break;
			}
		}
		Ok(Varint21 { ans, size })
	}

	async fn read_bytes(&mut self, limit: i32) -> io::Result<Vec<u8>> {
		let length = self.read_varint().await?.ans;
		if length > limit {
			panic!("Length exceeds limit"); // TODO don't panic
		}
		let mut buf = vec![0; length as usize];
		self.read_exact(&mut buf).await?;
		Ok(buf)
	}

	async fn read_string(&mut self, limit: i32) -> io::Result<String> {
		let bytes = self.read_bytes(limit).await?;
		Ok(String::from_utf8_lossy(&bytes).to_string())
	}
}
impl<T> MinecraftAsyncReadExt for T where T: AsyncRead + Unpin + Send {}

pub trait MinecraftReadExt: Read {
	fn read_varint(&mut self) -> io::Result<Varint21> {
		let mut buf = [0];
		let mut ans = 0;
		let mut size = 0;
		for i in 0..=4 {
			self.read_exact(&mut buf)?;
			size += 1;
			ans |= ((buf[0] & 0b0111_1111) as i32) << (7 * i);
			if buf[0] & 0b1000_0000 == 0 {
				break;
			}
		}
		Ok(Varint21 { ans, size })
	}
	fn read_bytes(&mut self, limit: i32) -> io::Result<Vec<u8>> {
		let length = self.read_varint().unwrap().ans;
		if length > limit {
			panic!("Length exceeds limit"); // TODO don't panic
		}
		let mut buf = vec![0; length as usize];
		self.read_exact(&mut buf).unwrap();
		Ok(buf)
	}

	fn read_string(&mut self, limit: i32) -> io::Result<String> {
		let bytes = self.read_bytes(limit).unwrap();
		Ok(String::from_utf8_lossy(&bytes).to_string())
	}
}
impl<T> MinecraftReadExt for T where T: Read {}

#[async_trait]
pub trait MinecraftAsyncWriteExt: AsyncWrite + Unpin {
	async fn write_packet<T: Packet + Sync>(
		&mut self,
		packet: &T,
	) -> io::Result<()> {
		self.write_packet_fn(T::ID, |w| packet.write(w))
			.await
	}
	async fn write_packet_fn<H: Send + FnOnce(&mut Cursor<Vec<u8>>) -> io::Result<()>>(
		&mut self,
		packet_id: i32,
		data: H,
	) -> io::Result<()> {
		let out = {
			use crate::ext::MinecraftWriteExt;
			let mut writer = Cursor::new(Vec::new());
			MinecraftWriteExt::write_varint(&mut writer, packet_id)?;
			data(&mut writer)?;
			writer.into_inner()
		};
		{
			use tokio::io::AsyncWriteExt;
			self.write_varint(out.len() as i32).await?;
			self.write_all(&out).await?;
			Ok(())
		}
	}
	async fn write_bytes_async(&mut self, buf: &[u8]) -> io::Result<()> {
		self.write_varint(buf.len() as i32).await?;
		self.write_all(&buf).await?;
		Ok(())
	}
	async fn write_varint(&mut self, mut value: i32) -> io::Result<()> {
		loop {
			let mut temp = value as u8 & 0b01111111;
			value >>= 7;
			if value != 0 {
				temp |= 0b10000000;
			}
			self.write_all(&[temp]).await?;
			if value == 0 {
				break;
			}
		}
		Ok(())
	}
}
impl<T> MinecraftAsyncWriteExt for T where T: AsyncWrite + Unpin {}

#[allow(dead_code)]
pub fn varint_size(mut value: i32) -> usize {
	let mut size = 0;
	loop {
		value >>= 7;
		size += 1;
		if value == 0 {
			break;
		}
	}
	size
}

pub trait MinecraftWriteExt: Write {
	fn write_varint(&mut self, mut value: i32) -> io::Result<()> {
		loop {
			let mut temp = value as u8 & 0b01111111;
			value >>= 7;
			if value != 0 {
				temp |= 0b10000000;
			}
			self.write_all(&[temp])?;
			if value == 0 {
				break;
			}
		}
		Ok(())
	}
	fn write_bytes(&mut self, buf: &[u8]) -> io::Result<()> {
		self.write_varint(buf.len() as i32)?;
		self.write_all(&buf)?;
		Ok(())
	}
	fn write_string(&mut self, str: &str) -> io::Result<()> {
		self.write_bytes(str.as_bytes())?;
		Ok(())
	}
}
impl<T> MinecraftWriteExt for T where T: Write {}
