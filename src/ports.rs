use std::{collections::HashMap, sync::Arc};

use futures::channel::oneshot::{Receiver, Sender};
use nats::{Handler};
use parking_lot::{Mutex};

use crate::tunnel::Tunnel;

pub struct PortManager {
    pub mutex: Mutex<PortManagerData>,
	pub port_range_start: i32,
	pub port_range_size: i32,
}

pub struct PortManagerData {
	pub tunnels: HashMap<String, Arc<Mutex<Tunnel>>>,
	pub kill_senders: HashMap<String, Sender<()>>,
	pub kill_receivers: HashMap<String, Receiver<()>>,
    pub subscriptions: HashMap<String, Handler>,
	pub used_ports: Vec<bool>,
}

pub fn create_port_manager(port_range_start: i32, port_range_size: i32) -> PortManager {
	PortManager {
        mutex: Mutex::new(PortManagerData {
            tunnels: HashMap::new(),
            kill_senders: HashMap::new(),
            kill_receivers: HashMap::new(),
            subscriptions: HashMap::new(),
            used_ports: vec![false; port_range_size as usize]
        }),
        port_range_start,
        port_range_size
	}
}

impl PortManager {
	pub fn get_tunnel(&self, id: &String) -> Option<Arc<Mutex<Tunnel>>> {
        self.mutex.lock().tunnels.get(id).map(|x| x.to_owned())
	}

	pub fn allocate_port(&self) -> Result<i32, ()> {
		for port in 0..self.port_range_size {
			let b = self.mutex.lock().used_ports[port as usize];
			if !b {
				return Ok(self.port_range_start + port);
			}
		}

		// All ports are in use
		Err(())
	}

	pub fn add_tunnel(
		&self,
		id: String,
		public_port: i32,
		tunnel: Arc<Mutex<Tunnel>>,
		kill_sender: Sender<()>,
		kill_receiver: Receiver<()>,
        subscription: Handler
	) {
        let mut data = self.mutex.lock();
		data.tunnels.insert(id.clone(), tunnel);
		data.kill_senders.insert(id.clone(), kill_sender);
		data.kill_receivers.insert(id.clone(), kill_receiver);
        data.subscriptions.insert(id.clone(), subscription);
		data.used_ports[(public_port - self.port_range_start) as usize] = true;
	}

    pub fn remove_tunnel_data(&self, tunnel_id: &String) -> Option<(Tunnel, Receiver<()>, Sender<()>, Handler)> {

        let mut data = self.mutex.lock();
		let removed = data.tunnels.remove(tunnel_id);
        removed.map(|tunnel|{
            let tunnel = tunnel.lock().clone();
            let receiver = data.kill_receivers.remove(tunnel_id).unwrap();
            let sender = data.kill_senders.remove(tunnel_id).unwrap();
            let subscription = data.subscriptions.remove(tunnel_id).unwrap();
            
            (tunnel, receiver, sender, subscription)
        })
    }

	pub async fn remove_tunnel(&self, tunnel_id: &String) -> bool {
        
        let option = self.remove_tunnel_data(tunnel_id);

		match option {
			Some((tunnel, receiver, sender, subscription)) => {

				sender.send(()).expect("kill request sender error");
				receiver.await.expect("kill response receiver error");

                {
                    self.mutex.lock().used_ports[(tunnel.public_port - self.port_range_start) as usize] = false;
                }
                
				println!("Marked port {} as free.", tunnel.public_port);
                subscription.unsubscribe().unwrap();
				true
			}
			None => false,
		}
	}
}
