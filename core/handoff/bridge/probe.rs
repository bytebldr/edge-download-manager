//! Health probe for port 14370 with exponential backoff.
//!
//! Provides zero-hang probing against loopback 127.0.0.1:14370 to verify
//! that EDM.exe is ready to accept WebSocket & HTTP handoffs.

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::sleep;
use tracing::{debug, info, warn};

pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// Probes loopback port 14370 until it accepts a TCP connection or times out.
pub async fn probe_port_ready(port: u16, timeout: Duration) -> Result<Duration, String> {
    let start = Instant::now();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut delay_ms = 50u64;

    while start.elapsed() < timeout {
        match tokio::time::timeout(Duration::from_millis(150), TcpStream::connect(addr)).await {
            Ok(Ok(_stream)) => {
                let elapsed = start.elapsed();
                info!("[Handoff Probe] Port {} is READY after {:?}", port, elapsed);
                return Ok(elapsed);
            }
            Ok(Err(_)) | Err(_) => {
                debug!("[Handoff Probe] Port {} not ready yet, sleeping {}ms", port, delay_ms);
                sleep(Duration::from_millis(delay_ms)).await;
                // Exponential backoff capped at 300ms
                delay_ms = std::cmp::min(delay_ms.saturating_mul(2), 300);
            }
        }
    }

    warn!("[Handoff Probe] Timed out waiting for port {} after {:?}", port, timeout);
    Err(format!("Timeout waiting for EDM desktop to initialize on port {}", port))
}
