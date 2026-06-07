use std::{
    collections::HashMap,
    io,
    net::{SocketAddr, ToSocketAddrs},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::config::ServerConfig;

use super::types::ServerName;

pub struct LoadBalancer {
    server_map: HashMap<ServerName, Balancer>,
}

impl LoadBalancer {
    pub fn new(servers: Box<[ServerConfig]>) -> io::Result<Arc<Self>> {
        let mut server_map = HashMap::new();

        for server in servers {
            let balancer = Balancer::new(server.ppv2, server.backends)?;
            server_map.insert(ServerName::new(server.hostname), balancer);
        }

        Ok(Arc::new(Self { server_map }))
    }

    pub fn get_host(self: &Arc<Self>, host: &ServerName) -> Option<&SocketAddr> {
        self.server_map.get(host).and_then(Balancer::take)
    }
}

pub struct Balancer {
    ppv2: bool,
    backends: Box<[SocketAddr]>,
    count: AtomicUsize,
}

impl Balancer {
    pub fn new(ppv2: bool, backends: Box<[String]>) -> io::Result<Self> {
        let mut addrs = Vec::with_capacity(backends.len());
        for backend in backends {
            let addr = backend.to_socket_addrs()?;
            addrs.extend(addr);
        }

        Ok(Self {
            ppv2,
            backends: addrs.into_boxed_slice(),
            count: AtomicUsize::new(0),
        })
    }

    #[allow(dead_code)]
    pub fn is_ppv2(&self) -> bool {
        self.ppv2
    }

    pub fn take(&self) -> Option<&SocketAddr> {
        self.backends
            .get(self.count.fetch_add(1, Ordering::Relaxed) % self.backends.len())
    }
}
