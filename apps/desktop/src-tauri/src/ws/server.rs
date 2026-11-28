use super::dispatch::handle_ws_connection;
use super::http::handle_http_token_request;
use super::manager::WsManager;
use super::types::{ACTIVE_CONNECTIONS, MAX_CONCURRENT_CONNECTIONS, WS_BIND_ADDR};
#[cfg(test)]
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{info, warn};

struct ConnectionGuard;

impl ConnectionGuard {
    fn try_acquire() -> Option<Self> {
        ACTIVE_CONNECTIONS
            .try_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                if current < MAX_CONCURRENT_CONNECTIONS {
                    Some(current + 1)
                } else {
                    None
                }
            })
            .ok()
            .map(|_| Self)
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

fn spawn_accept_loop(listener: TcpListener, manager: Arc<WsManager>) {
    tokio::spawn(async move {
        loop {
            let (stream, peer_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!(
                        "WebSocket listener accept error: {}. Retrying in 50ms...",
                        e
                    );
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            };

            let Some(guard) = ConnectionGuard::try_acquire() else {
                warn!(
                    "Rejecting connection from {}: max concurrent connections reached ({})",
                    peer_addr, MAX_CONCURRENT_CONNECTIONS
                );
                drop(stream);
                continue;
            };

            let mgr = manager.clone();
            tokio::spawn(async move {
                let _conn_guard = guard;
                let mut peek_buf = [0u8; 1024];
                let mut peek_len = 0;
                for _ in 0..20 {
                    peek_len = stream.peek(&mut peek_buf).await.unwrap_or(0);
                    if peek_len > 0 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                if peek_len == 0 {
                    return;
                }

                let peek_str = String::from_utf8_lossy(&peek_buf[..peek_len]);

                if peek_str.starts_with("GET /token")
                    || peek_str.starts_with("OPTIONS /token")
                    || peek_str.starts_with("GET /firefox.xpi")
                {
                    handle_http_token_request(stream, peer_addr, mgr).await;
                } else {
                    handle_ws_connection(stream, peer_addr, mgr).await;
                }
            });
        }
    });
}

pub async fn start_ws_server(manager: Arc<WsManager>) {
    let addr = WS_BIND_ADDR;
    let mut retry_count = 0;
    let listener = loop {
        match TcpListener::bind(addr).await {
            Ok(l) => {
                info!(
                    "EDM WebSocket & HTTP Token Server listening on ws://{}",
                    addr
                );
                break l;
            }
            Err(e) => {
                retry_count += 1;
                if retry_count % 10 == 1 {
                    warn!(
                        "WebSocket server bind to {} failed (attempt {}): {}. Retrying in 250ms...",
                        addr, retry_count, e
                    );
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
    };

    let ticker_mgr = manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        let mut last_serialized = String::new();
        loop {
            interval.tick().await;
            if ticker_mgr.tx_broadcast.receiver_count() == 0 {
                continue;
            }
            let snapshot = ticker_mgr.build_snapshot().await;
            if let Ok(json) = serde_json::to_string(&snapshot) {
                if json != last_serialized {
                    last_serialized = json.clone();
                    let _ = ticker_mgr.tx_broadcast.send(json);
                }
            }
        }
    });

    spawn_accept_loop(listener, manager);
}

#[cfg(test)]
pub async fn start_ws_server_on(
    bind_addr: &str,
    manager: Arc<WsManager>,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(bind_addr).await?;
    let local_addr = listener.local_addr()?;
    spawn_accept_loop(listener, manager);
    Ok(local_addr)
}
