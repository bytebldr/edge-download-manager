use super::dispatch::handle_ws_connection;
use super::http::handle_http_token_request;
use super::manager::WsManager;
use super::types::{ACTIVE_CONNECTIONS, MAX_CONCURRENT_CONNECTIONS, WS_BIND_ADDR};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

pub async fn start_ws_server(manager: Arc<WsManager>) {
    let addr = WS_BIND_ADDR;
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => {
            info!(
                "EDM WebSocket & HTTP Token Server listening on ws://{}",
                addr
            );
            l
        }
        Err(e) => {
            error!("Failed to bind WebSocket server to {}: {}", addr, e);
            return;
        }
    };

    let ticker_mgr = manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        let mut last_serialized = String::new();
        loop {
            interval.tick().await;
            let snapshot = ticker_mgr.build_snapshot().await;
            if let Ok(json) = serde_json::to_string(&snapshot) {
                if json != last_serialized {
                    last_serialized = json.clone();
                    let _ = ticker_mgr.tx_broadcast.send(json);
                }
            }
        }
    });

    let srv_mgr = manager.clone();
    tokio::spawn(async move {
        while let Ok((stream, peer_addr)) = listener.accept().await {
            let active = ACTIVE_CONNECTIONS.load(Ordering::Relaxed);
            if active >= MAX_CONCURRENT_CONNECTIONS {
                warn!(
                    "Rejecting connection from {}: max concurrent connections reached ({})",
                    peer_addr, active
                );
                drop(stream);
                continue;
            }

            let mgr = srv_mgr.clone();
            tokio::spawn(async move {
                ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
                let mut peek_buf = [0u8; 1024];
                let mut peek_len = 0;
                for _ in 0..20 {
                    peek_len = stream.peek(&mut peek_buf).await.unwrap_or(0);
                    if peek_len > 0 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
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
                ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
            });
        }
    });
}

#[allow(dead_code)]
pub async fn start_ws_server_on(
    bind_addr: &str,
    manager: Arc<WsManager>,
) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(bind_addr).await?;
    let local_addr = listener.local_addr()?;
    let srv_mgr = manager.clone();

    tokio::spawn(async move {
        while let Ok((stream, peer_addr)) = listener.accept().await {
            let active = ACTIVE_CONNECTIONS.load(Ordering::Relaxed);
            if active >= MAX_CONCURRENT_CONNECTIONS {
                drop(stream);
                continue;
            }
            let mgr = srv_mgr.clone();
            tokio::spawn(async move {
                ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
                let mut peek_buf = [0u8; 1024];
                let mut peek_len = 0;
                for _ in 0..20 {
                    peek_len = stream.peek(&mut peek_buf).await.unwrap_or(0);
                    if peek_len > 0 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
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
                ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
            });
        }
    });

    Ok(local_addr)
}
