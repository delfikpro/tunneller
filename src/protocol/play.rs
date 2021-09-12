use super::*;

#[derive(PacketData)]
pub struct JoinGame {
	pub entity_id: i32,
	pub game_mode: u8,
	pub dimension: i32,
	pub difficulty: u8,
	pub max_players: u8,
	pub level_type: String,
	pub reduced_debug_info: bool,
}

impl Packet for JoinGame {
	const ID: i32 = 0x23;
}

#[derive(Debug, PacketData)]
pub struct ChatRequest {
	pub message: String,
}
impl Packet for ChatRequest {
	const ID: i32 = 0x02;
}

#[derive(Debug, PacketData)]
pub struct ChatResponse {
	pub message: String,
	pub position: u8,
}
impl Packet for ChatResponse {
	const ID: i32 = 0x0F;
}

#[derive(Debug, PacketData)]
pub struct KeepAlive {
	pub random_id: i16,
}
impl Packet for KeepAlive {
	const ID: i32 = 0x21;
}
