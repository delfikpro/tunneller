use super::*;
use std::io::Read;

#[derive(Debug, PacketData)]
pub struct Handshake {
	pub protocol: VarInt,
	pub address: String,
	pub port: i16,
	pub next_state: State,
}
impl Packet for Handshake {
	const ID: i32 = 0;
}
