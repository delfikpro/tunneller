use std::sync::Mutex;
use async_trait::async_trait;

#[async_trait]
pub trait PortManager: Sync {
    fn allocate_port(&self) -> Result<u16, ()>;
    fn free_port(&self, port: u16);
}

pub struct PortManagerImpl {
    pub allocated_ports: Mutex<Vec<bool>>,
    pub port_range_start: u16,
}

pub fn create_port_manager(port_range_start: u16, port_range_size: u16) -> PortManagerImpl {
    PortManagerImpl {
        allocated_ports: Mutex::new(vec![false; port_range_size as usize]),
        port_range_start,
    }
}


#[async_trait]
impl PortManager for PortManagerImpl {

    fn allocate_port(&self) -> Result<u16, ()> {
        let mut ports = self.allocated_ports.lock().unwrap();
        let start = self.port_range_start;

        for i in 0..ports.len() {
            if !ports[i] {
                ports[i] = true;
                return Ok(start.saturating_add(i as u16));
            }
        }

        // All ports are in use
        Err(())
    }

    fn free_port(&self, port: u16) {
        if port < self.port_range_start {
            return;
        }
        let index = port - self.port_range_start;
        let mut ports = self.allocated_ports.lock().unwrap();

        if index as usize >= ports.len() {
            return;
        }
        if ports[index as usize] {
            ports[index as usize] = false;
        }
    }
}


