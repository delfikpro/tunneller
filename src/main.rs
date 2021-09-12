mod ext;
mod plugin;
mod protocol;
mod ports;

use ext::*;
use lazy_static::__Deref;
use plugin::Tunnel;
use protocol::handshake::Handshake;
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::{Mutex, Arc};
use std::net::SocketAddr;
use crate::ports::PortManager;

// const THRESHOLD: i32 = 256;

// #[derive(Debug)]
// pub struct LoggedInInfo {
//     username: String,
//     uuid: String,
//     protocol: i32,
// }

// #[derive(Debug, Error)]
// pub enum SocketLoginError {
//     #[error("io error: {0}")]
//     Io(#[from] io::Error),
//     #[error("unknown packet {1} in {0}")]
//     IncorrectStateIdCombo(State, i32),
//     #[error("client didn't sent login start")]
//     MissingLoginStart,
//     #[error("auth plugin didn't requested encryption")]
//     AuthPluginDidntRequestedEncryption,
//     #[error("auth error: {0}")]
//     AuthError(#[from] AuthError),
// }

// /// Проводит авторизацию юзера/выходит при ошибке/запросе статуса
// async fn handle_socket_login<A: AuthPlugin>(
//     mut stream: TcpStream,
//     auth_plugin: &A,
// ) -> Result<(TcpStream, LoggedInInfo), SocketLoginError> {
//     let mut state = State::Handshaking;
//     let mut protocol = None::<i32>;
//     let mut auth_data = None::<A::AuthData>;
//     loop {
//         let mut initial_buffer = Vec::new();
//         let mut data = stream.read_packet(None, &mut initial_buffer).await?;
//         match (
//             state,
//             data.id()
//                 .expect("in handshake all packets are not compressed"),
//         ) {
//             (State::Handshaking, Handshake::ID) => {
//                 let packet = data.decode::<Handshake>()?;
//                 println!("Handshake: {:?}", packet);
//                 state = packet.next_state;
//                 protocol = Some(packet.protocol.0);
//             }
//             (State::Status, StatusResponse::ID) => {
//                 let req = data.decode::<StatusRequest>()?;
//                 println!("Request: {:?}", req);
//                 stream
//                     .write_packet(
//                         None,
//                         &StatusResponse {
//                             response: r#"
// 							{
// 								"version": {
// 									"name": "Cristalix",
// 									"protocol": 340
// 								},
// 								"players": {
// 									"max": 100,
// 									"online": 20,
// 									"sample": [
// 										{
// 											"name": "Привет мир",
// 											"id": "d6a33537-0444-45be-b12b-af138b1ab81f"
// 										}
// 									]
// 								},
// 								"description": {
// 									"text": "Hello world"
// 								},
// 							"#
//                                 .to_owned(),
//                         },
//                     )
//                     .await?;
//             }
//             (State::Status, Ping::ID) => {
//                 let req = data.decode::<Ping>()?;
//                 stream
//                     .write_packet(
//                         None,
//                         &Pong {
//                             payload: req.payload,
//                         },
//                     )
//                     .await?;
//             }
//             (State::Login, LoginStart::ID) => {
//                 let req = data.decode::<LoginStart>()?;

//                 match auth_plugin.encryption_start(req.name.clone()) {
//                     plugins::auth::EncryptionStartResult::BeginEncryption(request, data) => {
//                         auth_data = Some(data);
//                         stream.write_packet(None, &request).await?;
//                     }
//                     plugins::auth::EncryptionStartResult::Skip(d) => {
//                         break Ok((
//                             stream,
//                             LoggedInInfo {
//                                 username: d.username,
//                                 uuid: d.uuid,
//                                 protocol: protocol.unwrap(),
//                             },
//                         ));
//                     }
//                 }
//             }
//             (State::Login, EncryptionResponse::ID) => {
//                 let auth_data = auth_data
//                     .take()
//                     .ok_or(SocketLoginError::AuthPluginDidntRequestedEncryption)?;
//                 let res = data.decode::<EncryptionResponse>()?;
//                 let success = auth_plugin.encryption_response(auth_data, res).await?;
//                 break Ok((
//                     stream,
//                     LoggedInInfo {
//                         username: success.username,
//                         uuid: success.uuid,
//                         protocol: protocol.unwrap(),
//                     },
//                 ));
//             }
//             (state, id) => break Err(SocketLoginError::IncorrectStateIdCombo(state, id)),
//         }
//     }
// }

// struct ConnectedServerInfo;

// #[derive(Debug, Error)]
// pub enum ServerConnectionError {
//     #[error("io error: {0}")]
//     Io(#[from] io::Error),
//     #[error("proxied server is in online mode")]
//     ServerIsInOnlineMode,
//     #[error("unknown packet {1} in {0}")]
//     IncorrectStateIdCombo(State, i32),
//     #[error("server kicked user with reason: {0}")]
//     Disconnect(String),
//     #[error("server returned wrong name/uuid")]
//     BadLoginSuccess(LoginSuccess),
//     #[error("compression error: {0}")]
//     Compression(#[from] CompressedError),
// }

// /// Открывает соединение с сервером для заданного юзера, проверяет корректность возвращённых данных
// async fn open_server_connection(
//     info: &LoggedInInfo,
//     target: Tunnel,
// ) -> Result<(TcpStream, ConnectedServerInfo), ServerConnectionError> {
//     let address: SocketAddr = format!("{}:{}", target.destination_host, target.destination_port).parse().unwrap();
//     let mut stream = TcpStream::connect(address).await?;
//     let mut buf = Vec::new();
//     println!("Opening");

//     let mut compression = None;
//     stream
//         .write_packet(
//             compression,
//             &Handshake {
//                 address: target.destination_host,
//                 protocol: info.protocol.into(),
//                 port: target.destination_port as i16,
//                 next_state: State::Login,
//             },
//         )
//         .await?;
//     stream
//         .write_packet(
//             compression,
//             &LoginStart {
//                 name: info.username.clone(),
//             },
//         )
//         .await?;
//     // Packet handling loop
//     let state = State::Login;
//     loop {
//         let mut data = stream.read_packet(compression, &mut buf).await?;
//         match (state, data.id()?) {
//             (State::Login, SetCompression::ID) => {
//                 let set_compression = data.decode::<SetCompression>()?;
//                 compression = Some(set_compression.threshold.0);
//             }
//             (State::Login, Disconnect::ID) => {
//                 let disconnect = data.decode::<Disconnect>()?;
//                 break Err(ServerConnectionError::Disconnect(disconnect.reason));
//             }
//             (State::Login, LoginSuccess::ID) => {
//                 let success = data.decode::<LoginSuccess>()?;
//                 println!("{:?}", success);
//                 // if success.username != info.username || success.uuid != info.uuid {
//                 // 	break Err(ServerConnectionError::BadLoginSuccess(success));
//                 // }
//                 if compression != Some(THRESHOLD) {
//                     warn!("Compression settings differ between proxy and server, unnecessary recompressions may be required");
//                 }
//                 return Ok((stream, ConnectedServerInfo));
//             }
//             (State::Login, EncryptionRequest::ID) => {
//                 break Err(ServerConnectionError::ServerIsInOnlineMode);
//             }
//             (state, id) => break Err(ServerConnectionError::IncorrectStateIdCombo(state, id)),
//         };
//     }
// }

// struct StreamPair {
//     user: TcpStream,
//     server: TcpStream,
// }

// /// Проводит общение юзера с сервером, успешно выходит после завершения соединения с сервером, падает при падении клиента
// async fn communicate_user_server(
//     streams: StreamPair,
// ) {
//     let compression = Some(THRESHOLD);
//     let (mut server_read, mut server_write) = streams.server.into_split();
//     let (mut user_read, mut user_write) = streams.user.into_split();

//     let mut s_peek_buf = [0];
//     let mut u_peek_buf = [0];
//     let mut packet_buf = Vec::new();

//     loop {
//         // Если есть пакет от сервера - шлём пакет от сервера
//         // Есть от клиента - шлём от клиента
//         let action = select! {
// 			_ = server_read.peek(&mut s_peek_buf) => {
// 				let packet = server_read.read_packet(compression, &mut packet_buf).await.unwrap();
// 				packet.write(compression, &mut user_write).await.unwrap();
// 			}
// 			_ = user_read.peek(&mut u_peek_buf) => {
// 				let packet = user_read.read_packet(compression, &mut packet_buf).await.unwrap();
// 				packet.write(compression, &mut server_write).await.unwrap();
// 			}
// 		};
//     }
// }

// quick_error! {
// 	#[derive(Debug)]
// 	pub enum SocketError {
// 		Io(err: io::Error) {
// 			from()
// 		}
// 		Login(err: SocketLoginError) {
// 			from()
// 		}
// 		Server(err: ServerConnectionError) {
// 			from()
// 		}
// 	}
// }

async fn handle_stream(
    mut upstream: TcpStream,
    target: Tunnel,
) -> Result<(), Box<dyn std::error::Error>> {

    let mut buf = Vec::new();
    let handshake: Handshake = upstream.read_packet(&mut buf).await?.decode()?;
    
    println!("Got handshake for {:?}: {:?}", target, handshake);

    let address: SocketAddr = format!("{}:{}", target.destination_host, target.destination_port).parse().unwrap();
    println!("Creating a bridge to {}", address);
    let mut downstream = TcpStream::connect(address).await?;

    println!("Sending handshake: {:?}", handshake);
    downstream.write_packet(&handshake).await?;

    println!("Linking the bridge");

    Err(Box::new(tokio::io::copy_bidirectional(&mut upstream, &mut downstream).await.unwrap_err()))
}

async fn create_tunnel(tunnel_mutex: &Mutex<Tunnel>) -> Result<(), Box<dyn std::error::Error>> {
    let address: String;
    {
        let tunnel = tunnel_mutex.lock().unwrap();
        address = format!("0.0.0.0:{}", tunnel.public_port);
    }

    let listener = TcpListener::bind(&address).await?;
    println!("Bound tunnel listener to {}", address);
    
    loop {
        let (stream, _) = listener.accept().await?;
        println!("Got connection: {:?}", stream);

        let tunnel: Tunnel;
        {
            let tunnel_m = tunnel_mutex.lock().unwrap();
            tunnel = tunnel_m.to_owned();
        }
        let tunnel = tunnel;
        tokio::spawn(async move {
            if let Err(e) = handle_stream(stream, tunnel).await {
                println!("User error: {:?}", e);
            };
        });
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let port_range_start = 34100;
    let port_range_size = 100;
    let bind_port = 34064;

    println!("Using ports {}-{} for tunnels", port_range_start, port_range_start + port_range_size - 1);

    let port_manager_arc = Arc::new(ports::create_port_manager(port_range_start, port_range_size));

    rt.block_on(async {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", bind_port)).await?;
        println!("Tunneller endpoint is running on 0.0.0.0:{}", bind_port);
        loop {
            let port_manager = port_manager_arc.clone();
            let (stream, _) = listener.accept().await?;
            println!("New tunnel client: {:?}", stream);
            tokio::spawn(async move {
                let result = new_client(stream, port_manager.deref()).await.unwrap_err();
                println!("Client disconnected: {}", result);
            });
        }
    })
}

#[derive(Debug, Error)]
pub enum TunnellerError {
	#[error("no free ports available")]
	NoFreePorts,
	#[error(transparent)]
	Other(anyhow::Error),
}

async fn new_client(mut stream: TcpStream, port_manager: &impl PortManager) -> Result<(), Box<dyn std::error::Error>> {

    stream.write_u8(0x41).await.unwrap();
    let realm_name = stream.read_string(50).await.unwrap();
    let mut destination_host = stream.read_string(255).await.unwrap();
    if destination_host.is_empty() {
        destination_host = stream.peer_addr().unwrap().ip().to_string();
        println!("Using client ip as the destination host: {}", destination_host)
    }

    let destination_port = stream.read_u16().await.unwrap();

    let public_port: u16;

    match port_manager.allocate_port() {
        Ok(v) => public_port = v,
        _ => {
            // too many active servers on this tunneller instance
            stream.write_u8(1).await.unwrap();
            return Err(Box::new(TunnellerError::NoFreePorts));
        }
    };

    println!("Tunnelling realm {} on {}:{} through 0.0.0.0:{}", realm_name, destination_host, destination_port, public_port);

    let tunnel = Tunnel {
        realm_name,
        public_port,
        destination_host,
        destination_port,
    };

    let tunnel_arc = Arc::new(Mutex::new(tunnel));
    let tunnel_arc_local = tunnel_arc.clone();

    tokio::spawn(async move {
        create_tunnel(tunnel_arc.deref()).await.unwrap();
    });

    // success
    stream.write_u8(0).await.unwrap();
    stream.write_u16(public_port).await.unwrap();

    loop {
        // when a node sends new realm name - we update it
        // it also works as keepalive
        tunnel_arc_local.lock().unwrap().realm_name = stream.read_string(50).await.unwrap();
        stream.write_u8(0).await.unwrap();
    }
}
