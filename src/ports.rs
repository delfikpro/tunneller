use std::{collections::HashMap, sync::Arc};

use fast_async_mutex::mutex::Mutex;

use crate::tunnel::Tunnel;

pub struct PortManager {
	pub tunnels: HashMap<String, Arc<Mutex<Tunnel>>>,
    pub used_ports: Vec<bool>, 
	pub port_range_start: i32,
	pub port_range_size: i32,
}

pub fn create_port_manager(port_range_start: i32, port_range_size: i32) -> PortManager {
	PortManager {
		tunnels: HashMap::new(),
        used_ports: vec![false; port_range_size as usize],
		port_range_start,
		port_range_size,
	}
}

impl PortManager {

    pub fn get_tunnel(&self, id: &String) -> Option<&Arc<Mutex<Tunnel>>> {
        self.tunnels.get(id)
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

    pub fn add_tunnel(&mut self, id: String, tunnel: Arc<Mutex<Tunnel>>) {
        self.tunnels.insert(id, tunnel);
    }

    pub async fn remove_tunnel(&mut self, tunnel_id: String) -> bool {
        let removed = self.tunnels.remove(&tunnel_id);
        match removed {
            Some(t) => {
                let tunnel = t.lock().await;
                self.used_ports[(tunnel.public_port - self.port_range_start) as usize] = false;
                println!("Marked port {} as free.", tunnel.public_port);
                true
            },
            None => false
        }
    }

}
