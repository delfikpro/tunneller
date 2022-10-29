mod ext;
mod plugin;
mod ports;
mod protocol;
mod tunnel;

use actix_web::middleware::Logger;
use actix_web::*;
use ext::*;
use std::env;
use std::fmt::Debug;

extern crate futures;
extern crate lazy_static;

use nats::Connection;
use parking_lot::Mutex;
use std::time::*;
use tokio::time;

use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::FutureExt;
use protocol::handshake::Handshake;
use serde_json::{json, Value};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};

use crate::ports::{create_port_manager, PortManager};
use crate::tunnel::TunnellerRequest;
use std::net::SocketAddr;
use std::sync::Arc;
use tunnel::{Tunnel, TunnellerResponse};

async fn handle_stream(
	mut upstream: TcpStream,
	target_mutex: &Mutex<Tunnel>,
) -> Result<(), Box<dyn std::error::Error>> {
	// println!("Locking on tunnel...");
	let realm_name: String;
	let destination_host: String;
	let destination_port: i32;
	{
		let target_instance = target_mutex.lock();
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
	println!(
		"Tunnelling connection to {} with realm {}",
		address, realm_name
	);
	let mut downstream = TcpStream::connect(address).await?;

	// println!("Sending handshake: {:?}", handshake);
	downstream.write_packet(&handshake).await?;

	// println!("Linking the bridge");

	match tokio::io::copy_bidirectional(&mut upstream, &mut downstream).await {
		Ok(_) => Ok(()),
		Err(err) => Err(Box::new(err)),
	}
}

async fn create_tunnel(
	tunnel_mutex: Arc<Mutex<Tunnel>>,
	mut kill_receiver: Receiver<()>,
	kill_sender: Sender<()>,
) -> Result<(), Box<dyn std::error::Error>> {
	let address: String;
	{
		let tunnel = tunnel_mutex.lock();
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

#[get("/tunnels")]
async fn get_tunnels(_: HttpRequest, data: web::Data<Arc<Application>>) -> Result<HttpResponse> {
	let mut tunnels = Vec::<Tunnel>::new();
	for ele in &data.port_manager.mutex.lock().tunnels {
		let t = ele.1.lock().clone();
		tunnels.push(t);
	}
	return Ok(HttpResponse::Ok().json(tunnels));
}

// this handler gets called if the query deserializes into `Info` successfully
// otherwise a 400 Bad Request error response is returned
#[get("/tunnel")]
async fn index(
	request: HttpRequest,
	info: web::Query<tunnel::TunnellerRequest>,
	data: web::Data<Arc<Application>>,
	// tokio_runtime: web::Data<Runtime>,
) -> Result<HttpResponse> {
	let port_manager = data.port_manager.clone();
	let tunnel_id = info.id.clone();
	let realm_name = info.realm.clone();

	let mut info = info.clone();
	info.dst_host = request.peer_addr().unwrap().ip().to_string();

	let a = &port_manager.get_tunnel(&tunnel_id.clone());
	match a {
		Some(tunnel) => {
			let public_port: i32;
			{
				let mut t = tunnel.lock();
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
		}
		None => {
			let tunnel = create_and_init_tunnel(&data, &info).await;
			match tunnel {
				Ok(tunnel) => Ok(HttpResponse::Ok().json(TunnellerResponse {
					tunnel_id: info.id.to_owned(),
					realm: info.realm.to_owned(),
					public_port: tunnel.public_port,
				})),
				Err(_) => Ok(HttpResponse::ServiceUnavailable().body("No free ports available")),
			}
		}
	}
}

fn current_time_millis() -> u128 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("Time went backwards")
		.as_millis()
}

async fn create_and_init_tunnel(
	app: &Arc<Application>,
	info: &TunnellerRequest,
) -> Result<Tunnel, ()> {
	println!("Allocating port...");

	let public_port = match app.port_manager.allocate_port() {
		Ok(port) => port,
		Err(_) => {
			println!("No free ports available");
			return Err(());
		}
	};
	println!("Allocated port {}", public_port);
	let (kill_request_sender, kill_request_receiver) = channel::<()>();
	let (kill_response_sender, kill_response_receiver) = channel::<()>();

	let t = current_time_millis();
	let tunnel = Tunnel {
		id: info.id.to_string(),
		realm_name: info.realm.to_string(),
		public_port,
		public_host: app.public_host.clone(),
		destination_host: info.dst_host.clone(),
		destination_port: info.dst_port,
		last_occupied_time: t,
		last_issuer_alive_time: t,
		online: 0,
	};

	let tunnel_mutex = Arc::new(Mutex::new(tunnel.clone()));

	{
		let tunnel_mutex = tunnel_mutex.clone();
		println!("Creating tunnel thread...");
		app.rt.spawn(async move {
			println!("Creating tunnel...");
			let result =
				create_tunnel(tunnel_mutex, kill_request_receiver, kill_response_sender).await;
			match result {
				Ok(_) => {}
				Err(err) => println!("{}", err),
			}
		});
	}

	let subscription = app
		.broker
		.subscribe(format!("cristalixconnect.main.tunnel_issuer_alive.{}", tunnel.id).as_str())
		.unwrap();

	let handler;

	{
		let tunnel_mutex = tunnel_mutex.clone();
		let app = app.clone();
		handler = subscription.with_handler(move |msg| {
			app.rt.block_on(async {
				let mut tunnel = tunnel_mutex.lock();
				tunnel.last_issuer_alive_time = current_time_millis();
				let request: TunnellerRequest =
					serde_json::from_str(std::str::from_utf8(&msg.data).unwrap()).unwrap();
				tunnel.realm_name = request.realm;
			});
			Ok(())
		});
	}

	app.port_manager.add_tunnel(
		info.id.to_string(),
		public_port,
		tunnel_mutex,
		kill_request_sender,
		kill_response_receiver,
		handler,
	);

	Ok(tunnel)
}

// fn main() {

// }

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	main0().await
}

struct Application {
	pub public_host: String,
	// pub port_range_start: i32,
	// pub port_range_size: i32,
	pub port_manager: Arc<PortManager>,
	pub rt: Arc<tokio::runtime::Runtime>,
	pub broker: Arc<Connection>,
}

fn get_i32_env(key: &str, default_value: i32) -> i32 {
	env::var(key)
		.ok()
		.map(|str| {
			str.parse()
				.expect(format!("unable to parse value {} of env {}", str, key).as_str())
		})
		.unwrap_or(default_value)
}

// thank you, rust error highlighting.
async fn main0() -> std::io::Result<()> {
	let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
	let port_range_start = get_i32_env("TUNNELLER_PORT_RANGE_START", 34100);
	let port_range_size = get_i32_env("TUNNELLER_PORT_RANGE_SIZE", 100);
	let bind_port: i32 = get_i32_env("TUNNELLER_BIND_PORT", 34064);

	let nats_address: String =
		env::var("TUNNELLER_NATS_ADDRESS").unwrap_or_else(|_| String::from("127.0.0.1:4222"));

	std::env::set_var("RUST_LOG", "info");
	std::env::set_var("RUST_BACKTRACE", "1");
	env_logger::init();

	let public_host = match rt.block_on(async { public_ip::addr_v4().await }) {
		Some(ip) => {
			println!("Using public hostname: {:?}", ip);
			format!("{:?}", ip)
		}
		None => {
			println!("Couldn't automatically resolve public hostname.");
			return Ok(());
		}
	};

	let port_manager = Arc::new(create_port_manager(port_range_start, port_range_size));

	let broker = Arc::new(nats::connect(nats_address)?);

	let broker_sub = broker.clone();

	println!(
		"Using ports {}-{} for tunnels",
		port_range_start,
		port_range_start + port_range_size - 1
	);

	let app = Arc::new(Application {
		public_host,
		// port_range_start,
		// port_range_size,
		port_manager,
		rt,
		broker,
	});

	let web_data = web::Data::new(app.clone());

	// let app = app.clone();

	{
		let app = app.clone();
		app.broker
			.subscribe("cristalixconnect.main.tunnellerinfo.request")?
			.with_handler(move |_| {
				app.rt.block_on(async {
					let port_manager = app.port_manager.clone();
					let mut tunnels = Vec::<Tunnel>::new();
					for ele in &port_manager.mutex.lock().tunnels {
						let t = ele.1.lock().clone();
						tunnels.push(t);
					}
					app.broker.publish(
						"cristalixconnect.main.tunnellerinfo.response",
						serde_json::to_string(&tunnels).unwrap().as_bytes(),
					)
				})
			});
	}

	let subscription = app.broker.queue_subscribe(
		"cristalixconnect.main.tunnelcreate",
		"cristalixconnect.main.tunnelcreate",
	)?;

	let app_clone = app.clone();
	subscription.with_handler(move |msg| {
		let request: TunnellerRequest =
			serde_json::from_str(std::str::from_utf8(&msg.data).unwrap())?;
		println!("data: {:?}", request);

		let tunnel_id = request.id.clone();
		let realm_name = request.realm.clone();

		// ToDo: Maybe worth making this code non-blocking
		match app_clone.rt.block_on(async {
			let port_manager = app_clone.port_manager.clone();
			let existing_tunnel = port_manager.get_tunnel(&tunnel_id.clone());
			drop(port_manager);
			match existing_tunnel {
				Some(tunnel) => {
					let mut t = tunnel.lock();
					t.realm_name = realm_name.clone();
					// ToDo: remove realm if empty id
					Ok(t.clone())
				}
				None => create_and_init_tunnel(&app_clone, &request).await,
			}
		}) {
			Ok(tunnel) => {
				broker_sub.publish(
					format!("cristalixconnect.main.tunnelalive.{}", request.id).as_str(),
					serde_json::to_string(&tunnel)?.as_bytes(),
				)?;
			}
			Err(_) => {
				broker_sub.publish(
					format!("cristalixconnect.main.tunnelalive.{}", request.id).as_str(),
					"null",
				)?;
			}
		}

		Ok(())
	});

	app.rt.spawn({
		let app = app.clone();
		async move {
			let mut update_interval = time::interval(Duration::from_secs(1));
			loop {
				update_interval.tick().await;
				let time = SystemTime::now()
					.duration_since(UNIX_EPOCH)
					.expect("Time went backwards")
					.as_millis();

				let task = app.rt.spawn({
					let app = app.clone();
					async move {
						let mut dead_tunnels: Vec<String> = Vec::new();

						{
							let port_manager = app.port_manager.clone();

							let tunnels = &port_manager.mutex.lock().tunnels;
							println!("Currently there are {} tunnels", tunnels.keys().len());
							for ele in tunnels {
								let mut t = ele.1.lock();
								let ref_count = Arc::strong_count(&ele.1);
								if ref_count <= 3 {
									if time - t.last_occupied_time > 60000
										&& time - t.last_issuer_alive_time > 15000
									{
										println!("Tunnel {} is dead for 60+ seconds", ele.0);
										dead_tunnels.push(ele.0.to_owned());
									}
								} else {
									println!(
										"Tunnel {} is alive and has {} references",
										t.id,
										Arc::strong_count(&ele.1)
									);
									t.last_occupied_time = time;
								}

								// 3 references to the tunnel mutex from different parts of program & two more for each active connection
								t.online = (ref_count as i32 - 3) / 2;

								let broker_subject =
									format!("cristalixconnect.main.tunnelalive.{}", t.id);
								app.broker
									.publish(
										broker_subject.as_str(),
										serde_json::to_string(&t.clone()).unwrap().as_bytes(),
									)
									.unwrap();
								// println!("Tunnel {} has {} references", ele.0, );
							}
						}

						let port_manager = app.port_manager.clone();
						for ele in dead_tunnels {
							port_manager.remove_tunnel(&ele).await;
						}
					}
				});
				let result = task.await;
				if result.is_err() {
					println!("Error while updating tunnels: {}", result.unwrap_err());
				}
			}
		}
	});

	HttpServer::new(move || {
		App::new()
			.app_data(web_data.clone())
			// .app_data(rt_arc.clone())
			.wrap(Logger::new("%a %t %r %s %b"))
			.service(index)
			.service(get_tunnels)
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
