
#[derive(PartialEq, Clone, Debug)]
pub struct Tunnel {

	pub realm_name: String,
	pub public_port: u16,
	pub destination_host: String,
	pub destination_port: u16,

}

