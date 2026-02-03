use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct PoolConfig {
    pub per_host_limit: usize,
    pub per_host_limit_h2: usize,
    pub connect_timeout_ms: u64,
    pub read_timeout_ms: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            per_host_limit: 6,
            per_host_limit_h2: 2,
            connect_timeout_ms: 5000,
            read_timeout_ms: 30000,
        }
    }
}

#[derive(Debug)]
struct HostState {
    active_connections: usize,
    is_http2: bool,
}

pub struct ConnectionPool {
    config: PoolConfig,
    hosts: Arc<Mutex<HashMap<String, HostState>>>,
}

impl ConnectionPool {
    pub fn new(config: PoolConfig) -> Self {
        Self {
            config,
            hosts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn can_connect(&self, host: &str) -> bool {
        let hosts = self.hosts.lock();
        match hosts.get(host) {
            Some(state) => {
                let limit = if state.is_http2 {
                    self.config.per_host_limit_h2
                } else {
                    self.config.per_host_limit
                };
                state.active_connections < limit
            }
            None => true,
        }
    }

    pub fn acquire(&self, host: &str, is_http2: bool) -> bool {
        let mut hosts = self.hosts.lock();
        let state = hosts.entry(host.to_string()).or_insert(HostState {
            active_connections: 0,
            is_http2,
        });

        let limit = if state.is_http2 {
            self.config.per_host_limit_h2
        } else {
            self.config.per_host_limit
        };

        if state.active_connections < limit {
            state.active_connections += 1;
            true
        } else {
            false
        }
    }

    pub fn release(&self, host: &str) {
        let mut hosts = self.hosts.lock();
        if let Some(state) = hosts.get_mut(host) {
            state.active_connections = state.active_connections.saturating_sub(1);
        }
    }

    pub fn set_http2(&self, host: &str) {
        let mut hosts = self.hosts.lock();
        if let Some(state) = hosts.get_mut(host) {
            state.is_http2 = true;
        }
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new(PoolConfig::default())
    }
}
