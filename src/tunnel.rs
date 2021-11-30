// use futures::channel::oneshot::Sender;
use serde::*;

#[derive(Deserialize)]
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


#[derive(Debug)]
pub struct Tunnel {

    pub id: String,
	pub realm_name: String,
	pub public_port: i32,
	pub destination_host: String,
	pub destination_port: i32,
    // pub kill_sender: Sender<()>

}