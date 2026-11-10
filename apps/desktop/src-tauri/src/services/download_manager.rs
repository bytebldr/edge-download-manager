use chrono::Utc;
use edm_core::adaptive::{AdaptiveConfig, AdaptiveDownloadTask, AdaptiveTelemetry};
use edm_ipc::DownloadState;
use edm_store::DownloadRecord;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::state::{ActiveTaskHandle, AppState};

#[allow(clippy::too_many_arguments)]
pub fn execute_download(
    state: &AppState,
    id: Uuid,
    url: String,
    filename: Option<String>,
    save_dir: Option<PathBuf>,
    max_connections: Option<u32>,
    size_total: Option<u64>,
    cookies: Option<String>,
    referer: Option<String>,
    user_agent: Option<String>,
) -> Result<Uuid, String> {
    let lc = state.lifecycle.clone();
    state.rt_handle.spawn(async move {
        lc.record_activity().await;
    });
    crate::os::validate_url_security(&url)?;
    let id_str = id.to_string();

    let pack_target = edm_core::FeaturePackManager::classify_target(&url);

    let raw_name = match filename {
        Some(ref f) if !f.trim().is_empty() && f != "download" && f != "download.bin" => f.clone(),
        _ => {
            if let Some(ref suggested) = pack_target.suggested_filename {
                suggested.clone()
            } else {
                let path_part = url
                    .split('/')
                    .next_back()
                    .unwrap_or("download.bin")
                    .split('?')
                    .next()
                    .unwrap_or("download.bin");
                if path_part.is_empty() || path_part == "download" || path_part == "download.bin" {
                    if let Ok(parsed) = url::Url::parse(&url) {
                        let mut found = None;
                        for (k, v) in parsed.query_pairs() {
                            if (k == "filename" || k == "file" || k == "title" || k == "name")
                                && !v.trim().is_empty()
                            {
                                found = Some(v.to_string());
                                break;
                            }
                        }
                        found.unwrap_or_else(|| "download.bin".to_string())
                    } else {
                        "download.bin".to_string()
                    }
                } else {
                    path_part.to_string()
                }
            }
        }
    };
    let clean_name = edm_core::sanitizer::Sanitizer::sanitize_filename(&raw_name);

    let save_path = save_dir.unwrap_or_else(|| {
        std::env::var("USERPROFILE")
            .map(|p| PathBuf::from(p).join("Downloads"))
            .unwrap_or_else(|_| std::env::temp_dir())
    });

    // Security Guard: Prevent writing to sensitive system directories or Startup folders
    let path_str = save_path.to_string_lossy().to_lowercase();
    if path_str.contains(r"\windows\system32")
        || path_str.contains(r"\windows\syswow64")
        || path_str.contains(r"\start menu\programs\startup")
        || path_str.contains(r"\microsoft\windows\start menu")
    {
        return Err(
            "Security violation: Target download directory is a restricted Windows system location"
                .into(),
        );
    }

    let resolved_path = edm_core::sanitizer::Sanitizer::resolve_duplicate(&save_path, &clean_name);
    let final_clean_name = resolved_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or(clean_name);

    if let Some(warn_msg) = edm_core::security::check_double_extension(&final_clean_name) {
        tracing::warn!("[SECURITY] {}", warn_msg);
    }

    let mut config = AdaptiveConfig::default();
    if let Some(conns) = max_connections {
        config.max_connections = conns;
    }

    // UI-01: Apply persistent settings from store (connections, segmentation, retries, proxy)
    let mut proxy_config: Option<reqwest::Proxy> = None;
    let mut req_timeout_secs = 45u64;

    if let Ok(Some(json_str)) = state.store.get_setting("app_settings") {
        if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(dl) = settings.get("downloads") {
                if max_connections.is_none() {
                    if let Some(conns) = dl.get("maxConnectionsPerFile").and_then(|v| v.as_u64()) {
                        config.max_connections = (conns.clamp(1, 64)) as u32;
                    }
                }
                if let Some(min_mb) = dl
                    .get("minFileSizeForSegmentationMb")
                    .and_then(|v| v.as_u64())
                {
                    config.min_chunk_size = min_mb.max(1) * 1024 * 1024;
                }
                if let Some(retries) = dl.get("maxRetries").and_then(|v| v.as_u64()) {
                    config.max_worker_retries = (retries.clamp(1, 20)) as u32;
                }
            }
            if let Some(net) = settings.get("network") {
                if let Some(t) = net.get("connectionTimeoutSeconds").and_then(|v| v.as_u64()) {
                    if t > 0 {
                        req_timeout_secs = t;
                    }
                }
                let proxy_mode = net
                    .get("proxyMode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("system");
                if proxy_mode == "manual" {
                    let proto = net
                        .get("proxyProtocol")
                        .and_then(|v| v.as_str())
                        .unwrap_or("http");
                    let host = net.get("proxyHost").and_then(|v| v.as_str()).unwrap_or("");
                    let port = net
                        .get("proxyPort")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(8080);
                    if !host.trim().is_empty() {
                        let p_url = format!("{}://{}:{}", proto, host.trim(), port);
                        if let Ok(p) = reqwest::Proxy::all(&p_url) {
                            proxy_config = Some(p);
                        }
                    }
                }
            }
        }
    }

    let cancelled = Arc::new(AtomicBool::new(false));
    let telemetry_store = Arc::new(Mutex::new(None));

    let handle = ActiveTaskHandle {
        cancelled: cancelled.clone(),
        telemetry: telemetry_store.clone(),
    };

    state.tasks.lock().unwrap().insert(id_str.clone(), handle);

    let ext = final_clean_name
        .split('.')
        .next_back()
        .unwrap_or("")
        .to_lowercase();
    let category = if pack_target.is_p2p {
        "Compressed"
    } else if pack_target.is_stream || ["mp4", "mkv", "webm", "avi", "mov"].contains(&ext.as_str())
    {
        "Video"
    } else if ["mp3", "flac", "wav", "aac"].contains(&ext.as_str()) {
        "Music"
    } else if ["zip", "rar", "7z", "tar", "gz", "iso"].contains(&ext.as_str()) {
        "Compressed"
    } else if ["pdf", "doc", "docx", "txt"].contains(&ext.as_str()) {
        "Documents"
    } else {
        "Programs"
    };

    let effective_size = size_total.or(pack_target.estimated_size);

    if let Some(sz) = effective_size {
        if let Err(e) = edm_core::security::check_disk_space(&save_path, sz) {
            tracing::warn!("[DISK SPACE] {}", e);
        }
    }

    if let Some(ref c) = cookies {
        if let Ok(enc) = edm_core::security::encrypt_dpapi(c.as_bytes()) {
            let _ = state.store.save_encrypted_cookies(id, &enc);
        }
    }

    let record = DownloadRecord {
        id,
        url: url.clone(),
        final_url: None,
        filename: final_clean_name.clone(),
        save_dir: save_path.to_string_lossy().to_string(),
        category: category.into(),
        state: DownloadState::Downloading,
        size_total: effective_size,
        size_done: 0,
        resumable: true,
        etag: None,
        last_modified: None,
        content_type: None,
        referer: referer.clone(),
        user_agent: user_agent.clone(),
        connections: config.max_connections,
        created_at: Utc::now(),
        error_msg: None,
    };
    let _ = state.store.insert_download(&record);

    let ws_mgr = state.ws_manager.clone();
    let ws_id = id_str.clone();
    let ws_filename = final_clean_name.clone();
    let ws_savedir = save_path.clone();
    let ws_total = effective_size;
    state.rt_handle.spawn(async move {
        ws_mgr
            .register_task(
                ws_id,
                ws_filename,
                ws_savedir,
                ws_total,
                "downloading".into(),
            )
            .await;
    });

    let task = AdaptiveDownloadTask {
        id,
        url: url.clone(),
        filename: final_clean_name.clone(),
        save_dir: save_path.clone(),
        config,
        limiter: Some((*state.global_limiter).clone()),
        cancelled,
        cookies,
        referer: referer.clone(),
        user_agent,
    };

    let engine = state.engine.clone();
    let telem_target = telemetry_store.clone();
    let store_clone = state.store.clone();
    let store_cb = state.store.clone();
    let ws_mgr_cb = state.ws_manager.clone();
    let ws_id_cb = id_str.clone();
    let tasks_map = state.tasks.clone();
    let save_path_fin = save_path.clone();
    let final_clean_name_fin = final_clean_name.clone();
    let url_fin = url.clone();
    let referer_fin = referer.clone();
    let power_ctrl = state.power_countdown.clone();
    let app_handle_opt = state.app_handle.lock().unwrap().clone();

    state.rt_handle.spawn(async move {
        let ws_mgr_tick = ws_mgr_cb.clone();
        let ws_id_tick = ws_id_cb.clone();
        let last_db_bytes = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let last_db_time = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
        let last_ws_time = Arc::new(std::sync::Mutex::new(std::time::Instant::now()));

        let cb = Arc::new(move |telem: AdaptiveTelemetry| {
            if let Ok(mut lock) = telem_target.lock() {
                *lock = Some(telem.clone());
            }

            // Low-overhead throttled database persistence (every 1.5s or >= 1MB delta)
            let prev_bytes = last_db_bytes.load(Ordering::Relaxed);
            let should_update_db = {
                let mut inst = last_db_time.lock().unwrap();
                if inst.elapsed() >= std::time::Duration::from_millis(1500)
                    || telem.downloaded_bytes.saturating_sub(prev_bytes) >= 1024 * 1024
                {
                    *inst = std::time::Instant::now();
                    true
                } else {
                    false
                }
            };
            if should_update_db {
                last_db_bytes.store(telem.downloaded_bytes, Ordering::Relaxed);
                let _ = store_cb.update_download_size_and_progress(id, telem.downloaded_bytes, telem.total_bytes);
            }

            // Low-overhead throttled WebSocket progress broadcast (at most every 250ms)
            let should_update_ws = {
                let mut inst = last_ws_time.lock().unwrap();
                if inst.elapsed() >= std::time::Duration::from_millis(250) {
                    *inst = std::time::Instant::now();
                    true
                } else {
                    false
                }
            };
            if should_update_ws {
                let ws_mgr = ws_mgr_tick.clone();
                let id_c = ws_id_tick.clone();
                let speed_bytes =
                    (telem.rolling_throughput_bps / 8).max(telem.instant_throughput_bps / 8);
                tokio::spawn(async move {
                    ws_mgr
                        .update_task_progress(
                            &id_c,
                            telem.downloaded_bytes,
                            telem.total_bytes,
                            speed_bytes,
                            "downloading",
                        )
                        .await;
                });
            }
        });
        let mut client_builder = Client::builder()
            .timeout(std::time::Duration::from_secs(req_timeout_secs))
            .redirect(reqwest::redirect::Policy::limited(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36 Edg/135.0.0.0");
        if let Some(p) = proxy_config {
            client_builder = client_builder.proxy(p);
        }
        let client = client_builder.build().unwrap_or_default();
        let result = edm_core::PackExecutor::execute(task, engine, client, Some(cb)).await;
        let (final_state, error_msg, verified_size) = match result {
            Ok(ref final_path) => {
                let s = if final_path.is_dir() {
                    let mut dir_total = 0u64;
                    if let Ok(entries) = std::fs::read_dir(final_path) {
                        for entry in entries.flatten() {
                            dir_total += std::fs::metadata(entry.path()).map(|m| m.len()).unwrap_or(0);
                        }
                    }
                    dir_total
                } else {
                    std::fs::metadata(final_path).map(|m| m.len()).unwrap_or(0)
                };
                if s > 0 {
                    (DownloadState::Completed, None, Some(s))
                } else {
                    (DownloadState::Failed, Some("Download failed: 0 bytes received or merge failed"), None)
                }
            }
            Err(ref e) if e.contains("cancelled") => (DownloadState::Paused, None, None),
            Err(ref e)
                if e.contains("401")
                    || e.contains("403")
                    || e.contains("410")
                    || e.contains("expired") =>
            {
                (
                    DownloadState::LinkExpired,
                    Some("Download link expired (HTTP 401/403/410). Refresh link to resume."),
                    None,
                )
            }
            Err(ref e) if e.contains("429") => {
                (DownloadState::Failed, Some("HTTP 429 Rate Limited"), None)
            }
            Err(ref e) if e.contains("stalled") => {
                (DownloadState::Failed, Some("Connection Stalled"), None)
            }
            Err(ref e) => (DownloadState::Failed, Some(e.as_str()), None),
        };

        // Guard against post-deletion resurrection: only update store if the task still exists
        let task_still_exists = store_clone
            .list_all_downloads()
            .map(|list| list.iter().any(|d| d.id == id))
            .unwrap_or(false);

        if task_still_exists {
            let _ = store_clone.update_download_state(id, final_state, error_msg);
            if let Some(vs) = verified_size {
                let _ = store_clone.update_download_size_and_progress(id, vs, Some(vs));
            }

            if final_state == DownloadState::Completed {
                let full_path = save_path_fin.join(&final_clean_name_fin);
                let _ = edm_core::security::apply_mark_of_the_web(&full_path, Some(&url_fin));
                let _ = store_clone.purge_encrypted_cookies(id);
                if let Ok(records) = store_clone.list_all_downloads() {
                    if let Some(r) = records.into_iter().find(|d| d.id == id) {
                        let _ = store_clone.add_to_history(&r, Utc::now());
                    }
                }
                if let Ok(queues) = store_clone.list_queues() {
                    if let Some(def_q) = queues.into_iter().find(|q| q.id == "default") {
                        if def_q.post_action != "none" {
                            let all_records = store_clone.list_all_downloads().unwrap_or_default();
                            let all_done = all_records.iter().all(|d| d.state == DownloadState::Completed);
                            let no_errors = all_records.iter().all(|d| d.state != DownloadState::Failed && d.state != DownloadState::LinkExpired);
                            if all_done && no_errors && !all_records.is_empty() {
                                power_ctrl.start_countdown(&def_q.post_action, ws_mgr_cb.clone(), app_handle_opt);
                            }
                        }
                    }
                }
            } else if final_state == DownloadState::LinkExpired {
                ws_mgr_cb
                    .broadcast_refresh_link(
                        &id.to_string(),
                        referer_fin.as_deref(),
                        &final_clean_name_fin,
                        effective_size,
                    )
                    .await;
            }

            let final_status_str = match &final_state {
                DownloadState::Completed => "completed".to_string(),
                DownloadState::Paused => "paused".to_string(),
                DownloadState::LinkExpired => "needs_fresh_link".to_string(),
                _ => "failed".to_string(),
            };
            let ws_mgr_fin = ws_mgr_cb.clone();
            let ws_id_fin = ws_id_cb.clone();
            let error_msg_owned = error_msg.map(String::from);
            let (final_done, final_total) = match store_clone.list_all_downloads() {
                Ok(records) => records
                    .into_iter()
                    .find(|d| d.id == id)
                    .map(|d| (d.size_done, d.size_total))
                    .unwrap_or((0, effective_size)),
                Err(_) => (0, effective_size),
            };
            let final_total = verified_size.or(final_total);
            let final_done = verified_size.unwrap_or(final_done);

            tokio::spawn(async move {
                if final_state == DownloadState::Failed {
                    ws_mgr_fin
                        .update_task_error(&ws_id_fin, error_msg_owned)
                        .await;
                } else {
                    ws_mgr_fin
                        .update_task_progress(
                            &ws_id_fin,
                            final_done,
                            final_total,
                            0,
                            &final_status_str,
                        )
                        .await;
                }
            });
        }

        // Evict active task handle and telemetry buffers from live memory
        tasks_map.lock().unwrap().remove(&ws_id_cb);
    });

    Ok(id)
}

pub async fn remove_download_task(
    state: &AppState,
    id_str: &str,
    delete_file: bool,
) -> Result<(), String> {
    // 1. Cancel and evict in-memory active task
    {
        let mut tasks = state.tasks.lock().unwrap();
        if let Some(t) = tasks.remove(id_str) {
            t.cancelled.store(true, Ordering::SeqCst);
        }
    }

    // Yield control briefly to allow any closing file handles on Windows to release
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 2. Lookup record in store for file paths if delete_file requested
    let id_uuid = Uuid::parse_str(id_str).map_err(|e| format!("Invalid UUID: {}", e))?;
    let maybe_record = state
        .store
        .list_all_downloads()
        .ok()
        .and_then(|records| records.into_iter().find(|d| d.id == id_uuid));

    // 3. Delete from persistent SQLite database
    let _ = state.store.delete_download(id_uuid);

    // 4. Remove from ws_manager live memory and broadcast task_removed immediately
    state.ws_manager.broadcast_task_removed(id_str).await;

    // 5. If delete_file, send target file and .edmpart to Recycle Bin with retry
    if delete_file {
        if let Some(r) = maybe_record {
            let base_dir = PathBuf::from(r.save_dir);
            let target_file = base_dir.join(&r.filename);
            let part_file = base_dir.join(format!("{}.edmpart", r.filename));
            if target_file.exists() {
                let _ = crate::ws_server::send_to_recycle_bin(&target_file);
            }
            if part_file.exists() {
                // Retry sending to recycle bin up to 3 times in case file handle was releasing
                for _ in 0..3 {
                    if !part_file.exists() {
                        break;
                    }
                    if crate::ws_server::send_to_recycle_bin(&part_file).is_ok() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    }

    // 6. Notify desktop frontend window
    if let Ok(guard) = state.app_handle.lock() {
        if let Some(ref handle) = *guard {
            use tauri::Emitter;
            let _ = handle.emit(
                "downloads-changed",
                serde_json::json!({ "id": id_str, "action": "removed" }),
            );
        }
    }

    Ok(())
}
