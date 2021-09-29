mod ext;
mod plugin;
mod ports;
mod protocol;
mod tunnel;

use actix_web::*;
use actix_web::dev::Service;
use actix_web::middleware::Logger;
use ext::*;
extern crate lazy_static;

use fast_async_mutex::mutex::Mutex;
use futures::FutureExt;
use log::logger;
use protocol::handshake::Handshake;
use serde_json::{json, Value};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;

use tunnel::{Tunnel, TunnellerResponse};
// use std::sync::{Mutex, Arc};
use crate::ports::{create_port_manager, PortManager};
use std::net::SocketAddr;
use std::sync::Arc;

async fn handle_stream(
	mut upstream: TcpStream,
	target_mutex: &Mutex<Tunnel>,
) -> Result<(), Box<dyn std::error::Error>> {
	let target = target_mutex.lock().await;

	let mut buf = Vec::new();
	let mut handshake: Handshake = upstream.read_packet(&mut buf).await?.decode()?;

	println!("Got handshake for {:?}: {:?}", target, handshake);

	let args = handshake.address.split("\0").collect::<Vec<&str>>();

	let mut json: Value = serde_json::from_str(args[3])?;

	json.as_array_mut().unwrap().append(&mut vec![json!({
		"name": "realmId",
		"value": target.realm_name
	})]);

	handshake.address = format!(
		"{}\0{}\0{}\0{}",
		args[0],
		args[1],
		args[2],
		json.to_string()
	);

	let address: SocketAddr = format!("{}:{}", target.destination_host, target.destination_port)
		.parse()
		.unwrap();
	println!("Creating a bridge to {}", address);
	let mut downstream = TcpStream::connect(address).await?;

	println!("Sending handshake: {:?}", handshake);
	downstream.write_packet(&handshake).await?;

	println!("Linking the bridge");

	Err(Box::new(
		tokio::io::copy_bidirectional(&mut upstream, &mut downstream)
			.await
			.unwrap_err(),
	))
}

async fn create_tunnel(tunnel_mutex: Arc<Mutex<Tunnel>>) -> Result<(), Box<dyn std::error::Error>> {

	let address: String;
	{
		let tunnel = tunnel_mutex.lock().await;
		address = format!("0.0.0.0:{}", tunnel.public_port);
	}

	println!("Bindning tunnel listener to {}", address);

	let listener = TcpListener::bind(&address).await?;
	println!("Bound tunnel listener to {}", address);

	loop {
		let (stream, _) = listener.accept().await?;
		println!("Got connection: {:?}", stream);

		let mutex_arc_2 = tunnel_mutex.clone();

		tokio::spawn(async move {
			if let Err(e) = handle_stream(stream, mutex_arc_2.as_ref()).await {
				println!("User error: {:?}", e);
			};
		});
	}
}

// this handler gets called if the query deserializes into `Info` successfully
// otherwise a 400 Bad Request error response is returned
#[get("/tunnel")]
async fn index(
	request: HttpRequest,
	info: web::Query<tunnel::TunnellerRequest>,
	port_manager_mutex: web::Data<Mutex<PortManager>>,
    // tokio_runtime: web::Data<Runtime>,
) -> Result<HttpResponse> {
	let mut port_manager = port_manager_mutex.lock().await;
	match &port_manager.get_tunnel(&info.id) {
		Some(tunnel) => {
            let t = tunnel.lock().await;
            Ok(HttpResponse::Ok().json(TunnellerResponse {
                tunnel_id: t.id.to_owned(),
                realm: t.realm_name.to_owned(),
                public_port: t.public_port,
            }))
        },
		None => {
			println!("Allocating port...");
			let public_port = match port_manager.allocate_port() {
				Ok(port) => port,
				Err(_) => {
					println!("No free ports available");
					return Ok(HttpResponse::ServiceUnavailable().finish());
				}
			};
			println!("Allocated port {}", public_port);

			let tunnel = Tunnel {
				id: info.id.to_string(),
				realm_name: info.realm.to_string(),
				public_port,
				destination_host: request.peer_addr().unwrap().ip().to_string(),
				destination_port: info.dst_port,
			};

			let tunnel_mutex = Mutex::new(tunnel);

            let arc = Arc::new(tunnel_mutex);
            let arc_2 = arc.clone();
			println!("Creating tunnel thread...");
			tokio::runtime::Runtime::new().unwrap().spawn(async move {
				println!("Creating tunnel...");
                create_tunnel(arc_2)
            });
            port_manager.add_tunnel(info.id.to_string(), arc);

			Ok(HttpResponse::Ok().json(TunnellerResponse {
				tunnel_id: info.id.to_owned(),
				realm: info.realm.to_owned(),
				public_port: 123,
			}))
		}
	}
}

// fn main() {

// }

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	let rt = tokio::runtime::Runtime::new().unwrap();

    let rt_arc = web::Data::new(rt);

	let port_range_start = 34100;
	let port_range_size = 100;
	let bind_port: i32 = 34064;

	std::env::set_var("RUST_LOG", "info");
	std::env::set_var("RUST_BACKTRACE", "1");
	env_logger::init();

	println!(
		"Using ports {}-{} for tunnels",
		port_range_start,
		port_range_start + port_range_size - 1
	);

	let port_manager_arc = web::Data::new(Mutex::new(create_port_manager(
		port_range_start,
		port_range_size,
	)));
	HttpServer::new(move || {
		App::new()
		.app_data(port_manager_arc.clone())
		// .app_data(rt_arc.clone())
		.wrap(Logger::new("%a %t %r %s %b"))
		.service(index)
	})
	.disable_signals()
	.bind(format!("0.0.0.0:{}", bind_port))?
	.run()
	.await

	// rt.block_on(async {
	// 	let listener = TcpListener::bind(format!("0.0.0.0:{}", bind_port)).await?;
	// 	println!("Tunneller endpoint is running on 0.0.0.0:{}", bind_port);
	// 	loop {
	// 		let port_manager = port_manager_arc.clone();
	// 		let (stream, _) = listener.accept().await?;
	// 		println!("New tunnel client: {:?}", stream);
	// 		tokio::spawn(async move {
	// 			let result = new_client(stream, port_manager.as_ref()).await.unwrap_err();
	// 			println!("Client disconnected: {}", result);
	// 		});
	// 	}
	// })
}

#[derive(Debug, Error)]
pub enum TunnellerError {
	#[error("no free ports available")]
	NoFreePorts,
	#[error(transparent)]
	Other(anyhow::Error),
}

// async fn new_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
// 	stream.write_u8(0x41).await.unwrap();
// 	let realm_name = stream.read_string(50).await.unwrap();
// 	let mut destination_host = stream.read_string(255).await.unwrap();
// 	if destination_host.is_empty() {
// 		destination_host = stream.peer_addr().unwrap().ip().to_string();
// 		println!(
// 			"Using client ip as the destination host: {}",
// 			destination_host
// 		)
// 	}

// 	let destination_port = stream.read_u16().await.unwrap();

// 	let public_port: u16;

// 	match port_manager.allocate_port() {
// 		Ok(v) => public_port = v,
// 		_ => {
// 			// too many active servers on this tunneller instance
// 			stream.write_u8(1).await.unwrap();
// 			return Err(Box::new(TunnellerError::NoFreePorts));
// 		}
// 	};

// 	println!(
// 		"Tunnelling realm {} on {}:{} through 0.0.0.0:{}",
// 		realm_name, destination_host, destination_port, public_port
// 	);

// 	let tunnel = TunnelInfo {
// 		realm_name,
// 		public_port,
// 		destination_host,
// 		destination_port,
// 	};

// 	let tunnel_arc = Arc::new(Mutex::new(tunnel));
// 	let tunnel_arc_local = tunnel_arc.clone();

// 	tokio::spawn(async move {
// 		create_tunnel(tunnel_arc.as_ref()).await.unwrap();
// 	});

// 	// success
// 	stream.write_u8(0).await.unwrap();
// 	stream.write_u16(public_port).await.unwrap();

// 	loop {
// 		// when a node sends new realm name - we update it
// 		// it also works as keepalive
// 		tunnel_arc_local.lock().unwrap().realm_name = stream.read_string(50).await.unwrap();
// 		stream.write_u8(0).await.unwrap();
// 	}
// }
