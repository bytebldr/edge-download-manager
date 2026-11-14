//! Host Resilient TCP & WebSocket Listener for EDM Handoff.
//!
//! Enforces an immortal retry loop on Windows loopback sockets, preventing
//! transient connection resets (WSAECONNRESET / ECONNABORTED) from terminating
//! the listener and breaking browser extension handoffs.

use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

pub const HANDOFF_PORT: u16 = 14370;
pub const LOOPBACK_HOST: &str = "127.0.0.1";

/// Binds the TCP listener to 127.0.0.1:14370 with retries to handle socket TIME_WAIT.
pub async fn bind_handoff_listener() -> std::io::Result<TcpListener> {
    let addr = SocketAddr::from(([127, 0, 0, 1], HANDOFF_PORT));
    info!("[Handoff] Binding immortal listener on {}", addr);
    let mut retry_count = 0;
    loop {
        match TcpListener::bind(addr).await {
            Ok(l) => return Ok(l),
            Err(e) => {
                retry_count += 1;
                if retry_count > 40 {
                    return Err(e);
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
    }
}

/// Runs the immortal accept loop. Never terminates on transient stream errors.
pub async fn run_accept_loop<F, Fut>(listener: TcpListener, handler: F)
where
    F: Fn(TcpStream, SocketAddr) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let handler = std::sync::Arc::new(handler);
    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                // Only allow loopback traffic
                if !peer_addr.ip().is_loopback() {
                    warn!("[Handoff] Dropping non-loopback connection attempt from {}", peer_addr);
                    drop(stream);
                    continue;
                }
                let handler_clone = handler.clone();
                tokio::spawn(async move {
                    handler_clone(stream, peer_addr).await;
                });
            }
            Err(e) => {
                // CRITICAL FIX: Do NOT exit the loop on accept errors.
                // Transient Windows socket resets must be logged and backed off.
                warn!("[Handoff] Listener transient error on accept: {:?}. Backing off 100ms...", e);
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        }
    }
}
