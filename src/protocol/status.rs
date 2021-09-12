use super::*;

#[derive(Debug)]
pub struct StatusRequest;
impl Packet for StatusRequest {
	const ID: i32 = 0;
}
impl PacketData for StatusRequest {
	fn read<R: Read>(_buf: &mut R) -> io::Result<Self> {
		Ok(StatusRequest)
	}
	fn write<W: std::io::Write>(&self, _buf: &mut W) -> io::Result<()> {
		todo!()
	}
}

#[derive(PacketData)]
pub struct StatusResponse {
	pub response: String,
}
impl Packet for StatusResponse {
	const ID: i32 = 0;
}

#[derive(PacketData)]
pub struct Ping {
	pub payload: i64,
}
impl Packet for Ping {
	const ID: i32 = 0x01;
}

#[derive(PacketData)]
pub struct Pong {
	pub payload: i64,
}
impl Packet for Pong {
	const ID: i32 = 0x01;
}
