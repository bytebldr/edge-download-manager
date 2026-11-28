use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use edm_core::adaptive::{AdaptiveConfig, AdaptiveDownloadTask, AdaptiveEngine, AdaptiveTelemetry};
use edm_core::packs::PackExecutor;
use edm_store::Store;
use futures_util::stream;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Clone)]
struct ChaosState {
    active_conns: Arc<AtomicU32>,
    peak_conns: Arc<AtomicU32>,
    total_requests: Arc<AtomicU64>,
    cached_digests: Arc<std::sync::Mutex<HashMap<(u64, u64), String>>>,
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = *chunk.get(1).unwrap_or(&0) as usize;
        let b2 = *chunk.get(2).unwrap_or(&0) as usize;
        out.push(TABLE[b0 >> 2] as char);
        out.push(TABLE[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[b2 & 0x3f] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn deterministic_byte(idx: u64, seed: u64) -> u8 {
    ((idx.wrapping_mul(1337).wrapping_add(seed)) % 256) as u8
}

fn expected_sha256_hex(len: u64, seed: u64) -> String {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut off = 0u64;
    while off < len {
        let take = ((len - off) as usize).min(buf.len());
        let base = (off ^ seed) as u8;
        for (i, b) in buf[..take].iter_mut().enumerate() {
            *b = base.wrapping_add((i & 0xff) as u8);
        }
        hasher.update(&buf[..take]);
        off += take as u64;
    }
    hex::encode(hasher.finalize())
}

fn actual_file_sha256(path: &std::path::Path) -> String {
    let data = std::fs::read(path).expect("read file for sha256");
    let mut hasher = Sha256::new();
    hasher.update(&data);
    hex::encode(hasher.finalize())
}

async fn handle_throttled_file(
    State(st): State<ChaosState>,
    AxumPath(size): AxumPath<u64>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let seed: u64 = params.get("seed").and_then(|s| s.parse().ok()).unwrap_or(7);
    let per_conn_bps: u64 = params
        .get("throttle_bps")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let ttfb_ms: u64 = params
        .get("ttfb_ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let force_gzip = params.get("gzip").map(|v| v == "1").unwrap_or(false);
    let expire_after = params
        .get("expire_after_reqs")
        .and_then(|s| s.parse::<u64>().ok());

    let req_num = st.total_requests.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(limit) = expire_after {
        if req_num > limit && headers.contains_key(header::RANGE) {
            // Only expire once some range requests succeeded
            if let Some(r) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
                if r != "bytes=0-0" {
                    return Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .body(Body::from("Expired token"))
                        .unwrap();
                }
            }
        }
    }

    if ttfb_ms > 0 {
        tokio::time::sleep(Duration::from_millis(ttfb_ms)).await;
    }

    let range_hdr = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let (start, end, status) = if let Some(r) = range_hdr {
        if let Some(spec) = r.strip_prefix("bytes=") {
            let parts: Vec<&str> = spec.split('-').collect();
            let s = parts[0].parse::<u64>().unwrap_or(0);
            let e = parts
                .get(1)
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or(size - 1)
                .min(size - 1);
            (s, e, StatusCode::PARTIAL_CONTENT)
        } else {
            (0, size - 1, StatusCode::OK)
        }
    } else {
        (0, size - 1, StatusCode::OK)
    };

    let chunk_len = end.saturating_sub(start).saturating_add(1);
    let repr_digest = {
        let mut cache = st.cached_digests.lock().unwrap();
        cache
            .entry((size, seed))
            .or_insert_with(|| {
                let digest_hex = expected_sha256_hex(size, seed);
                let digest_bytes = hex::decode(&digest_hex).unwrap();
                format!("sha-256=:{}:", encode_base64(&digest_bytes))
            })
            .clone()
    };

    let cur = st.active_conns.fetch_add(1, Ordering::SeqCst) + 1;
    st.peak_conns.fetch_max(cur, Ordering::SeqCst);

    struct ConnGuard(Arc<AtomicU32>);
    impl Drop for ConnGuard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let guard = Arc::new(ConnGuard(st.active_conns.clone()));

    let stream = stream::unfold((start, end, guard), move |(pos, end, guard)| async move {
        if pos > end {
            drop(guard);
            return None;
        }
        let step = 64 * 1024u64;
        let next_end = (pos + step - 1).min(end);
        let len = (next_end - pos + 1) as usize;
        // Fast deterministic fill so debug-build CPU doesn't bottleneck 32-stream network throttle
        let mut buf = vec![0u8; len];
        let base = (pos ^ seed) as u8;
        for (i, b) in buf.iter_mut().enumerate() {
            *b = base.wrapping_add((i & 0xff) as u8);
        }
        if per_conn_bps > 0 && len > 1 {
            let delay_ms = (len as u64 * 1000) / per_conn_bps.max(1);
            if delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
        Some((
            Ok::<_, std::io::Error>(axum::body::Bytes::from(buf)),
            (next_end + 1, end, guard),
        ))
    });

    let mut builder = Response::builder()
        .status(status)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, chunk_len.to_string())
        .header(header::ETAG, format!("\"v014-seed-{}\"", seed))
        .header("repr-digest", repr_digest);

    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end, size),
        );
    }
    if force_gzip {
        builder = builder.header(header::CONTENT_ENCODING, "gzip");
    }

    builder.body(Body::from_stream(stream)).unwrap()
}

async fn handle_m3u8_playlist(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let count: usize = params.get("n").and_then(|s| s.parse().ok()).unwrap_or(32);
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("127.0.0.1");
    let mut body = String::from("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:2\n");
    for i in 0..count {
        body.push_str(&format!("#EXTINF:1.0,\nhttp://{}/hls/seg/{}.ts\n", host, i));
    }
    body.push_str("#EXT-X-ENDLIST\n");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .body(Body::from(body))
        .unwrap()
}

async fn handle_hls_segment(AxumPath(seg): AxumPath<String>) -> Response {
    tokio::time::sleep(Duration::from_millis(120)).await;
    let idx: u8 = seg
        .trim_end_matches(".ts")
        .parse::<u64>()
        .unwrap_or(0) as u8;
    // 1 KiB payload tagged with segment index
    let mut data = vec![0x47u8; 1024]; // MPEG-TS sync byte 0x47
    data[1] = idx;
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp2t")
        .body(Body::from(data))
        .unwrap()
}

async fn spawn_v014_server() -> (SocketAddr, ChaosState, tokio::task::JoinHandle<()>) {
    let state = ChaosState {
        active_conns: Arc::new(AtomicU32::new(0)),
        peak_conns: Arc::new(AtomicU32::new(0)),
        total_requests: Arc::new(AtomicU64::new(0)),
        cached_digests: Arc::new(std::sync::Mutex::new(HashMap::new())),
    };
    let app = Router::new()
        .route("/file/{size}", get(handle_throttled_file))
        .route("/hls/playlist.m3u8", get(handle_m3u8_playlist))
        .route("/hls/seg/{seg}", get(handle_hls_segment))
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (addr, state, handle)
}

#[tokio::test]
async fn test_v014_32_parallel_saturation_and_speedup() {
    let (addr, state, _srv) = spawn_v014_server().await;
    let file_size: u64 = 48 * 1024 * 1024; // 48 MiB
    // Per-socket throttle 512 KiB/s + 15ms TTFB
    let url = format!(
        "http://{}/file/{}?seed=42&throttle_bps=524288&ttfb_ms=15",
        addr, file_size
    );

    let dir = tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    let engine = AdaptiveEngine::new(store);

    let max_reported_workers = Arc::new(AtomicU32::new(0));
    let max_rep_clone = max_reported_workers.clone();
    let cb = Arc::new(move |t: AdaptiveTelemetry| {
        max_rep_clone.fetch_max(t.active_connections, Ordering::Relaxed);
    });

    let task = AdaptiveDownloadTask {
        id: Uuid::new_v4(),
        url,
        filename: "sat32.bin".into(),
        save_dir: dir.path().to_path_buf(),
        config: AdaptiveConfig {
            initial_connections: 8,
            min_connections: 1,
            max_connections: 32,
            max_connections_per_host: 32,
            min_chunk_size: 256 * 1024,
            cooldown_duration: Duration::from_millis(600),
            ..Default::default()
        },
        cancelled: Arc::new(AtomicBool::new(false)),
        ..Default::default()
    };

    let t0 = Instant::now();
    let out = engine.download(task, Some(cb)).await.expect("32-stream download failed");
    let elapsed = t0.elapsed();
    let speed_mbs = (file_size as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64();
    let peak = state.peak_conns.load(Ordering::SeqCst);

    println!(
        "v0.1.4 32-parallel result: {:.2} MiB in {:?} ({:.2} MiB/s), server peak_conns={}",
        file_size as f64 / (1024.0 * 1024.0),
        elapsed,
        speed_mbs,
        peak
    );

    assert!(
        peak == 32,
        "Expected server peak_conns == 32, got {}",
        peak
    );
    assert!(
        speed_mbs >= 3.5,
        "Expected >= 3.5 MiB/s (>= 7x single-stream in debug build) under 512 KiB/s per-socket throttle, got {:.2} MiB/s",
        speed_mbs
    );
    assert_eq!(
        actual_file_sha256(&out),
        expected_sha256_hex(file_size, 42)
    );
}

#[tokio::test]
async fn test_v014_pre_rename_hash_quarantine_and_repr_digest() {
    let (addr, _state, _srv) = spawn_v014_server().await;
    let file_size: u64 = 1024 * 1024;
    let url = format!("http://{}/file/{}?seed=11", addr, file_size);

    let dir = tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    let engine = AdaptiveEngine::new(store);

    // 1. Wrong expected_hash -> must quarantine to .edmpart.corrupt and NOT expose final file
    let bad_task = AdaptiveDownloadTask {
        id: Uuid::new_v4(),
        url: url.clone(),
        filename: "verified.bin".into(),
        save_dir: dir.path().to_path_buf(),
        cancelled: Arc::new(AtomicBool::new(false)),
        expected_hash: Some((
            "sha256".into(),
            "0000000000000000000000000000000000000000000000000000000000000000".into(),
        )),
        ..Default::default()
    };
    let err = engine.download(bad_task, None).await.unwrap_err();
    assert!(
        err.contains("mismatch"),
        "Expected hash mismatch error, got: {}",
        err
    );
    assert!(!dir.path().join("verified.bin").exists());
    assert!(dir.path().join("verified.bin.edmpart.corrupt").exists());

    // 2. No explicit expected_hash -> automatically verifies Repr-Digest header from server!
    let good_task = AdaptiveDownloadTask {
        id: Uuid::new_v4(),
        url,
        filename: "verified_repr.bin".into(),
        save_dir: dir.path().to_path_buf(),
        cancelled: Arc::new(AtomicBool::new(false)),
        ..Default::default()
    };
    let ok_path = engine.download(good_task, None).await.expect("Repr-Digest download failed");
    assert_eq!(
        actual_file_sha256(&ok_path),
        expected_sha256_hex(file_size, 11)
    );
}

#[tokio::test]
async fn test_v014_content_encoding_guard_rejects_compressed_range() {
    let (addr, _state, _srv) = spawn_v014_server().await;
    let file_size: u64 = 512 * 1024;
    let url = format!("http://{}/file/{}?seed=5&gzip=1", addr, file_size);

    let dir = tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    let engine = AdaptiveEngine::new(store);

    let task = AdaptiveDownloadTask {
        id: Uuid::new_v4(),
        url,
        filename: "gzip_bad.bin".into(),
        save_dir: dir.path().to_path_buf(),
        cancelled: Arc::new(AtomicBool::new(false)),
        ..Default::default()
    };

    let err = engine.download(task, None).await.unwrap_err();
    assert!(
        err.contains("Content-Encoding"),
        "Expected Content-Encoding guard error, got: {}",
        err
    );
}

#[tokio::test]
async fn test_v014_m3u8_32way_parallel_speedup() {
    let (addr, _state, _srv) = spawn_v014_server().await;
    let url = format!("http://{}/hls/playlist.m3u8?n=32", addr);

    let dir = tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    let engine = Arc::new(AdaptiveEngine::new(store));

    let segs_done = Arc::new(AtomicUsize::new(0));
    let segs_clone = segs_done.clone();
    let cb: edm_core::adaptive::TelemetryCallback = Arc::new(move |t| {
        if t.progress_pct >= 99.0 {
            segs_clone.fetch_add(1, Ordering::Relaxed);
        }
    });

    let task = AdaptiveDownloadTask {
        id: Uuid::new_v4(),
        url,
        filename: "parallel_hls.ts".into(),
        save_dir: dir.path().to_path_buf(),
        config: AdaptiveConfig {
            max_connections: 32,
            ..Default::default()
        },
        cancelled: Arc::new(AtomicBool::new(false)),
        ..Default::default()
    };

    let t0 = Instant::now();
    let out = PackExecutor::execute(task, engine, Client::new(), Some(cb))
        .await
        .expect("32-way HLS failed");
    let elapsed = t0.elapsed();

    // Sequential 32 * 120ms = 3.84s; 32-way parallel should finish well under 1.5s!
    assert!(
        elapsed < Duration::from_millis(1500),
        "Expected 32-way M3U8 < 1.5s, took {:?}",
        elapsed
    );
    let data = std::fs::read(&out).unwrap();
    assert_eq!(data.len(), 32 * 1024);
    // Verify strict index ordering [0..32]
    for i in 0..32usize {
        assert_eq!(data[i * 1024], 0x47);
        assert_eq!(data[i * 1024 + 1], i as u8);
    }
}

#[tokio::test]
async fn test_v014_link_expired_preserves_segments() {
    let (addr, _state, _srv) = spawn_v014_server().await;
    let file_size: u64 = 4 * 1024 * 1024;
    let url = format!(
        "http://{}/file/{}?seed=9&expire_after_reqs=3&token=abc",
        addr, file_size
    );

    let dir = tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    let engine = AdaptiveEngine::new(store.clone());

    let id = Uuid::new_v4();
    let task = AdaptiveDownloadTask {
        id,
        url,
        filename: "expiring.bin".into(),
        save_dir: dir.path().to_path_buf(),
        config: AdaptiveConfig {
            initial_connections: 4,
            max_connections: 4,
            ..Default::default()
        },
        cancelled: Arc::new(AtomicBool::new(false)),
        ..Default::default()
    };

    let res = engine.download(task, None).await;
    assert!(res.is_err());

    let state_str = store.get_download_state(id).unwrap().unwrap();
    assert_eq!(state_str.to_lowercase(), "linkexpired");
    let segs = store.get_segments(id).unwrap();
    assert!(
        !segs.is_empty(),
        "Expected LinkExpired exit to preserve checkpointed segments"
    );
}
