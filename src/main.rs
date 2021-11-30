mod ext;
mod plugin;
mod ports;
mod protocol;
mod tunnel;

use actix_web::*;
use actix_web::middleware::Logger;
use ext::*;
extern crate lazy_static;
extern crate futures;

use tokio::time;
use std::time::*;

use fast_async_mutex::mutex::Mutex;
use futures::{FutureExt};
use futures::channel::oneshot::{Receiver, Sender, channel};
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
	// println!("Locking on tunnel...");
	let realm_name: String;
	let destination_host: String;
	let destination_port: i32;
	{
		let target_instance = target_mutex.lock().await;
		realm_name = target_instance.realm_name.clone();
		destination_host = target_instance.destination_host.clone();
		destination_port = target_instance.destination_port;
	}
	// println!("Got a tunnel copy, waiting for the handshake...");

	let mut buf = Vec::new();
	let mut handshake: Handshake = upstream.read_packet(&mut buf).await?.decode()?;

	// println!("Got handshake for {:?}: {:?}", realm_name, handshake);

	let args = handshake.address.split("\0").collect::<Vec<&str>>();

	let mut json: Value = serde_json::from_str(args[3])?;

	json.as_array_mut().unwrap().append(&mut vec![json!({
		"name": "realmId",
		"signature": "",
		"value": realm_name
	})]);

	handshake.address = format!(
		"{}\0{}\0{}\0{}",
		args[0],
		args[1],
		args[2],
		json.to_string()
	);

	let address: SocketAddr = format!("{}:{}", destination_host, destination_port)
		.parse()
		.unwrap();
	println!("Tunnelling connection to {} with realm {}", address, realm_name);
	let mut downstream = TcpStream::connect(address).await?;

	// println!("Sending handshake: {:?}", handshake);
	downstream.write_packet(&handshake).await?;

	// println!("Linking the bridge");

	match tokio::io::copy_bidirectional(&mut upstream, &mut downstream)
				.await {
		Ok(_) => Ok(()),
		Err(err) => Err(Box::new(err)),
	}
}

async fn create_tunnel(tunnel_mutex: Arc<Mutex<Tunnel>>, 
	mut kill_receiver: Receiver<()>,
	kill_sender: Sender<()>) -> Result<(), Box<dyn std::error::Error>> {

	let address: String;
	{
		let tunnel = tunnel_mutex.lock().await;
		address = format!("0.0.0.0:{}", tunnel.public_port);
	}


	println!("Binding tunnel listener to {}", address);
	let listener = TcpListener::bind(&address).await?;
	println!("Bound tunnel listener to {}", address);

	loop {

		println!("Awaiting connection...");
		futures::select! {
			result = listener.accept().fuse() => {
				let (stream, _) = result?;
				println!("Got connection: {:?}", stream);
		
				let mutex_arc_2 = tunnel_mutex.clone();
		
				tokio::spawn(async move {
					match handle_stream(stream, &mutex_arc_2.clone()).await {
						Ok(_) => {
							println!("handle_stream terminated successfully")
						},
						Err(err) => {
							println!("Error: {:?}", err);
						},
					}
				});

			},
			_ = kill_receiver => {
				println!("Killing tunnel {:?}...", address);
				drop(listener);
				println!("Killed tunnel {:?}.", address);
				kill_sender.send(()).expect("kill response sender error");
				return Ok(())
			}
		}
	}
}

// this handler gets called if the query deserializes into `Info` successfully
// otherwise a 400 Bad Request error response is returned
#[get("/tunnel")]
async fn index(
	request: HttpRequest,
	info: web::Query<tunnel::TunnellerRequest>,
	data: web::Data<(Mutex<PortManager>, Runtime)>,
    // tokio_runtime: web::Data<Runtime>,
) -> Result<HttpResponse> {
	let runtime = &data.1;
	let mut port_manager = data.0.lock().await;
	let tunnel_id = info.id.clone();
	let realm_name = info.realm.clone();

	let a = &port_manager.get_tunnel(&tunnel_id.clone());
	match a {
		Some(tunnel) => {
			let public_port: i32;
			{
				let mut t = tunnel.lock().await;
				t.realm_name = realm_name.clone();
				public_port = t.public_port;
			}
			if realm_name.is_empty() {
				port_manager.remove_tunnel(&tunnel_id).await;
			}
            Ok(HttpResponse::Ok().json(TunnellerResponse {
                tunnel_id,
                realm: info.realm.clone(),
                public_port,
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
			let (kill_request_sender, kill_request_receiver) = channel::<()>();
			let (kill_response_sender, kill_response_receiver) = channel::<()>();
			
			let t = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis();
			let tunnel = Tunnel {
				id: info.id.to_string(),
				realm_name: info.realm.to_string(),
				public_port,
				destination_host: request.peer_addr().unwrap().ip().to_string(),
				destination_port: info.dst_port,
				last_alive_time: t
			};

			let tunnel_mutex = Mutex::new(tunnel);

            let arc = Arc::new(tunnel_mutex);
            let arc_2 = arc.clone();
			println!("Creating tunnel thread...");
			runtime.spawn(async move {
				println!("Creating tunnel...");
				let result = create_tunnel(arc_2, kill_request_receiver, kill_response_sender)
				.await;
				match result {
					Ok(_) => {},
					Err(err) => println!("{}", err),
				}
            });
            port_manager.add_tunnel(info.id.to_string(), public_port, arc,
		kill_request_sender, kill_response_receiver);

			Ok(HttpResponse::Ok().json(TunnellerResponse {
				tunnel_id: info.id.to_owned(),
				realm: info.realm.to_owned(),
				public_port,
			}))
		}
	}
}

// fn main() {

// }

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	let rt = tokio::runtime::Runtime::new().unwrap();

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

	let port_manager_arc = web::Data::new((Mutex::new(create_port_manager(
		port_range_start,
		port_range_size,
	)), rt));

	let b = port_manager_arc.clone();
	let a = &port_manager_arc.1;
	a.spawn(async move {
		let mut interval_day = time::interval(Duration::from_secs(1));
		loop {
			let now = interval_day.tick().await;
			let time = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis();

			let mut dead_tunnels: Vec<String> = Vec::new();

			let mut port_manager = b.0.lock().await;
			
			let tunnels = &port_manager.tunnels;
			for ele in tunnels {
				let mut t = ele.1.lock().await;
				if Arc::strong_count(&ele.1) <= 2 {
					if time - t.last_alive_time > 5000 {
						println!("Tunnel {} is dead for 30+ seconds", ele.0);
						dead_tunnels.push(ele.0.to_owned());
					}
				} else {
					t.last_alive_time = time
				}
				// println!("Tunnel {} has {} references", ele.0, );
			}
			

			// let mut port_manager = b.0.lock().await;
			for ele in dead_tunnels {
				port_manager.remove_tunnel(&ele).await;
			}

			println!("Renew sitemaps for each day. (Time now = {:?})", now);
		}
	});

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
