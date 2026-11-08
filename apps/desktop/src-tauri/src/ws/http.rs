use super::manager::{is_valid_loopback_host, WsManager};
use super::types::WS_PORT;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::warn;

pub async fn handle_http_token_request(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    manager: Arc<WsManager>,
) {
    let mut buf = [0u8; 2048];
    let n = match stream.read(&mut buf).await {
        Ok(read_bytes) if read_bytes > 0 => read_bytes,
        _ => return,
    };
    let request_str = String::from_utf8_lossy(&buf[..n]);

    let mut origin: Option<String> = None;
    let mut host: Option<String> = None;
    let mut ext_id_hdr: Option<String> = None;
    for line in request_str.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("origin:") {
            origin = Some(line[7..].trim().trim_end_matches('/').to_string());
        } else if lower.starts_with("host:") {
            host = Some(line[5..].trim().to_string());
        } else if lower.starts_with("x-edm-extension-id:") {
            ext_id_hdr = Some(line[19..].trim().to_string());
        }
    }

    // Guard 1: DNS rebinding defense on Host header
    let port = stream.local_addr().map(|a| a.port()).unwrap_or(WS_PORT);
    if !is_valid_loopback_host(host.as_deref(), port) {
        warn!(
            "Invalid Host header on /token from {}: {:?}",
            peer_addr, host
        );
        let resp = "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nInvalid Host Header";
        let _ = stream.write_all(resp.as_bytes()).await;
        return;
    }

    // Handle Firefox XPI direct install endpoint
    if request_str.starts_with("GET /firefox.xpi") {
        let xpi_candidates = [
            std::path::PathBuf::from("extension/dist/firefox/edm-firefox.xpi"),
            std::path::PathBuf::from("resources/firefox.xpi"),
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("resources").join("firefox.xpi")))
                .unwrap_or_default(),
        ];
        for candidate in &xpi_candidates {
            if candidate.exists() {
                if let Ok(bytes) = tokio::fs::read(candidate).await {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/x-xpinstall\r\nContent-Disposition: attachment; filename=\"edm-firefox.xpi\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                    let _ = stream.write_all(&bytes).await;
                    return;
                }
            }
        }
        let redirect_url =
            "https://github.com/bytebldr/edge-download-manager/releases/latest/download/edm-firefox.xpi";
        let resp = format!(
            "HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nRedirecting to Firefox XPI",
            redirect_url
        );
        let _ = stream.write_all(resp.as_bytes()).await;
        return;
    }

    // Guard 2: Strict Origin & Extension Identity Requirement
    // Only verified extension callers or direct loopback service workers are allowed to read the authentication token.
    let is_forbidden = if let Some(ref o) = origin {
        let trimmed = o.trim().trim_end_matches('/');
        if let Some(rest) = trimmed.strip_prefix("chrome-extension://") {
            let ext_id = rest.split('/').next().unwrap_or(rest);
            !edm_ipc::extensions::CHROMIUM_ALLOWED_IDS.contains(&ext_id)
        } else if trimmed.starts_with("moz-extension://") {
            // For Firefox moz-extension, if X-EDM-Extension-Id header is provided, verify it matches
            if let Some(ref id) = ext_id_hdr {
                id != edm_ipc::extensions::FIREFOX_GECKO_ID
            } else {
                false
            }
        } else {
            true
        }
    } else {
        // Direct loopback or extension background service worker without origin header
        false
    };

    let ac_origin = if let Some(ref o) = origin {
        if !is_forbidden {
            o.as_str()
        } else {
            "null"
        }
    } else {
        ""
    };

    if request_str.starts_with("OPTIONS") {
        let resp = format!(
            "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nAccess-Control-Max-Age: 86400\r\nConnection: close\r\n\r\n",
            if !is_forbidden && !ac_origin.is_empty() { ac_origin } else { "null" }
        );
        let _ = stream.write_all(resp.as_bytes()).await;
        return;
    }

    if is_forbidden {
        warn!(
            "Forbidden origin '{:?}' on HTTP /token request from {}",
            origin, peer_addr
        );
        let resp = "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nForbidden Origin";
        let _ = stream.write_all(resp.as_bytes()).await;
        return;
    }

    let body = serde_json::json!({
        "token": manager.get_token(),
        "extension_id": manager.allowed_extension_id
    })
    .to_string();

    let mut resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    );
    if !ac_origin.is_empty() {
        resp.push_str(&format!("Access-Control-Allow-Origin: {}\r\n", ac_origin));
    }
    resp.push_str("Connection: close\r\n\r\n");
    resp.push_str(&body);
    let _ = stream.write_all(resp.as_bytes()).await;
}
