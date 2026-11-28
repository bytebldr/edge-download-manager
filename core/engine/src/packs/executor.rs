//! Production-Grade Multi-Protocol Feature Pack Executor
//! Orchestrates downloads for HTTP, BitTorrent, FTP, eD2k, M3U8, FFmpeg, yt-dlp, GitHub, and Hugging Face

use super::bittorrent::BitTorrentPack;
use super::ed2k::Ed2kPack;
use super::ftp::FtpPack;
use super::m3u8::M3u8Pack;
use crate::adaptive::{
    AccelerationMode, AdaptiveDownloadTask, AdaptiveEngine, AdaptiveTelemetry, TelemetryCallback,
};
use chrono::Utc;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct PackExecutor;

impl PackExecutor {
    /// Execute a download using the appropriate feature pack pipeline through the AdaptiveWrapper
    pub async fn execute(
        task: AdaptiveDownloadTask,
        engine: Arc<AdaptiveEngine>,
        client: Client,
        telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        crate::adaptive_wrapper::AdaptiveWrapper::execute(task, engine, client, telemetry_cb).await
    }

    /// M3U8 / DASH Pack: Parse playlist or .mpd manifest, download segments, and transmux to MP4
    pub(crate) async fn execute_m3u8(
        task: AdaptiveDownloadTask,
        client: Client,
        telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        if task
            .url
            .split('?')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .ends_with(".mpd")
        {
            let target_file = task.save_dir.join(&task.filename);
            let recorder = super::dash::DashLiveRecorder::new(super::dash::LiveRecordConfig::new(
                task.url.clone(),
                target_file.clone(),
            ));
            let cancel_handle = recorder.cancel_handle();
            let task_cancelled = task.cancelled.clone();
            let watcher = tokio::spawn(async move {
                while !task_cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
                cancel_handle.cancel();
            });
            let res = recorder.record().await;
            watcher.abort();
            return res.map(|p| p.output_path);
        }

        let resp = client
            .get(&task.url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch M3U8 manifest: {}", e))?;

        let manifest_text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read M3U8 manifest body: {}", e))?;

        let playlist = if manifest_text.contains("#EXT-X-STREAM-INF") {
            let variants = M3u8Pack::parse_master_playlist(&manifest_text, &task.url)?;
            let best_variant = variants
                .into_iter()
                .max_by_key(|v| v.bandwidth)
                .ok_or_else(|| "No stream variants found in master playlist".to_string())?;

            let sub_resp = client
                .get(&best_variant.url)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let sub_text = sub_resp.text().await.map_err(|e| e.to_string())?;
            M3u8Pack::parse_media_playlist(&sub_text, &best_variant.url)?
        } else {
            M3u8Pack::parse_media_playlist(&manifest_text, &task.url)?
        };

        let total_segments = playlist.segments.len();
        if total_segments == 0 {
            return Err("M3U8 playlist contains 0 media segments".to_string());
        }

        let target_file = task.save_dir.join(&task.filename);
        let temp_dir = task.save_dir.join(format!(".tmp_{}", task.id));
        tokio::fs::create_dir_all(&temp_dir)
            .await
            .map_err(|e| format!("Failed to create segment temp dir: {}", e))?;

        let mut segment_files = Vec::new();
        let key_cache: Arc<tokio::sync::Mutex<std::collections::HashMap<String, [u8; 16]>>> =
            Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));

        // 1. Download initialization segment if present (#EXT-X-MAP)
        if let Some(ref init_url) = playlist.init_segment_url {
            let mut req = client.get(init_url);
            if let Some(ref ua) = task.user_agent {
                req = req.header("User-Agent", ua);
            }
            if let Some(ref cookies) = task.cookies {
                req = req.header("Cookie", cookies);
            }
            if let Some(ref referer) = task.referer {
                req = req.header("Referer", referer);
            }

            let init_resp = req
                .send()
                .await
                .map_err(|e| format!("Init segment download failed: {}", e))?;
            if !init_resp.status().is_success() {
                return Err(format!(
                    "M3U8 init segment returned HTTP {}",
                    init_resp.status().as_u16()
                ));
            }
            let init_bytes = init_resp
                .bytes()
                .await
                .map_err(|e| format!("Init segment read error: {}", e))?;
            let init_path = temp_dir.join("init.mp4");
            tokio::fs::write(&init_path, init_bytes)
                .await
                .map_err(|e| e.to_string())?;
            segment_files.push(init_path);
        }

        // 2. Download all media segments in parallel (up to 32 concurrent connections)
        use futures_util::stream::{self, StreamExt};
        let concurrency = (task.config.max_connections.clamp(1, 32)) as usize;
        let total_downloaded = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let completed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let start_time = Utc::now();

        let results: Vec<Result<(usize, PathBuf), String>> =
            stream::iter(playlist.segments.iter().cloned().enumerate())
                .map(|(i, seg)| {
                    let client = client.clone();
                    let task = task.clone();
                    let temp_dir = temp_dir.clone();
                    let key_cache = key_cache.clone();
                    let total_downloaded = total_downloaded.clone();
                    let completed_count = completed_count.clone();
                    let telemetry_cb = telemetry_cb.clone();
                    async move {
                        if task.cancelled.load(Ordering::Relaxed) {
                            return Err("Download cancelled by user".to_string());
                        }

                        let mut last_err = String::new();
                        let mut raw_bytes_opt = None;
                        for attempt in 0..3u32 {
                            if task.cancelled.load(Ordering::Relaxed) {
                                return Err("Download cancelled by user".to_string());
                            }
                            let mut req = client.get(&seg.url);
                            if let Some(ref ua) = task.user_agent {
                                req = req.header("User-Agent", ua);
                            }
                            if let Some(ref cookies) = task.cookies {
                                req = req.header("Cookie", cookies);
                            }
                            if let Some(ref referer) = task.referer {
                                req = req.header("Referer", referer);
                            }

                            match req.send().await {
                                Ok(resp) if resp.status().is_success() => {
                                    match resp.bytes().await {
                                        Ok(b) => {
                                            raw_bytes_opt = Some(b);
                                            break;
                                        }
                                        Err(e) => {
                                            last_err =
                                                format!("Segment #{} read error: {}", i + 1, e);
                                        }
                                    }
                                }
                                Ok(resp) => {
                                    last_err = format!(
                                        "Segment #{} returned HTTP {}",
                                        i + 1,
                                        resp.status().as_u16()
                                    );
                                }
                                Err(e) => {
                                    last_err = format!("Segment #{} failed: {}", i + 1, e);
                                }
                            }
                            if attempt < 2 {
                                tokio::time::sleep(std::time::Duration::from_millis(
                                    250 * (attempt as u64 + 1),
                                ))
                                .await;
                            }
                        }

                        let raw_bytes = raw_bytes_opt.ok_or(last_err)?;

                        let final_bytes = if let Some(ref key_url) = seg.key_url {
                            let key_bytes = {
                                let mut guard = key_cache.lock().await;
                                if let Some(cached) = guard.get(key_url) {
                                    *cached
                                } else {
                                    let mut k_req = client.get(key_url);
                                    if let Some(ref ua) = task.user_agent {
                                        k_req = k_req.header("User-Agent", ua);
                                    }
                                    if let Some(ref cookies) = task.cookies {
                                        k_req = k_req.header("Cookie", cookies);
                                    }
                                    if let Some(ref referer) = task.referer {
                                        k_req = k_req.header("Referer", referer);
                                    }
                                    let k_resp = k_req.send().await.map_err(|e| {
                                        format!("Failed to fetch HLS key from {}: {}", key_url, e)
                                    })?;
                                    if !k_resp.status().is_success() {
                                        return Err(format!(
                                            "HLS key {} returned HTTP {}",
                                            key_url,
                                            k_resp.status().as_u16()
                                        ));
                                    }
                                    let k_data = k_resp
                                        .bytes()
                                        .await
                                        .map_err(|e| format!("Failed to read HLS key: {}", e))?;
                                    if k_data.len() != 16 {
                                        return Err(format!(
                                            "Invalid AES-128 key length {} from {}",
                                            k_data.len(),
                                            key_url
                                        ));
                                    }
                                    let mut arr = [0u8; 16];
                                    arr.copy_from_slice(&k_data);
                                    guard.insert(key_url.clone(), arr);
                                    arr
                                }
                            };

                            let iv = seg.iv.unwrap_or_else(|| {
                                super::remux::sequence_to_iv(seg.sequence_number)
                            });
                            super::remux::decrypt_aes128_segment(&raw_bytes, &key_bytes, &iv)?
                        } else {
                            raw_bytes.to_vec()
                        };

                        let part_seg_path = temp_dir.join(format!("seg_{:05}.part", i));
                        let seg_file_path = temp_dir.join(format!("seg_{:05}.chunk", i));
                        tokio::fs::write(&part_seg_path, &final_bytes)
                            .await
                            .map_err(|e| format!("Write error on segment #{}: {}", i + 1, e))?;
                        tokio::fs::rename(&part_seg_path, &seg_file_path)
                            .await
                            .map_err(|e| format!("Rename error on segment #{}: {}", i + 1, e))?;

                        let dl_now = total_downloaded
                            .fetch_add(final_bytes.len() as u64, Ordering::Relaxed)
                            + final_bytes.len() as u64;
                        let done_now = completed_count.fetch_add(1, Ordering::Relaxed) + 1;

                        if let Some(ref cb) = telemetry_cb {
                            let elapsed = (Utc::now() - start_time).num_seconds().max(1) as u64;
                            let bps = dl_now / elapsed;
                            let pct = (done_now as f64 / total_segments as f64) * 100.0;

                            cb(AdaptiveTelemetry {
                                download_id: task.id.to_string(),
                                total_bytes: None,
                                downloaded_bytes: dl_now,
                                progress_pct: pct,
                                instant_throughput_bps: bps,
                                rolling_throughput_bps: bps,
                                upload_throughput_bps: 0,
                                uploaded_bytes: 0,
                                active_connections: concurrency as u32,
                                target_connections: concurrency as u32,
                                live_workers: concurrency as u32,
                                max_allowed_connections: 32,
                                mode: AccelerationMode::Optimizing,
                                eta_seconds: Some((total_segments.saturating_sub(done_now)) as u64),
                                decision_reason: format!(
                                    "M3U8 32-way Parallel Chunk {}/{}",
                                    done_now, total_segments
                                ),
                                phase: Some("Downloading".to_string()),
                                workers: Vec::new(),
                                updated_at: Utc::now(),
                                ..Default::default()
                            });
                        }

                        Ok((i, seg_file_path))
                    }
                })
                .buffer_unordered(concurrency)
                .collect()
                .await;

        let mut indexed_paths = Vec::with_capacity(total_segments);
        for res in results {
            match res {
                Ok(pair) => indexed_paths.push(pair),
                Err(e) => {
                    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                    return Err(e);
                }
            }
        }
        indexed_paths.sort_by_key(|(idx, _)| *idx);
        for (_, path) in indexed_paths {
            segment_files.push(path);
        }

        let target_ext = target_file
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("mp4");

        // Inspect first segment bytes to determine container format (fMP4 vs MPEG-TS)
        let is_ts_stream = if let Some(first_seg) = segment_files.first() {
            if let Ok(mut f) = tokio::fs::File::open(first_seg).await {
                use tokio::io::AsyncReadExt;
                let mut header = [0u8; 4];
                if f.read_exact(&mut header).await.is_ok() {
                    header[0] == 0x47 // MPEG-TS sync byte
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        let final_path = if is_ts_stream || target_ext == "ts" {
            let out_ts = target_file.with_extension("ts");
            super::remux::NativeRemuxer::remux_segments(&segment_files, &out_ts).await?
        } else {
            let final_mp4 = target_file.with_extension("mp4");
            super::remux::NativeRemuxer::remux_segments(&segment_files, &final_mp4).await?
        };

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        Ok(final_path)
    }

    /// Media Extraction Pipeline: Native In-Process Extractors (YouTube, Vimeo)
    /// Streams directly with the high-performance Adaptive HTTP Engine
    pub(crate) async fn execute_ytdlp(
        task: AdaptiveDownloadTask,
        engine: Arc<AdaptiveEngine>,
        client: Client,
        telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        use super::extractors::{SiteExtractor, VimeoExtractor, YouTubeExtractor};
        use super::ytdlp::YtDlpPack;

        // 1. If yt-dlp is available on host system, prioritize it!
        // Solves YouTube JavaScript challenges and merges high-res video + audio streams cleanly.
        let mut ytdlp_err: Option<String> = None;
        if YtDlpPack::ensure_binary().await.is_some() {
            match YtDlpPack::download_media(task.clone(), telemetry_cb.clone()).await {
                Ok(path) => return Ok(path),
                Err(err) => {
                    if err.contains("cancelled") || task.cancelled.load(Ordering::Relaxed) {
                        return Err(err);
                    }
                    tracing::warn!(
                        "yt-dlp execution error: {}. Falling back to in-process extractors...",
                        err
                    );
                    ytdlp_err = Some(err);
                }
            }
        }

        // 2. In-process extractor fallback
        let yt = YouTubeExtractor;
        let vimeo = VimeoExtractor;

        let extracted = if yt.can_extract(&task.url) {
            match yt.extract(&task.url, &client).await {
                Ok(ext) => ext,
                Err(ext_err) => {
                    if let Some(err) = ytdlp_err {
                        return Err(format!("Media extraction failed: {err}"));
                    } else {
                        return Err(format!("YouTube extraction error: {ext_err}"));
                    }
                }
            }
        } else if vimeo.can_extract(&task.url) {
            match vimeo.extract(&task.url, &client).await {
                Ok(ext) => ext,
                Err(ext_err) => {
                    if let Some(err) = ytdlp_err {
                        return Err(format!("Media extraction failed: {err}"));
                    } else {
                        return Err(format!("Vimeo extraction error: {ext_err}"));
                    }
                }
            }
        } else {
            if let Some(err) = ytdlp_err {
                return Err(format!("Media extraction failed: {err}"));
            }
            return Err("Site not supported by EDM yet. Please install yt-dlp for universal media extraction.".to_string());
        };

        let stream_url = extracted
            .best_stream_url
            .ok_or_else(|| "No streamable format found".to_string())?;

        let mut actual_task = task;
        actual_task.url = stream_url;
        if yt.can_extract(&actual_task.url) || actual_task.url.contains("googlevideo.com") {
            actual_task.user_agent = Some("Mozilla/5.0 (Linux; Android 12; Quest 3) AppleWebKit/537.36 (KHTML, like Gecko) OculusBrowser/32.0.0.35.485 Chrome/120.0.6099.144 Mobile VR Safari/537.36".to_string());
        }
        if actual_task.filename.is_empty()
            || actual_task.filename == "download"
            || actual_task.filename == "download.bin"
            || actual_task.filename == "watch"
            || actual_task.filename == "watch.mp4"
            || actual_task.filename == "video"
            || actual_task.filename == "video.mp4"
        {
            let sanitized_title = crate::sanitizer::Sanitizer::sanitize_filename(&extracted.title);
            actual_task.filename = format!("{}.mp4", sanitized_title);
        }

        engine.download(actual_task, telemetry_cb).await
    }

    /// FTP Pack: Async TCP wire connection & file streaming
    pub(crate) async fn execute_ftp(
        task: AdaptiveDownloadTask,
        telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        let target = FtpPack::parse_url(&task.url)?;
        let target_file = task.save_dir.join(&task.filename);
        let mut file = tokio::fs::File::create(&target_file)
            .await
            .map_err(|e| format!("Failed to create FTP target file: {}", e))?;

        let addr = format!("{}:{}", target.host, target.port);
        let mut control_stream = tokio::net::TcpStream::connect(&addr)
            .await
            .map_err(|e| format!("Failed to connect to FTP server {}: {}", addr, e))?;

        let mut buf = [0u8; 1024];

        async fn read_ftp_reply(
            stream: &mut tokio::net::TcpStream,
            buf: &mut [u8],
            allowed_prefixes: &[char],
            step: &str,
        ) -> Result<String, String> {
            let n = stream
                .read(buf)
                .await
                .map_err(|e| format!("FTP {} read error: {}", step, e))?;
            if n == 0 {
                return Err(format!("FTP {} closed connection unexpectedly", step));
            }
            let reply = String::from_utf8_lossy(&buf[..n]).to_string();
            let first = reply.trim_start().chars().next().unwrap_or('\0');
            if !allowed_prefixes.contains(&first) {
                return Err(format!("FTP {} failed: {}", step, reply.trim()));
            }
            Ok(reply)
        }

        read_ftp_reply(&mut control_stream, &mut buf, &['2'], "greeting").await?;

        control_stream
            .write_all(FtpPack::cmd_user(&target.user).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        read_ftp_reply(&mut control_stream, &mut buf, &['2', '3'], "USER").await?;

        control_stream
            .write_all(FtpPack::cmd_pass(&target.pass).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        read_ftp_reply(&mut control_stream, &mut buf, &['2'], "PASS").await?;

        control_stream
            .write_all(FtpPack::cmd_type_binary().as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        read_ftp_reply(&mut control_stream, &mut buf, &['2'], "TYPE I").await?;

        let total_size: Option<u64> = if control_stream
            .write_all(FtpPack::cmd_size(&target.path).as_bytes())
            .await
            .is_ok()
        {
            read_ftp_reply(&mut control_stream, &mut buf, &['2'], "SIZE")
                .await
                .ok()
                .and_then(|r| r.split_whitespace().nth(1)?.parse::<u64>().ok())
        } else {
            None
        };

        control_stream
            .write_all(FtpPack::cmd_pasv().as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let pasv_resp = read_ftp_reply(&mut control_stream, &mut buf, &['2'], "PASV").await?;
        let (data_ip, data_port) = FtpPack::parse_pasv_response(&pasv_resp)?;

        let data_addr = format!("{}:{}", data_ip, data_port);
        let mut data_stream = tokio::net::TcpStream::connect(&data_addr)
            .await
            .map_err(|e| {
                format!(
                    "Failed to connect to FTP passive data port {}: {}",
                    data_addr, e
                )
            })?;

        control_stream
            .write_all(FtpPack::cmd_retr(&target.path).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        read_ftp_reply(&mut control_stream, &mut buf, &['1'], "RETR").await?;

        let mut total_downloaded: u64 = 0;
        let mut data_chunk = [0u8; 32768];
        let start_time = Utc::now();

        loop {
            if task.cancelled.load(Ordering::Relaxed) {
                return Err("Download cancelled by user".to_string());
            }

            let bytes_read = data_stream
                .read(&mut data_chunk)
                .await
                .map_err(|e| e.to_string())?;
            if bytes_read == 0 {
                break;
            }

            file.write_all(&data_chunk[..bytes_read])
                .await
                .map_err(|e| e.to_string())?;
            total_downloaded += bytes_read as u64;

            if let Some(ref cb) = telemetry_cb {
                let elapsed = (Utc::now() - start_time).num_seconds().max(1) as u64;
                let bps = total_downloaded / elapsed;
                let progress_pct = total_size
                    .map(|t| {
                        if t > 0 {
                            ((total_downloaded as f64 / t as f64) * 100.0).min(100.0)
                        } else {
                            0.0
                        }
                    })
                    .unwrap_or(0.0);

                cb(AdaptiveTelemetry {
                    download_id: task.id.to_string(),
                    total_bytes: total_size,
                    downloaded_bytes: total_downloaded,
                    progress_pct,
                    instant_throughput_bps: bps,
                    rolling_throughput_bps: bps,
                    upload_throughput_bps: 0,
                    uploaded_bytes: 0,
                    active_connections: 1,
                    target_connections: 1,
                    live_workers: 1,
                    max_allowed_connections: 1,
                    mode: AccelerationMode::Stable,
                    eta_seconds: None,
                    decision_reason: "FTP Stream Transfer".into(),
                    phase: Some("Downloading".to_string()),
                    workers: vec![],
                    updated_at: Utc::now(),
                    ..Default::default()
                });
            }
        }

        file.flush().await.map_err(|e| e.to_string())?;
        Ok(target_file)
    }

    /// BitTorrent Pack: In-process Torrent and Magnet Swarm Engine via librqbit
    pub(crate) async fn execute_bittorrent(
        task: AdaptiveDownloadTask,
        telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        let power = task.torrent_power.clone();
        BitTorrentPack::download_in_process_with_power(task, telemetry_cb, power).await
    }

    /// eD2k Pack: eD2k link resolver
    pub(crate) async fn execute_ed2k(
        task: AdaptiveDownloadTask,
        _telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        let _ed2k = Ed2kPack::parse_link(&task.url)?;
        Err("eD2k/Kad P2P protocol transfer is not supported in EDM v0.0.12. Please use HTTP/HTTPS, FTP, HLS/DASH, or BitTorrent/Magnet.".into())
    }
}
