// use futures::channel::oneshot::Sender;
use serde::*;

#[derive(Deserialize, Clone, Debug)]
pub struct TunnellerRequest {
    pub id: String,
    pub realm: String,
    pub dst_host: String,
    pub dst_port: i32
}

#[derive(Serialize)]
pub struct TunnellerResponse {
    pub tunnel_id: String,
    pub realm: String,
    pub public_port: i32
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tunnel {

    pub id: String,
	pub realm_name: String,
    pub public_host: String,
	pub public_port: i32,
	pub destination_host: String,
	pub destination_port: i32,
    pub last_occupied_time: u128,
    pub last_issuer_alive_time: u128,
    pub online: i32,
    // pub kill_sender: Sender<()>

}
