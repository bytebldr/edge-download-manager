use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::Client;

use super::types::TransportProtocol;

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36 Edg/135.0.0.0";
const H2_POOL_SIZE: usize = 8;
const MAX_REGISTRY_ENTRIES: usize = 16;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ProxyKey {
    pub url: String,
}

impl ProxyKey {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into().trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ClientKey {
    proxy: Option<ProxyKey>,
    timeout_secs: u64,
}

#[derive(Clone)]
pub struct ClientPair {
    pub h1_client: Client,
    pub h2_pool: Vec<Client>,
}

impl ClientPair {
    pub fn build(proxy: Option<&ProxyKey>, timeout_secs: u64) -> Result<Self, String> {
        let req_timeout = Duration::from_secs(timeout_secs.max(5));
        let parsed_proxy = match proxy {
            Some(p) if !p.url.is_empty() => Some(
                reqwest::Proxy::all(&p.url)
                    .map_err(|e| format!("Invalid proxy URL '{}': {}", p.url, e))?,
            ),
            _ => None,
        };

        let mut h1_builder = Client::builder()
            .http1_only()
            .tcp_nodelay(true)
            .pool_max_idle_per_host(64)
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .connect_timeout(Duration::from_secs(5))
            .timeout(req_timeout)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(DEFAULT_USER_AGENT);
        if let Some(ref p) = parsed_proxy {
            h1_builder = h1_builder.proxy(p.clone());
        }
        let h1_client = h1_builder
            .build()
            .map_err(|e| format!("Failed to build HTTP/1.1 client: {}", e))?;

        let mut h2_pool = Vec::with_capacity(H2_POOL_SIZE);
        for i in 0..H2_POOL_SIZE {
            let mut h2_builder = Client::builder()
                .http2_adaptive_window(true)
                .tcp_nodelay(true)
                .pool_max_idle_per_host(8)
                .pool_idle_timeout(Some(Duration::from_secs(90)))
                .tcp_keepalive(Some(Duration::from_secs(30)))
                .connect_timeout(Duration::from_secs(5))
                .timeout(req_timeout)
                .no_gzip()
                .no_brotli()
                .no_deflate()
                .redirect(reqwest::redirect::Policy::none())
                .user_agent(DEFAULT_USER_AGENT);
            if let Some(ref p) = parsed_proxy {
                h2_builder = h2_builder.proxy(p.clone());
            }
            let c = h2_builder
                .build()
                .map_err(|e| format!("Failed to build HTTP/2 pool client {}: {}", i, e))?;
            h2_pool.push(c);
        }

        Ok(Self { h1_client, h2_pool })
    }

    pub fn probe_client(&self) -> Client {
        self.h2_pool
            .first()
            .cloned()
            .unwrap_or_else(|| self.h1_client.clone())
    }

    pub fn client_for_worker(&self, worker_idx: u32, proto: TransportProtocol) -> Client {
        match proto {
            TransportProtocol::Http1 => self.h1_client.clone(),
            TransportProtocol::Http2 | TransportProtocol::Http3 => {
                if self.h2_pool.is_empty() {
                    self.h1_client.clone()
                } else {
                    self.h2_pool[(worker_idx as usize) % self.h2_pool.len()].clone()
                }
            }
        }
    }
}

/// LRU-bounded registry of `(Option<ProxyKey>, timeout_secs)` -> `Arc<ClientPair>`.
pub struct ClientRegistry {
    inner: Mutex<ClientRegistryInner>,
}

struct ClientRegistryInner {
    map: HashMap<ClientKey, Arc<ClientPair>>,
    order: VecDeque<ClientKey>,
    capacity: usize,
}

impl ClientRegistry {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(ClientRegistryInner {
                map: HashMap::new(),
                order: VecDeque::new(),
                capacity: capacity.max(1),
            }),
        }
    }

    pub fn get_or_build(
        &self,
        proxy: Option<&ProxyKey>,
        timeout_secs: u64,
    ) -> Result<Arc<ClientPair>, String> {
        let key = ClientKey {
            proxy: proxy.cloned(),
            timeout_secs: timeout_secs.max(5),
        };
        {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(pair) = inner.map.get(&key).cloned() {
                inner.order.retain(|k| k != &key);
                inner.order.push_back(key);
                return Ok(pair);
            }
        }

        let built = Arc::new(ClientPair::build(proxy, timeout_secs)?);

        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = inner.map.get(&key).cloned() {
            return Ok(existing);
        }
        if inner.map.len() >= inner.capacity {
            if let Some(evicted) = inner.order.pop_front() {
                inner.map.remove(&evicted);
            }
        }
        inner.order.push_back(key.clone());
        inner.map.insert(key, built.clone());
        Ok(built)
    }
}

impl Default for ClientRegistry {
    fn default() -> Self {
        Self::new(MAX_REGISTRY_ENTRIES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_registry_lru_eviction() {
        let reg = ClientRegistry::new(2);
        let p1 = ProxyKey::new("http://127.0.0.1:8081");
        let p2 = ProxyKey::new("http://127.0.0.1:8082");
        let p3 = ProxyKey::new("http://127.0.0.1:8083");

        let _ = reg.get_or_build(Some(&p1), 20).unwrap();
        let _ = reg.get_or_build(Some(&p2), 20).unwrap();
        // Touch p1 so p2 becomes LRU
        let _ = reg.get_or_build(Some(&p1), 20).unwrap();
        let _ = reg.get_or_build(Some(&p3), 20).unwrap();

        let inner = reg.inner.lock().unwrap();
        assert_eq!(inner.map.len(), 2);
        assert!(inner.map.contains_key(&ClientKey {
            proxy: Some(p1),
            timeout_secs: 20
        }));
        assert!(inner.map.contains_key(&ClientKey {
            proxy: Some(p3),
            timeout_secs: 20
        }));
    }
}
