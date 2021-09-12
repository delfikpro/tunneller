use super::*;

#[derive(Debug, PacketData)]
pub struct LoginStart {
	pub name: String,
}
impl Packet for LoginStart {
	const ID: i32 = 0x00;
}

#[derive(Debug, PacketData)]
pub struct Disconnect {
	pub reason: String,
}
impl Packet for Disconnect {
	const ID: i32 = 0x00;
}

#[derive(Debug, PacketData)]
pub struct SetCompression {
	pub threshold: VarInt,
}
impl Packet for SetCompression {
	const ID: i32 = 0x03;
}

#[derive(Debug, PacketData)]
pub struct LoginSuccess {
	pub uuid: String,
	pub username: String,
}
impl Packet for LoginSuccess {
	const ID: i32 = 0x02;
}

#[derive(Debug, PacketData)]
pub struct EncryptionResponse {
	pub shared_secret: Vec<u8>,
	pub verify_token: Vec<u8>,
}
impl Packet for EncryptionResponse {
	const ID: i32 = 0x01;
}

#[derive(PacketData)]
pub struct EncryptionRequest {
	pub server_id: String,
	pub public: Vec<u8>,
	pub verify_token: Vec<u8>,
}
impl Packet for EncryptionRequest {
	const ID: i32 = 0x01;
}
