use std::{collections::HashMap, sync::Arc};

use fast_async_mutex::mutex::Mutex;
use futures::channel::oneshot::{Receiver, Sender};
use nats::{Handler};

use crate::tunnel::Tunnel;

pub struct PortManager {
	pub tunnels: HashMap<String, Arc<Mutex<Tunnel>>>,
	pub kill_senders: HashMap<String, Sender<()>>,
	pub kill_receivers: HashMap<String, Receiver<()>>,
    pub subscriptions: HashMap<String, Handler>,
	pub used_ports: Vec<bool>,
	pub port_range_start: i32,
	pub port_range_size: i32,
}

pub fn create_port_manager(port_range_start: i32, port_range_size: i32) -> PortManager {
	PortManager {
		tunnels: HashMap::new(),
		kill_senders: HashMap::new(),
		kill_receivers: HashMap::new(),
		subscriptions: HashMap::new(),
		used_ports: vec![false; port_range_size as usize],
		port_range_start,
		port_range_size,
	}
}

impl PortManager {
	pub fn get_tunnel(&self, id: &String) -> Option<Arc<Mutex<Tunnel>>> {
		self.tunnels.get(id).map(|x| x.to_owned())
	}

	pub fn allocate_port(&self) -> Result<i32, ()> {
		for port in 0..self.port_range_size {
			let b = self.used_ports[port as usize];
			if !b {
				return Ok(self.port_range_start + port);
			}
		}

		// All ports are in use
		Err(())
	}

	pub fn add_tunnel(
		&mut self,
		id: String,
		public_port: i32,
		tunnel: Arc<Mutex<Tunnel>>,
		kill_sender: Sender<()>,
		kill_receiver: Receiver<()>,
        subscription: Handler
	) {
		self.tunnels.insert(id.clone(), tunnel);
		self.kill_senders.insert(id.clone(), kill_sender);
		self.kill_receivers.insert(id.clone(), kill_receiver);
        self.subscriptions.insert(id.clone(), subscription);
		self.used_ports[(public_port - self.port_range_start) as usize] = true;
	}

	pub async fn remove_tunnel(&mut self, tunnel_id: &String) -> bool {
		let removed = self.tunnels.remove(tunnel_id);
		match removed {
			Some(t) => {
				let tunnel = t.lock().await;
				let receiver = self.kill_receivers.remove(tunnel_id).unwrap();
				let sender = self.kill_senders.remove(tunnel_id).unwrap();
                let subscription = self.subscriptions.remove(tunnel_id).unwrap();

				sender.send(()).expect("kill request sender error");
				receiver.await.expect("kill response receiver error");

                
				self.used_ports[(tunnel.public_port - self.port_range_start) as usize] = false;
				println!("Marked port {} as free.", tunnel.public_port);
                subscription.unsubscribe().unwrap();
				true
			}
			None => false,
		}
	}
}
