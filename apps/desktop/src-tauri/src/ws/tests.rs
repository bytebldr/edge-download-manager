use super::manager::WsManager;
use super::server::start_ws_server_on;
use super::types::*;
use crate::os::files::secure_open_file;
use edm_store::Store;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

async fn setup_test_server(
    token: &str,
) -> (
    SocketAddr,
    Arc<WsManager>,
    mpsc::Receiver<CreateTaskCommand>,
    mpsc::Receiver<ActionCommand>,
) {
    let store = Store::open_in_memory().unwrap();
    let (create_tx, create_rx) = mpsc::channel(32);
    let (action_tx, action_rx) = mpsc::channel(32);
    let (refresh_tx, _refresh_rx) = mpsc::channel(32);
    let (probe_tx, _probe_rx) = mpsc::channel(32);

    let manager = Arc::new(WsManager::new(
        token.to_string(),
        EXTENSION_ID.to_string(),
        store,
        create_tx,
        action_tx,
        refresh_tx,
        probe_tx,
    ));

    let addr = start_ws_server_on("127.0.0.1:0", manager.clone())
        .await
        .unwrap();
    (addr, manager, create_rx, action_rx)
}

#[tokio::test]
async fn test_bad_token_rejected() {
    let (addr, _mgr, _create_rx, _act_rx) =
        setup_test_server("correct_token_1234567890_abcdef12345").await;

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(format!("ws://{}", addr))
        .header("Origin", format!("chrome-extension://{}", EXTENSION_ID))
        .header("Host", addr.to_string())
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(())
        .unwrap();

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("Handshake should succeed");

    let hello = serde_json::to_string(&InboundWsMessage::hello("wrong_token_here")).unwrap();
    ws_stream.send(Message::Text(hello.into())).await.unwrap();

    let next_msg = ws_stream.next().await;
    match next_msg {
        None => {}
        Some(Ok(Message::Close(_))) => {}
        Some(Err(_)) => {}
        other => panic!("Expected connection closure on bad token, got {:?}", other),
    }
}

#[tokio::test]
async fn test_bad_origin_rejected() {
    let (addr, _mgr, _create_rx, _act_rx) =
        setup_test_server("token_1234567890_abcdef123456789012").await;

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(format!("ws://{}", addr))
        .header("Origin", "https://malicious-website.com")
        .header("Host", addr.to_string())
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(())
        .unwrap();

    let result = tokio_tungstenite::connect_async(request).await;
    assert!(
        result.is_err(),
        "Connection from malicious origin must be rejected"
    );
}

#[tokio::test]
async fn test_bad_host_rejected() {
    let (addr, _mgr, _create_rx, _act_rx) =
        setup_test_server("token_1234567890_abcdef123456789012").await;

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(format!("ws://{}", addr))
        .header("Origin", format!("chrome-extension://{}", EXTENSION_ID))
        .header("Host", "attacker.dnsrebinding.com")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(())
        .unwrap();

    let result = tokio_tungstenite::connect_async(request).await;
    assert!(result.is_err(), "Wrong host must be rejected");
}

#[tokio::test]
async fn test_token_endpoint_security() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let token = "super_secure_token_constant_time_987654";
    let (addr, _mgr, _create_rx, _act_rx) = setup_test_server(token).await;

    // 1. Untrusted External Web Origin -> 403
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(
            format!(
                "GET /token HTTP/1.1\r\nHost: {}\r\nOrigin: https://attacker.com\r\n\r\n",
                addr
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut resp = [0u8; 512];
    let n = stream.read(&mut resp).await.unwrap();
    let resp_str = String::from_utf8_lossy(&resp[..n]);
    assert!(
        resp_str.contains("403 Forbidden"),
        "External web origin must be 403"
    );

    // 2. Loopback HTTP Origin -> 403 (Token endpoint requires extension origin only)
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(
            format!(
                "GET /token HTTP/1.1\r\nHost: {}\r\nOrigin: http://127.0.0.1:3000\r\n\r\n",
                addr
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut resp = [0u8; 512];
    let n = stream.read(&mut resp).await.unwrap();
    let resp_str = String::from_utf8_lossy(&resp[..n]);
    assert!(
        resp_str.contains("403 Forbidden"),
        "Loopback origin must be rejected on /token"
    );

    // 3. Valid Chrome Extension Origin -> 200 OK
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let valid_origin = format!("chrome-extension://{}", EXTENSION_ID);
    stream
        .write_all(
            format!(
                "GET /token HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\n\r\n",
                addr, valid_origin
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut resp = [0u8; 1024];
    let n = stream.read(&mut resp).await.unwrap();
    let resp_str = String::from_utf8_lossy(&resp[..n]);
    assert!(
        resp_str.contains("200 OK"),
        "Valid extension origin must succeed"
    );
    assert!(resp_str.contains(token));

    // 4. Missing Origin (direct loopback / extension service worker fetch) -> 200 OK
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(format!("GET /token HTTP/1.1\r\nHost: {}\r\n\r\n", addr).as_bytes())
        .await
        .unwrap();
    let mut resp = [0u8; 1024];
    let n = stream.read(&mut resp).await.unwrap();
    let resp_str = String::from_utf8_lossy(&resp[..n]);
    assert!(
        resp_str.contains("200 OK"),
        "Direct loopback without origin must succeed"
    );
    assert!(resp_str.contains(token));
}

#[tokio::test]
async fn test_multiple_extension_ids_accepted_and_rejected() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let token = "test_multi_id_token_1234567890abcdef123";
    let (addr, _mgr, _create_rx, _act_rx) = setup_test_server(token).await;

    // 1. Verify every allowed Chromium ID origin succeeds on /token
    for id in edm_ipc::extensions::CHROMIUM_ALLOWED_IDS {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let origin = format!("chrome-extension://{}", id);
        stream
            .write_all(
                format!(
                    "GET /token HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\n\r\n",
                    addr, origin
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut resp = [0u8; 1024];
        let n = stream.read(&mut resp).await.unwrap();
        let resp_str = String::from_utf8_lossy(&resp[..n]);
        assert!(
            resp_str.contains("200 OK"),
            "Allowed Chromium ID origin {} must succeed",
            id
        );
        assert!(resp_str.contains(token));
    }

    // 2. Verify Firefox moz-extension origin succeeds on /token
    {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let origin = "moz-extension://b4b5e6fc-5231-419b-a010-86b2496a3fc4";
        stream
            .write_all(
                format!(
                    "GET /token HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\n\r\n",
                    addr, origin
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut resp = [0u8; 1024];
        let n = stream.read(&mut resp).await.unwrap();
        let resp_str = String::from_utf8_lossy(&resp[..n]);
        assert!(
            resp_str.contains("200 OK"),
            "Firefox moz-extension origin must succeed"
        );
        assert!(resp_str.contains(token));
    }

    // 3. Verify unknown/fake extension origin is rejected with 403 Forbidden
    {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let origin = "chrome-extension://unknownfakeextensionid123456";
        stream
            .write_all(
                format!(
                    "GET /token HTTP/1.1\r\nHost: {}\r\nOrigin: {}\r\n\r\n",
                    addr, origin
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut resp = [0u8; 512];
        let n = stream.read(&mut resp).await.unwrap();
        let resp_str = String::from_utf8_lossy(&resp[..n]);
        assert!(
            resp_str.contains("403 Forbidden"),
            "Unknown extension origin must be rejected with 403 Forbidden"
        );
    }
}

#[tokio::test]
async fn test_oversized_ws_message_rejected() {
    let token = "test_token_valid_1234567890abcdef123456";
    let (addr, _mgr, _create_rx, _act_rx) = setup_test_server(token).await;

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(format!("ws://{}", addr))
        .header("Origin", format!("chrome-extension://{}", EXTENSION_ID))
        .header("Host", addr.to_string())
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(())
        .unwrap();

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(request).await.unwrap();

    // Authenticate
    let hello = serde_json::to_string(&InboundWsMessage::hello(token)).unwrap();
    ws_stream.send(Message::Text(hello.into())).await.unwrap();
    let _ = ws_stream.next().await.unwrap().unwrap(); // Ready

    // Send oversized payload > 64 KiB
    let huge_text = "A".repeat(70 * 1024);
    let _ = ws_stream.send(Message::Text(huge_text.into())).await;

    // Connection should terminate
    let next_msg = ws_stream.next().await;
    match next_msg {
        None | Some(Ok(Message::Close(_))) | Some(Err(_)) => {}
        other => panic!(
            "Expected connection close on oversized message, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_create_task_ack() {
    let token = "test_token_valid_1234567890abcdef123456";
    let (addr, _mgr, mut create_rx, _act_rx) = setup_test_server(token).await;

    tokio::spawn(async move {
        if let Some(cmd) = create_rx.recv().await {
            let _ = cmd.resp_tx.send(Ok("task-uuid-abc-123".into()));
        }
    });

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(format!("ws://{}", addr))
        .header("Origin", format!("chrome-extension://{}", EXTENSION_ID))
        .header("Host", addr.to_string())
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(())
        .unwrap();

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(request).await.unwrap();

    let hello = serde_json::to_string(&InboundWsMessage::hello(token)).unwrap();
    ws_stream.send(Message::Text(hello.into())).await.unwrap();
    let _ = ws_stream.next().await.unwrap().unwrap();

    let create = serde_json::to_string(&InboundWsMessage::CreateTask {
        req_id: "req-999".into(),
        url: "https://example.com/file.zip".into(),
        cookies: vec![],
        referrer: None,
        user_agent: None,
        filename: Some("file.zip".into()),
    })
    .unwrap();
    ws_stream.send(Message::Text(create.into())).await.unwrap();

    let ack_msg = ws_stream.next().await.unwrap().unwrap();
    let ack_text = ack_msg.to_text().unwrap();
    let parsed: OutboundWsMessage = serde_json::from_str(ack_text).unwrap();
    match parsed {
        OutboundWsMessage::Ack { req_id, ok, id, .. } => {
            assert_eq!(req_id, "req-999");
            assert!(ok);
            assert_eq!(id, Some("task-uuid-abc-123".into()));
        }
        other => panic!("Expected Ack, got {:?}", other),
    }
}

#[tokio::test]
async fn test_open_file_refused_for_incomplete_tasks() {
    let temp_dir = std::env::temp_dir();

    let res1 = secure_open_file(&temp_dir, "../test.zip", "completed");
    assert!(res1.is_err());
    assert_eq!(res1.unwrap_err(), "Invalid file path");

    let res2 = secure_open_file(&temp_dir, "non_existent_file_xyz_123.bin", "completed");
    assert!(res2.is_err());
    assert_eq!(res2.unwrap_err(), "File does not exist on disk");

    let res3 = secure_open_file(&temp_dir, "test.zip", "paused");
    assert!(res3.is_err());
    assert_eq!(
        res3.unwrap_err(),
        "Cannot open file: download is not completed"
    );
}

#[tokio::test]
async fn test_unknown_task_id_ignored() {
    let token = "test_token_valid_1234567890abcdef123456";
    let (addr, _mgr, _create_rx, mut act_rx) = setup_test_server(token).await;

    tokio::spawn(async move {
        while let Some(cmd) = act_rx.recv().await {
            let _ = cmd.resp_tx.send(Err("Task not found".into()));
        }
    });

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(format!("ws://{}", addr))
        .header("Origin", format!("chrome-extension://{}", EXTENSION_ID))
        .header("Host", addr.to_string())
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(())
        .unwrap();

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(request).await.unwrap();

    let hello = serde_json::to_string(&InboundWsMessage::hello(token)).unwrap();
    ws_stream.send(Message::Text(hello.into())).await.unwrap();
    let _ = ws_stream.next().await.unwrap().unwrap();

    let act = serde_json::to_string(&InboundWsMessage::TaskAction {
        req_id: None,
        id: "unknown-uuid-999".into(),
        action: "pause".into(),
        delete_file: None,
    })
    .unwrap();
    ws_stream.send(Message::Text(act.into())).await.unwrap();

    ws_stream
        .send(Message::Text("{\"type\":\"ping\"}".into()))
        .await
        .unwrap();
    let pong = ws_stream.next().await.unwrap().unwrap();
    assert!(pong.to_text().unwrap().contains("\"type\":\"pong\""));
}
