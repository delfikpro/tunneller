pub mod handshake;
pub mod login;
mod packet;
pub mod play;
pub mod status;

use std::fmt::{self, Display};
use std::io::{Read, Write};
use tokio::io;

pub use crate::ext::MinecraftReadExt;
pub use crate::ext::MinecraftWriteExt;
use derive_packetdata::PacketData;
pub use packet::*;

#[derive(Debug, Clone, Copy)]
pub enum State {
	Handshaking,
	Status,
	Login,
	Play,
}
impl Display for State {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"{}",
			match self {
				State::Handshaking => "HANDSHAKE",
				State::Status => "STATUS",
				State::Login => "LOGIN",
				State::Play => "PLAY",
			}
		)
	}
}
impl PacketData for State {
	fn read<R: Read>(buf: &mut R) -> io::Result<Self> {
		Ok(match buf.read_varint()?.ans {
			0 => Self::Handshaking,
			1 => Self::Status,
			2 => Self::Login,
			3 => Self::Play,
			_ => unreachable!(),
		})
	}
	fn write<W: std::io::Write>(&self, buf: &mut W) -> io::Result<()> {
		buf.write_varint(match self {
			State::Handshaking => 0,
			State::Status => 1,
			State::Login => 2,
			State::Play => 3,
		})?;
		Ok(())
	}
}
