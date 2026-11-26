//! BitTorrent & Magnet Feature Pack (Native Rust Implementation)
//! Features: Magnet links, torrents, peers, trackers, bencode parser, peer wire protocol

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagnetInfo {
    pub info_hash_hex: String,
    pub display_name: Option<String>,
    pub trackers: Vec<String>,
    pub exact_topic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFileEntry {
    pub index: usize,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentInspectionResult {
    pub name: String,
    pub total_bytes: u64,
    pub files: Vec<TorrentFileEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BencodeValue {
    Integer(i64),
    ByteString(Vec<u8>),
    List(Vec<BencodeValue>),
    Dict(BTreeMap<Vec<u8>, BencodeValue>),
}

pub struct BitTorrentPack;

pub const TIER1_TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://opentracker.i2p.rocks:6969/announce",
    "udp://open.demonii.com:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://explodie.org:6969/announce",
    "udp://tracker.coppersurfer.tk:6969/announce",
    "udp://tracker.moeking.me:6969/announce",
];

fn find_largest_file_recursive(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut best_file = None;
    let mut max_size = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file() {
                    let sz = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    if sz > max_size {
                        max_size = sz;
                        best_file = Some(path);
                    }
                }
            }
        }
    }
    best_file
}

async fn load_add_torrent(
    url: &str,
    cookies: Option<&str>,
    referer: Option<&str>,
    user_agent: Option<&str>,
) -> Result<librqbit::AddTorrent<'static>, String> {
    use librqbit::AddTorrent;

    const MAX_TORRENT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB limit

    if url.starts_with("magnet:") {
        let mut enriched = url.to_string();
        for tr in TIER1_TRACKERS {
            let enc = urlencoding::encode(tr);
            if !enriched.contains(&enc.to_string()) && !enriched.contains(tr) {
                enriched.push_str("&tr=");
                enriched.push_str(&enc);
            }
        }
        return Ok(AddTorrent::from_url(enriched));
    }

    if url.starts_with("http://") || url.starts_with("https://") {
        let mut req = reqwest::Client::new().get(url);
        if let Some(c) = cookies {
            req = req.header("Cookie", c);
        }
        if let Some(r) = referer {
            req = req.header("Referer", r);
        }
        if let Some(ua) = user_agent {
            req = req.header("User-Agent", ua);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Failed to download remote .torrent file: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Failed to fetch .torrent: HTTP {}", resp.status()));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read .torrent response bytes: {}", e))?;

        validate_torrent_bytes(
            bytes.as_ref(),
            "received HTML authentication page or invalid response instead of bencode data",
        )?;
        return Ok(AddTorrent::from_bytes(bytes.to_vec()));
    }

    let path = if url.starts_with("file://") {
        url::Url::parse(url)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .unwrap_or_else(|| std::path::PathBuf::from(url))
    } else {
        std::path::PathBuf::from(url)
    };

    if path.exists() {
        let meta = tokio::fs::metadata(&path)
            .await
            .map_err(|e| format!("Failed to read metadata for {}: {}", path.display(), e))?;
        if meta.len() as usize > MAX_TORRENT_BYTES {
            return Err("Torrent file exceeds 10 MiB limit".into());
        }
        let data = tokio::fs::read(&path)
            .await
            .map_err(|e| format!("Failed to read .torrent file: {}", e))?;

        validate_torrent_bytes(&data, "contains HTML text instead of bencode data")?;
        return Ok(AddTorrent::from_bytes(data));
    }

    Ok(AddTorrent::from_url(url.to_string()))
}

fn validate_torrent_bytes(bytes: &[u8], html_msg: &str) -> Result<(), String> {
    const MAX_TORRENT_BYTES: usize = 10 * 1024 * 1024;
    if bytes.len() > MAX_TORRENT_BYTES {
        return Err("Torrent file exceeds 10 MiB limit".into());
    }
    if bytes.is_empty() {
        return Err("Torrent file is empty (0 bytes)".into());
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(64)]).to_lowercase();
    if head.contains("<html") || head.contains("<!doctype") {
        return Err(format!("Invalid .torrent file: {}", html_msg));
    }
    Ok(())
}

impl BitTorrentPack {
    /// Inspect torrent/magnet metadata and file list without downloading payload
    pub async fn inspect_torrent(
        url: &str,
        save_dir: &std::path::Path,
    ) -> Result<TorrentInspectionResult, String> {
        use librqbit::{
            AddTorrentOptions, AddTorrentResponse, ListenerOptions, Session, SessionOptions,
        };
        use std::time::Duration;

        let add_torrent = load_add_torrent(url, None, None, None).await?;

        let mut listener_opts = ListenerOptions::default();
        listener_opts.listen_addr = "0.0.0.0:0"
            .parse()
            .unwrap_or_else(|_| std::net::SocketAddr::from(([0, 0, 0, 0], 0)));

        let session_opts = SessionOptions {
            peer_limit: Some(32),
            client_name_and_version: Some(format!(
                "EDM/{} (Evo Download Manager)",
                env!("CARGO_PKG_VERSION")
            )),
            listen: Some(listener_opts),
            ..Default::default()
        };

        let session = Session::new_with_opts(save_dir.to_path_buf(), session_opts)
            .await
            .map_err(|e| format!("Failed to create BitTorrent session: {}", e))?;

        let add_opts = AddTorrentOptions {
            list_only: true,
            ..Default::default()
        };

        let add_res = tokio::time::timeout(
            Duration::from_secs(25),
            session.add_torrent(add_torrent, Some(add_opts)),
        )
        .await;

        let _ = session.stop().await;

        let resp = match add_res {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => return Err(format!("Failed to inspect torrent: {}", e)),
            Err(_) => {
                return Err("Timed out after 25s resolving torrent/magnet metadata".to_string())
            }
        };

        match resp {
            AddTorrentResponse::ListOnly(list_res) => {
                let files: Vec<TorrentFileEntry> = list_res
                    .info
                    .iter_file_details()
                    .enumerate()
                    .map(|(index, d)| TorrentFileEntry {
                        index,
                        path: d.filename.to_string(),
                        size_bytes: d.len,
                    })
                    .collect();
                let total_bytes = files.iter().map(|f| f.size_bytes).sum();
                let name = list_res
                    .info
                    .name()
                    .map(|n| n.into_owned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "torrent".to_string());

                Ok(TorrentInspectionResult {
                    name,
                    total_bytes,
                    files,
                })
            }
            _ => Err("Unexpected non-list response during torrent inspection".to_string()),
        }
    }

    /// Download a torrent or magnet URI in-process using librqbit
    pub async fn download_in_process(
        task: crate::adaptive::AdaptiveDownloadTask,
        telemetry_cb: Option<crate::adaptive::TelemetryCallback>,
    ) -> Result<std::path::PathBuf, String> {
        Self::download_in_process_with_power(task, telemetry_cb, None).await
    }

    /// Download a torrent or magnet URI in-process with optional TorrentPowerConfig
    pub async fn download_in_process_with_power(
        task: crate::adaptive::AdaptiveDownloadTask,
        telemetry_cb: Option<crate::adaptive::TelemetryCallback>,
        power: Option<crate::adaptive::types::TorrentPowerConfig>,
    ) -> Result<std::path::PathBuf, String> {
        use crate::adaptive::{AccelerationMode, AdaptiveTelemetry, WorkerStatus, WorkerTelemetry};
        use chrono::Utc;
        use librqbit::{AddTorrentOptions, ListenerOptions, Session, SessionOptions};
        use std::sync::atomic::Ordering;
        use std::time::Duration;

        let power_cfg = power.unwrap_or_default();

        let add_torrent = load_add_torrent(
            &task.url,
            task.cookies.as_deref(),
            task.referer.as_deref(),
            task.user_agent.as_deref(),
        )
        .await?;

        let peer_limit = task.config.max_connections.clamp(4, 128) as usize;
        let mut listener_opts = ListenerOptions::default();
        listener_opts.enable_upnp_port_forwarding = true;
        listener_opts.listen_addr = "0.0.0.0:0"
            .parse()
            .unwrap_or_else(|_| std::net::SocketAddr::from(([0, 0, 0, 0], 0)));

        let mut session_opts = SessionOptions {
            peer_limit: Some(peer_limit),
            client_name_and_version: Some(format!(
                "EDM/{} (Evo Download Manager)",
                env!("CARGO_PKG_VERSION")
            )),
            listen: Some(listener_opts),
            disable_local_service_discovery: !power_cfg.enable_pex,
            ..Default::default()
        };
        if !power_cfg.enable_dht {
            session_opts.dht = None;
        }

        let session = Session::new_with_opts(task.save_dir.clone(), session_opts)
            .await
            .map_err(|e| format!("Failed to create BitTorrent session: {}", e))?;

        let custom_sub = if power_cfg.create_subfolder {
            power_cfg
                .custom_folder_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(crate::sanitizer::Sanitizer::sanitize_filename)
        } else {
            None
        };

        let extra_trackers = if power_cfg.auto_update_trackers {
            Some(TIER1_TRACKERS.iter().map(|s| (*s).to_string()).collect())
        } else {
            None
        };

        let add_opts = AddTorrentOptions {
            paused: false,
            overwrite: true,
            only_files: task.selected_files.clone(),
            sub_folder: custom_sub.clone(),
            trackers: extra_trackers,
            ..Default::default()
        };

        let add_fut = session.add_torrent(add_torrent, Some(add_opts));
        tokio::pin!(add_fut);

        let handle_res = loop {
            tokio::select! {
                res = &mut add_fut => {
                    break res.map_err(|e| format!("Failed to add torrent: {}", e))?;
                }
                _ = tokio::time::sleep(Duration::from_millis(500)) => {
                    if task.cancelled.load(Ordering::Relaxed) {
                        let _ = session.stop().await;
                        return Err("BitTorrent download cancelled by user".to_string());
                    }
                    if let Some(ref cb) = telemetry_cb {
                        cb(AdaptiveTelemetry {
                            download_id: task.id.to_string(),
                            total_bytes: None,
                            downloaded_bytes: 0,
                            progress_pct: 0.0,
                            instant_throughput_bps: 0,
                            rolling_throughput_bps: 0,
                            upload_throughput_bps: 0,
                            uploaded_bytes: 0,
                            active_connections: 1,
                            target_connections: task.config.max_connections,
                            live_workers: 1,
                            max_allowed_connections: task.config.max_connections.max(32),
                            mode: AccelerationMode::Optimizing,
                            eta_seconds: None,
                            decision_reason: "Resolving Magnet & DHT Metadata...".to_string(),
                            phase: Some("Resolving".to_string()),
                            workers: Vec::new(),
                            updated_at: Utc::now(),
                            ..Default::default()
                        });
                    }
                }
            }
        };

        let handle = handle_res
            .into_handle()
            .ok_or_else(|| "Torrent already exists or could not create handle".to_string())?;

        let mut ticker = tokio::time::interval(Duration::from_millis(500));
        let mut target_file_path = if let Some(ref sub) = custom_sub {
            task.save_dir.join(sub)
        } else {
            task.save_dir.join(&task.filename)
        };
        let mut disk_space_checked = false;
        let mut prev_downloaded = 0u64;
        let mut prev_uploaded_bytes = 0u64;
        let mut rolling_bps = 0u64;
        let mut prev_instant_bps = 0u64;
        let mut last_tick_time = std::time::Instant::now();
        let mut last_upload_sample = std::time::Instant::now();
        let mut was_initializing = true;
        let mut smoothed_eta_sec: Option<u64> = None;
        let mut eta_sample_count: u32 = 0;
        let mut seeding_started_at: Option<std::time::Instant> = None;

        loop {
            ticker.tick().await;

            if task.cancelled.load(Ordering::Relaxed) {
                let _ = session.stop().await;
                return Err("BitTorrent download cancelled by user".to_string());
            }

            let stats = handle.stats();
            if matches!(stats.state, librqbit::TorrentStatsState::Error) {
                let _ = session.stop().await;
                return Err(
                    "BitTorrent engine entered error state (disk I/O or swarm failure)".to_string(),
                );
            }

            let is_initializing = matches!(
                stats.state,
                librqbit::TorrentStatsState::Initializing { .. }
            );

            let now = std::time::Instant::now();
            let elapsed_sec = now.duration_since(last_tick_time).as_secs_f64().max(0.1);
            last_tick_time = now;

            // Handle transition out of piece verification
            if is_initializing {
                prev_downloaded = stats.progress_bytes;
                prev_uploaded_bytes = stats.uploaded_bytes;
                was_initializing = true;
            } else if was_initializing {
                was_initializing = false;
                prev_downloaded = stats.progress_bytes;
                prev_uploaded_bytes = stats.uploaded_bytes;
                last_tick_time = now;
                last_upload_sample = now;
            }

            // 1. Adaptive Engine Token Bucket Speed Limiter Enforcement (Network bytes only)
            let delta = if is_initializing {
                0
            } else {
                let d = stats.progress_bytes.saturating_sub(prev_downloaded);
                prev_downloaded = stats.progress_bytes;
                d
            };

            if delta > 0 {
                if let Some(ref lim) = task.limiter {
                    lim.acquire(delta).await;
                }
            }

            let total_bytes = if stats.total_bytes > 0 {
                Some(stats.total_bytes)
            } else {
                None
            };

            // Deferred disk space check once torrent metadata resolves total size
            if !disk_space_checked && stats.total_bytes > 0 {
                disk_space_checked = true;
                if let Err(e) = crate::security::check_disk_space(&task.save_dir, stats.total_bytes)
                {
                    let _ = session.stop().await;
                    return Err(format!("Insufficient disk space: {}", e));
                }
            }

            let progress_pct = if stats.total_bytes > 0 {
                (stats.progress_bytes as f64 / stats.total_bytes as f64) * 100.0
            } else {
                0.0
            };

            let (live_conns, dl_speed, ul_speed) = if let Some(ref live) = stats.live {
                (
                    live.snapshot.peer_stats.live,
                    live.download_speed.as_bytes(),
                    live.upload_speed.as_bytes(),
                )
            } else {
                (0, 0, 0)
            };

            let upload_delta = if is_initializing {
                0
            } else {
                stats.uploaded_bytes.saturating_sub(prev_uploaded_bytes)
            };
            prev_uploaded_bytes = stats.uploaded_bytes;
            let elapsed_ul_sec = now
                .saturating_duration_since(last_upload_sample)
                .as_secs_f64();
            last_upload_sample = now;
            let measured_ul_bps = if elapsed_ul_sec > 0.0 {
                ((upload_delta as f64 / elapsed_ul_sec) * 8.0) as u64
            } else {
                0
            };
            let upload_throughput_bps = measured_ul_bps.max(ul_speed * 8);

            if let Some(ref sub) = custom_sub {
                target_file_path = task.save_dir.join(sub);
            } else if let Some(name) = handle.name() {
                let safe_name = crate::sanitizer::Sanitizer::sanitize_filename(&name);
                target_file_path = task.save_dir.join(safe_name);
            }

            // 2. High-Accuracy Throughput Evaluation: Ignore piece hashing disk reads
            let instant_bps = if is_initializing {
                0
            } else {
                let measured_bps = ((delta as f64 / elapsed_sec) * 8.0) as u64;
                let reported_bps = dl_speed * 8;
                if measured_bps > 0 {
                    measured_bps.max(reported_bps)
                } else {
                    reported_bps
                }
            };

            rolling_bps = if is_initializing {
                0
            } else if rolling_bps == 0 {
                instant_bps
            } else {
                (0.3 * instant_bps as f64 + 0.7 * rolling_bps as f64) as u64
            };

            let mode =
                if live_conns == 0 || (instant_bps > prev_instant_bps && prev_instant_bps > 0) {
                    AccelerationMode::Optimizing
                } else if live_conns >= (task.config.max_connections / 2).max(2) {
                    AccelerationMode::Stable
                } else {
                    AccelerationMode::Auto
                };
            prev_instant_bps = instant_bps;

            let decision_reason = if is_initializing {
                "Verifying existing data on disk...".to_string()
            } else {
                match mode {
                    AccelerationMode::Optimizing => {
                        if stats.total_bytes == 0 {
                            "Resolving DHT Swarm & Metadata".to_string()
                        } else if live_conns == 0 {
                            "Probing Swarm Peers via DHT".to_string()
                        } else {
                            let eff_speed = instant_bps / 8;
                            let speed_display = if eff_speed >= 1024 * 1024 {
                                format!("{:.1} MB/s", eff_speed as f64 / (1024.0 * 1024.0))
                            } else if eff_speed >= 1024 {
                                format!("{:.1} KB/s", eff_speed as f64 / 1024.0)
                            } else {
                                format!("{} B/s", eff_speed)
                            };
                            format!(
                                "Adaptive Swarm Ramp-Up ({} peers @ {})",
                                live_conns, speed_display
                            )
                        }
                    }
                    AccelerationMode::Stable => {
                        format!(
                            "High-Throughput Swarm Plateau ({} peers connected)",
                            live_conns
                        )
                    }
                    _ => format!("BitTorrent Swarm ({} active peers)", live_conns),
                }
            };

            // 3. Multi-Stream Adaptive Worker Telemetry Synthesis
            let num_workers = live_conns.clamp(1, task.config.max_connections).min(32);
            let worker_speed = if num_workers > 0 {
                instant_bps / num_workers as u64
            } else {
                0
            };
            let mut workers = Vec::with_capacity(num_workers as usize);

            if stats.total_bytes > 0 {
                let chunk_size = stats.total_bytes / num_workers as u64;
                for i in 0..num_workers {
                    let start = i as u64 * chunk_size;
                    let end = if i == num_workers - 1 {
                        stats.total_bytes
                    } else {
                        (i as u64 + 1) * chunk_size
                    };
                    let worker_total = end.saturating_sub(start);
                    let worker_downloaded = if stats.total_bytes > 0 {
                        ((stats.progress_bytes as f64
                            * (worker_total as f64 / stats.total_bytes as f64))
                            as u64)
                            .min(worker_total)
                    } else {
                        0
                    };
                    let status = if stats.finished || worker_downloaded >= worker_total {
                        WorkerStatus::Completed
                    } else if instant_bps > 0 {
                        WorkerStatus::Downloading
                    } else {
                        WorkerStatus::Connecting
                    };
                    workers.push(WorkerTelemetry {
                        idx: i,
                        start,
                        end,
                        pos: start + worker_downloaded,
                        bytes_downloaded: worker_downloaded,
                        speed_bps: worker_speed,
                        status,
                        retry_count: 0,
                    });
                }
            } else {
                let fallback_end = if stats.total_bytes > 0 {
                    stats.total_bytes
                } else {
                    stats.progress_bytes
                };
                workers.push(WorkerTelemetry {
                    idx: 0,
                    start: 0,
                    end: fallback_end,
                    pos: stats.progress_bytes,
                    bytes_downloaded: stats.progress_bytes,
                    speed_bps: instant_bps,
                    status: if stats.finished {
                        WorkerStatus::Completed
                    } else if instant_bps > 0 {
                        WorkerStatus::Downloading
                    } else {
                        WorkerStatus::Connecting
                    },
                    retry_count: 0,
                });
            }

            let eff_speed = if is_initializing {
                0
            } else {
                (instant_bps / 8).max(dl_speed)
            };
            let is_download_complete =
                stats.total_bytes > 0 && (stats.finished || progress_pct >= 100.0);
            let has_seed_policy =
                power_cfg.seed_ratio_limit.is_some() || power_cfg.seed_time_limit_mins.is_some();

            if is_download_complete && has_seed_policy && seeding_started_at.is_none() {
                seeding_started_at = Some(std::time::Instant::now());
            }

            let seeding_elapsed_secs = seeding_started_at
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);
            let current_ratio = if stats.total_bytes > 0 {
                stats.uploaded_bytes as f64 / stats.total_bytes as f64
            } else {
                0.0
            };

            let should_stop_seed = if is_download_complete && has_seed_policy {
                let ratio_reached = power_cfg
                    .seed_ratio_limit
                    .map(|lim| lim <= 0.0 || current_ratio >= lim)
                    .unwrap_or(false);
                let time_reached = power_cfg
                    .seed_time_limit_mins
                    .map(|mins| seeding_elapsed_secs >= mins.saturating_mul(60))
                    .unwrap_or(false);
                ratio_reached || time_reached
            } else {
                false
            };

            let current_phase = if is_initializing {
                Some("Verifying".to_string())
            } else if stats.total_bytes == 0 {
                Some("Resolving".to_string())
            } else if is_download_complete && has_seed_policy && !should_stop_seed {
                Some("Seeding".to_string())
            } else if is_download_complete {
                Some("Completed".to_string())
            } else {
                Some("Downloading".to_string())
            };

            let decision_reason = if is_download_complete && has_seed_policy && !should_stop_seed {
                format!(
                    "Seeding Swarm (ratio {:.2}, {}s elapsed)",
                    current_ratio, seeding_elapsed_secs
                )
            } else if power_cfg.sequential && !is_initializing {
                format!("[Sequential] {}", decision_reason)
            } else {
                decision_reason
            };

            let eta_seconds =
                if !is_initializing && eff_speed > 0 && stats.total_bytes > stats.progress_bytes {
                    let raw_eta = (stats.total_bytes - stats.progress_bytes) / eff_speed;
                    eta_sample_count += 1;
                    if eta_sample_count < 3 {
                        None
                    } else {
                        let smoothed = match smoothed_eta_sec {
                            None => raw_eta,
                            Some(prev) => (0.1 * raw_eta as f64 + 0.9 * prev as f64) as u64,
                        };
                        smoothed_eta_sec = Some(smoothed);
                        smoothed_eta_sec
                    }
                } else {
                    if !is_initializing {
                        eta_sample_count = 0;
                        smoothed_eta_sec = None;
                    }
                    None
                };

            if let Some(ref cb) = telemetry_cb {
                cb(AdaptiveTelemetry {
                    download_id: task.id.to_string(),
                    total_bytes,
                    downloaded_bytes: stats.progress_bytes,
                    progress_pct,
                    instant_throughput_bps: instant_bps,
                    rolling_throughput_bps: rolling_bps,
                    upload_throughput_bps,
                    uploaded_bytes: stats.uploaded_bytes,
                    active_connections: if is_initializing { 0 } else { live_conns },
                    target_connections: task.config.max_connections,
                    live_workers: num_workers,
                    max_allowed_connections: task.config.max_connections.max(32),
                    mode,
                    eta_seconds,
                    decision_reason,
                    phase: current_phase,
                    workers,
                    updated_at: Utc::now(),
                    ..Default::default()
                });
            }

            // Complete when metadata is resolved, download finished, and any seeding policy is satisfied
            if is_download_complete && (!has_seed_policy || should_stop_seed) {
                break;
            }
        }

        let _ = session.stop().await;

        // If target_file_path is a directory (multi-file torrent), resolve to the largest payload file
        if target_file_path.is_dir() {
            if let Some(bf) = find_largest_file_recursive(&target_file_path) {
                target_file_path = bf;
            }
        }

        Ok(target_file_path)
    }
    /// Parse a magnet URI into info hash, display name, and tracker list
    pub fn parse_magnet(uri: &str) -> Result<MagnetInfo, String> {
        let trimmed = uri.trim();
        let query_part = trimmed
            .strip_prefix("magnet:?")
            .ok_or_else(|| "Invalid magnet URI prefix".to_string())?;

        let mut info_hash_hex = None;
        let mut display_name = None;
        let mut trackers = Vec::new();
        let mut exact_topic = String::new();

        for pair in query_part.split('&') {
            if let Some(xt) = pair.strip_prefix("xt=") {
                exact_topic = xt.to_string();
                if let Some(hash) = xt.strip_prefix("urn:btih:") {
                    // Check if 40-char hex or 32-char base32
                    if hash.len() == 40 {
                        info_hash_hex = Some(hash.to_ascii_lowercase());
                    } else if hash.len() == 32 {
                        // Decode base32 to hex if needed
                        info_hash_hex = Some(hash.to_ascii_lowercase());
                    }
                }
            } else if let Some(dn) = pair.strip_prefix("dn=") {
                let decoded = urlencoding::decode(dn).unwrap_or(std::borrow::Cow::Borrowed(dn));
                display_name = Some(decoded.to_string());
            } else if let Some(tr) = pair.strip_prefix("tr=") {
                let decoded = urlencoding::decode(tr).unwrap_or(std::borrow::Cow::Borrowed(tr));
                trackers.push(decoded.to_string());
            }
        }

        let hash = info_hash_hex
            .ok_or_else(|| "Magnet URI missing valid xt=urn:btih parameter".to_string())?;

        Ok(MagnetInfo {
            info_hash_hex: hash,
            display_name,
            trackers,
            exact_topic,
        })
    }

    /// Build a standard 68-byte BitTorrent peer wire handshake buffer
    pub fn build_peer_handshake(info_hash_bytes: &[u8; 20], peer_id_bytes: &[u8; 20]) -> [u8; 68] {
        let mut packet = [0u8; 68];
        packet[0] = 19; // length of protocol string
        packet[1..20].copy_from_slice(b"BitTorrent protocol");
        packet[20..28].copy_from_slice(&[0u8; 8]); // 8 reserved extension bytes
        packet[28..48].copy_from_slice(info_hash_bytes);
        packet[48..68].copy_from_slice(peer_id_bytes);
        packet
    }

    /// Parse Bencoded binary data into a structured BencodeValue
    pub fn decode_bencode(bytes: &[u8]) -> Result<BencodeValue, String> {
        let mut pos = 0;
        Self::parse_bencode_recursive(bytes, &mut pos)
    }

    fn parse_bencode_recursive(bytes: &[u8], pos: &mut usize) -> Result<BencodeValue, String> {
        if *pos >= bytes.len() {
            return Err("Unexpected end of bencoded stream".to_string());
        }

        match bytes[*pos] {
            b'i' => {
                *pos += 1;
                let start = *pos;
                while *pos < bytes.len() && bytes[*pos] != b'e' {
                    *pos += 1;
                }
                if *pos >= bytes.len() {
                    return Err("Unterminated bencode integer".to_string());
                }
                let int_str = std::str::from_utf8(&bytes[start..*pos])
                    .map_err(|e| format!("Invalid integer bytes: {}", e))?;
                *pos += 1; // skip 'e'
                let val = int_str
                    .parse::<i64>()
                    .map_err(|e| format!("Bad integer: {}", e))?;
                Ok(BencodeValue::Integer(val))
            }
            b'l' => {
                *pos += 1;
                let mut list = Vec::new();
                while *pos < bytes.len() && bytes[*pos] != b'e' {
                    list.push(Self::parse_bencode_recursive(bytes, pos)?);
                }
                if *pos >= bytes.len() {
                    return Err("Unterminated bencode list".to_string());
                }
                *pos += 1; // skip 'e'
                Ok(BencodeValue::List(list))
            }
            b'd' => {
                *pos += 1;
                let mut dict = BTreeMap::new();
                while *pos < bytes.len() && bytes[*pos] != b'e' {
                    let key = match Self::parse_bencode_recursive(bytes, pos)? {
                        BencodeValue::ByteString(s) => s,
                        _ => return Err("Bencode dictionary key must be a byte string".to_string()),
                    };
                    let val = Self::parse_bencode_recursive(bytes, pos)?;
                    dict.insert(key, val);
                }
                if *pos >= bytes.len() {
                    return Err("Unterminated bencode dict".to_string());
                }
                *pos += 1; // skip 'e'
                Ok(BencodeValue::Dict(dict))
            }
            b'0'..=b'9' => {
                let start = *pos;
                while *pos < bytes.len() && bytes[*pos] >= b'0' && bytes[*pos] <= b'9' {
                    *pos += 1;
                }
                if *pos >= bytes.len() || bytes[*pos] != b':' {
                    return Err("Expected ':' delimiter in byte string length".to_string());
                }
                let len_str = std::str::from_utf8(&bytes[start..*pos])
                    .map_err(|e| format!("Invalid length: {}", e))?;
                let len = len_str
                    .parse::<usize>()
                    .map_err(|e| format!("Bad length: {}", e))?;
                *pos += 1; // skip ':'

                if *pos + len > bytes.len() {
                    return Err("Byte string length exceeds buffer".to_string());
                }
                let slice = bytes[*pos..*pos + len].to_vec();
                *pos += len;
                Ok(BencodeValue::ByteString(slice))
            }
            other => Err(format!("Unexpected byte in bencode stream: {:#x}", other)),
        }
    }
}
