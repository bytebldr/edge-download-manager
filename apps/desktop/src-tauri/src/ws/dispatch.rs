use super::manager::{is_valid_loopback_host, WsManager};
use super::types::*;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, warn};
use uuid::Uuid;

pub async fn handle_ws_connection(stream: TcpStream, peer_addr: SocketAddr, manager: Arc<WsManager>) {
    debug!("Incoming WebSocket connection from {}", peer_addr);

    let mut origin_valid = false;
    let _expected_origin = format!("chrome-extension://{}", manager.allowed_extension_id);
    let port = stream.local_addr().map(|a| a.port()).unwrap_or(WS_PORT);

    #[allow(clippy::result_large_err)]
    let callback = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        // Guard 1: Host header validation
        let host_val = req.headers().get("Host").and_then(|h| h.to_str().ok());
        if !is_valid_loopback_host(host_val, port) {
            let err_resp = tokio_tungstenite::tungstenite::http::Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Some("Invalid Host Header".to_string()))
                .unwrap();
            return Err(err_resp);
        }

        // Guard 2: Origin header validation
        if let Some(origin) = req.headers().get("Origin") {
            if let Ok(origin_str) = origin.to_str() {
                if edm_ipc::extensions::is_allowed_origin(origin_str) {
                    origin_valid = true;
                    return Ok(resp);
                }
            }
        }
        let err_resp = tokio_tungstenite::tungstenite::http::Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Some("Forbidden Origin".to_string()))
            .unwrap();
        Err(err_resp)
    };

    let ws_stream = match tokio_tungstenite::accept_hdr_async(stream, callback).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!("Rejected WebSocket handshake from {}: {}", peer_addr, e);
            return;
        }
    };

    if !origin_valid {
        warn!("Forbidden origin on WebSocket from {}", peer_addr);
        return;
    }

    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Enforce first message MUST be: hello {token} within 5 seconds
    let hello_result = tokio::time::timeout(Duration::from_secs(5), ws_rx.next()).await;
    let (authenticated, client_browser, client_version, client_ext_id) = match hello_result {
        Ok(Some(Ok(Message::Text(text)))) => {
            if text.len() > MAX_WS_MESSAGE_SIZE {
                (false, None, None, None)
            } else if let Ok(InboundWsMessage::Hello {
                token,
                browser,
                version,
                extension_id,
            }) = serde_json::from_str::<InboundWsMessage>(&text)
            {
                let token_ok = manager.verify_token_constant_time(&token);
                let ext_ok = match extension_id.as_deref() {
                    Some(id) => edm_ipc::extensions::is_allowed_extension_id(id),
                    None => true, // backward compatible for legacy extension builds
                };
                let ok = token_ok && ext_ok;
                (ok, browser, version, extension_id)
            } else {
                (false, None, None, None)
            }
        }
        _ => (false, None, None, None),
    };

    if !authenticated {
        warn!(
            "WebSocket authentication failed from {}. Closing socket immediately.",
            peer_addr
        );
        let _ = ws_tx.close().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        return;
    }

    debug!("WebSocket authenticated successfully from {}", peer_addr);

    let client_id = Uuid::new_v4().to_string();
    let norm_browser = match client_browser.as_deref() {
        Some(b) if b.to_lowercase().contains("edge") || b.to_lowercase().contains("edg") => "edge",
        Some(b) if b.to_lowercase().contains("brave") => "brave",
        Some(b) if b.to_lowercase().contains("firefox") => "firefox",
        Some(b) if b.to_lowercase().contains("chrome") => "chrome",
        Some(b) => b,
        None => "chrome",
    }
    .to_string();

    let client_info = ConnectedClientInfo {
        client_id: client_id.clone(),
        browser: norm_browser,
        version: client_version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
        extension_id: client_ext_id,
        connected_at_secs: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    manager.connected_clients.write().await.insert(client_id.clone(), client_info);

    // Reply with ready containing current app/extension version
    let ready_json = serde_json::to_string(&OutboundWsMessage::Ready {
        extension_version: Some(env!("CARGO_PKG_VERSION").to_string()),
    })
    .unwrap();
    if ws_tx.send(Message::Text(ready_json.into())).await.is_err() {
        manager.connected_clients.write().await.remove(&client_id);
        return;
    }

    let mut rx_broadcast = manager.tx_broadcast.subscribe();
    let mut is_subscribed = false;

    loop {
        tokio::select! {
            // Outbound broadcast
            msg_res = rx_broadcast.recv(), if is_subscribed => {
                match msg_res {
                    Ok(json) => {
                        if ws_tx.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("WebSocket client lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }

            // Inbound messages from extension
            ws_msg = ws_rx.next() => {
                match ws_msg {
                    Some(Ok(Message::Text(text))) => {
                        if text.len() > MAX_WS_MESSAGE_SIZE {
                            warn!("Oversized WebSocket message received ({} bytes) from {}", text.len(), peer_addr);
                            break;
                        }
                        match serde_json::from_str::<InboundWsMessage>(&text) {
                            Ok(InboundWsMessage::Ping) => {
                                let pong = serde_json::to_string(&OutboundWsMessage::Pong).unwrap();
                                if ws_tx.send(Message::Text(pong.into())).await.is_err() {
                                    break;
                                }
                            }
                            Ok(InboundWsMessage::Subscribe) => {
                                is_subscribed = true;
                                let snapshot = manager.build_snapshot().await;
                                if let Ok(json) = serde_json::to_string(&snapshot) {
                                    if ws_tx.send(Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Ok(InboundWsMessage::CreateTask { req_id, url, cookies, referrer, user_agent, filename }) => {
                                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                                let cmd = CreateTaskCommand {
                                    req_id: req_id.clone(),
                                    url,
                                    cookies,
                                    referrer,
                                    user_agent,
                                    filename,
                                    resp_tx,
                                };

                                let ack = if manager.tx_create_task.send(cmd).await.is_ok() {
                                    match tokio::time::timeout(Duration::from_secs(4), resp_rx).await {
                                        Ok(Ok(Ok(task_id))) => OutboundWsMessage::Ack {
                                            req_id,
                                            ok: true,
                                            id: Some(task_id),
                                            error: None,
                                        },
                                        Ok(Ok(Err(err))) => OutboundWsMessage::Ack {
                                            req_id,
                                            ok: false,
                                            id: None,
                                            error: Some(err),
                                        },
                                        _ => OutboundWsMessage::Ack {
                                            req_id,
                                            ok: false,
                                            id: None,
                                            error: Some("Timeout creating download task".into()),
                                        },
                                    }
                                } else {
                                    OutboundWsMessage::Ack {
                                        req_id,
                                        ok: false,
                                        id: None,
                                        error: Some("Internal task dispatcher unavailable".into()),
                                    }
                                };

                                if let Ok(json) = serde_json::to_string(&ack) {
                                    if ws_tx.send(Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Ok(InboundWsMessage::TaskAction { req_id, id, action, delete_file }) => {
                                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                                let cmd = ActionCommand {
                                    req_id: req_id.clone(),
                                    id,
                                    action,
                                    delete_file: delete_file.unwrap_or(false),
                                    resp_tx,
                                };
                                let ack = if manager.tx_action.send(cmd).await.is_ok() {
                                    match tokio::time::timeout(Duration::from_secs(3), resp_rx).await {
                                        Ok(Ok(Ok(()))) => {
                                            req_id.map(|rid| OutboundWsMessage::Ack {
                                                req_id: rid,
                                                ok: true,
                                                id: None,
                                                error: None,
                                            })
                                        }
                                        Ok(Ok(Err(err))) => {
                                            req_id.map(|rid| OutboundWsMessage::Ack {
                                                req_id: rid,
                                                ok: false,
                                                id: None,
                                                error: Some(err),
                                            })
                                        }
                                        _ => {
                                            req_id.map(|rid| OutboundWsMessage::Ack {
                                                req_id: rid,
                                                ok: false,
                                                id: None,
                                                error: Some("Timeout executing task action".into()),
                                            })
                                        }
                                    }
                                } else {
                                    req_id.map(|rid| OutboundWsMessage::Ack {
                                        req_id: rid,
                                        ok: false,
                                        id: None,
                                        error: Some("Internal task dispatcher unavailable".into()),
                                    })
                                };

                                if let Some(ack_msg) = ack {
                                    if let Ok(json) = serde_json::to_string(&ack_msg) {
                                        let _ = ws_tx.send(Message::Text(json.into())).await;
                                    }
                                }
                            }
                            Ok(InboundWsMessage::RefreshTask { req_id, task_id, new_url, headers }) => {
                                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                                let cmd = RefreshTaskCommand {
                                    req_id: req_id.clone(),
                                    task_id,
                                    new_url,
                                    headers,
                                    resp_tx,
                                };
                                let ack = if manager.tx_refresh_task.send(cmd).await.is_ok() {
                                    match tokio::time::timeout(Duration::from_secs(15), resp_rx).await {
                                        Ok(Ok(Ok(()))) => OutboundWsMessage::Ack {
                                            req_id: req_id.unwrap_or_default(),
                                            ok: true,
                                            id: None,
                                            error: None,
                                        },
                                        Ok(Ok(Err(err))) => OutboundWsMessage::Ack {
                                            req_id: req_id.unwrap_or_default(),
                                            ok: false,
                                            id: None,
                                            error: Some(err),
                                        },
                                        _ => OutboundWsMessage::Ack {
                                            req_id: req_id.unwrap_or_default(),
                                            ok: false,
                                            id: None,
                                            error: Some("Timeout verifying and refreshing link".into()),
                                        },
                                    }
                                } else {
                                    OutboundWsMessage::Ack {
                                        req_id: req_id.unwrap_or_default(),
                                        ok: false,
                                        id: None,
                                        error: Some("Task refresh service unavailable".into()),
                                    }
                                };
                                if let Ok(json) = serde_json::to_string(&ack) {
                                    let _ = ws_tx.send(Message::Text(json.into())).await;
                                }
                            }
                            Ok(InboundWsMessage::ProbeMedia { req_id, media_url }) => {
                                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                                let cmd = ProbeMediaCommand {
                                    req_id: req_id.clone(),
                                    media_url,
                                    resp_tx,
                                };
                                if manager.tx_probe_media.send(cmd).await.is_ok() {
                                    if let Ok(Ok(Ok((variants, is_drm)))) = tokio::time::timeout(Duration::from_secs(10), resp_rx).await {
                                        let reply = OutboundWsMessage::MediaVariants {
                                            req_id,
                                            variants,
                                            is_drm,
                                        };
                                        if let Ok(json) = serde_json::to_string(&reply) {
                                            let _ = ws_tx.send(Message::Text(json.into())).await;
                                        }
                                    }
                                }
                            }
                            Ok(InboundWsMessage::BatchAdd { req_id, items }) => {
                                let mut added = 0;
                                for item in items {
                                    let (resp_tx, _resp_rx) = tokio::sync::oneshot::channel();
                                    let cmd = CreateTaskCommand {
                                        req_id: format!("{}_{}", req_id, added),
                                        url: item.url,
                                        cookies: Vec::new(),
                                        referrer: item.referrer,
                                        user_agent: None,
                                        filename: item.filename,
                                        resp_tx,
                                    };
                                    let _ = manager.tx_create_task.send(cmd).await;
                                    added += 1;
                                }
                                let ack = OutboundWsMessage::Ack {
                                    req_id,
                                    ok: true,
                                    id: Some(format!("Added {} tasks", added)),
                                    error: None,
                                };
                                if let Ok(json) = serde_json::to_string(&ack) {
                                    let _ = ws_tx.send(Message::Text(json.into())).await;
                                }
                            }
                            Ok(InboundWsMessage::Hello { .. }) => {
                                // Redundant hello ignored
                            }
                            Err(e) => {
                                warn!("Malformed JSON from extension {}: {}", peer_addr, e);
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if ws_tx.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    manager.connected_clients.write().await.remove(&client_id);
    debug!("WebSocket connection closed for {}", peer_addr);
}
