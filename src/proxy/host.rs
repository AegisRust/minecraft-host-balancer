use std::{collections::HashMap, net::AddrParseError, sync::Arc};

use crate::{
    config::ServerConfig,
    proxy::loadbalancer::{Balancer, GuardBalancer},
};

pub struct HostManager {
    server_map: HashMap<String, Balancer>,
}

pub type SmartHostManager = Arc<HostManager>;

impl HostManager {
    pub fn new(servers: Vec<ServerConfig>) -> Result<Self, AddrParseError> {
        let mut server_map = HashMap::new();

        for server in servers {
            let balancer = Balancer::new(server.ppv2, server.backends)?;
            server_map.insert(server.hostname, balancer);
        }

        Ok(Self { server_map })
    }

    pub fn get_host(&self, host: &str) -> Option<GuardBalancer<'_>> {
        self.server_map.get(host).map(GuardBalancer::new)
    }
}
