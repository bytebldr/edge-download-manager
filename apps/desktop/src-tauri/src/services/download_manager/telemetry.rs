use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use edm_core::adaptive::AdaptiveTelemetry;
use edm_ipc::DownloadState;
use edm_store::Store;
use uuid::Uuid;

use crate::features::PowerCountdownController;
use crate::ws_server::WsManager;

/// Emit a `downloads-changed` Tauri event to the frontend window if available.
pub(super) fn emit_downloads_changed(
    app_handle: &Arc<Mutex<Option<tauri::AppHandle>>>,
    id: &str,
    action: &str,
) {
    if let Ok(guard) = app_handle.lock() {
        if let Some(ref handle) = *guard {
            use tauri::Emitter;
            let _ = handle.emit(
                "downloads-changed",
                serde_json::json!({ "id": id, "action": action }),
            );
        }
    }
}

/// Register a task in `WsManager` (either `"queued"` or `"downloading"`), optionally broadcasting a snapshot.
pub(super) fn spawn_ws_register(
    rt_handle: &tokio::runtime::Handle,
    ws_manager: Arc<WsManager>,
    id: String,
    filename: String,
    save_dir: PathBuf,
    size_total: Option<u64>,
    status: &'static str,
    broadcast_snapshot: bool,
) {
    rt_handle.spawn(async move {
        ws_manager
            .register_task(
                id,
                filename,
                save_dir,
                size_total,
                status.into(),
                Some(Utc::now().timestamp_millis() as u64),
            )
            .await;
        if broadcast_snapshot {
            ws_manager.broadcast_snapshot().await;
        }
    });
}

/// Build the throttled telemetry callback for DB progress persistence and WS broadcasts.
pub(super) fn build_telemetry_callback(
    id: Uuid,
    ws_id: String,
    telem_target: Arc<Mutex<Option<AdaptiveTelemetry>>>,
    store: Store,
    ws_mgr: Arc<WsManager>,
) -> Arc<dyn Fn(AdaptiveTelemetry) + Send + Sync> {
    let last_db_bytes = Arc::new(AtomicU64::new(0));
    let last_db_time = Arc::new(Mutex::new(Instant::now()));
    let last_ws_time = Arc::new(Mutex::new(Instant::now()));

    Arc::new(move |telem: AdaptiveTelemetry| {
        if let Ok(mut lock) = telem_target.lock() {
            *lock = Some(telem.clone());
        }

        // Low-overhead throttled database persistence (every 1.5s or >= 1MB delta)
        let prev_bytes = last_db_bytes.load(Ordering::Relaxed);
        let should_update_db = {
            let mut inst = crate::lock_unpoisoned(&last_db_time);
            if inst.elapsed() >= Duration::from_millis(1500)
                || telem.downloaded_bytes.saturating_sub(prev_bytes) >= 1024 * 1024
            {
                *inst = Instant::now();
                true
            } else {
                false
            }
        };
        if should_update_db {
            last_db_bytes.store(telem.downloaded_bytes, Ordering::Relaxed);
            let _ = store.update_download_size_and_progress(
                id,
                telem.downloaded_bytes,
                telem.total_bytes,
            );
        }

        // Low-overhead throttled WebSocket progress broadcast (at most every 250ms)
        let should_update_ws = {
            let mut inst = crate::lock_unpoisoned(&last_ws_time);
            if inst.elapsed() >= Duration::from_millis(250) {
                *inst = Instant::now();
                true
            } else {
                false
            }
        };
        if should_update_ws {
            let ws_mgr_c = ws_mgr.clone();
            let id_c = ws_id.clone();
            let speed_bytes =
                (telem.rolling_throughput_bps / 8).max(telem.instant_throughput_bps / 8);
            let is_phase_done = telem.phase.as_deref() == Some("Completed")
                || (telem.phase.as_deref() != Some("Failed") && telem.progress_pct >= 100.0);
            let status_str = if is_phase_done {
                "completed"
            } else {
                "downloading"
            };
            tokio::spawn(async move {
                ws_mgr_c
                    .update_task_progress(
                        &id_c,
                        telem.downloaded_bytes,
                        telem.total_bytes,
                        speed_bytes,
                        status_str,
                    )
                    .await;
            });
        }
    })
}

/// Map `PackExecutor::execute` output into final `DownloadState`, optional error message, verified size, and actual final path.
pub(super) fn classify_execution_result(
    result: Result<PathBuf, String>,
) -> (DownloadState, Option<String>, Option<u64>, Option<PathBuf>) {
    match result {
        Ok(final_path) => {
            let s = if final_path.is_dir() {
                let mut dir_total = 0u64;
                if let Ok(entries) = std::fs::read_dir(&final_path) {
                    for entry in entries.flatten() {
                        dir_total += std::fs::metadata(entry.path())
                            .map(|m| m.len())
                            .unwrap_or(0);
                    }
                }
                dir_total
            } else {
                std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0)
            };
            if s > 0 {
                (DownloadState::Completed, None, Some(s), Some(final_path))
            } else {
                (
                    DownloadState::Failed,
                    Some("Download failed: 0 bytes received or merge failed".to_string()),
                    None,
                    None,
                )
            }
        }
        Err(ref e) if e.contains("cancelled") => (DownloadState::Paused, None, None, None),
        Err(ref e)
            if !e.contains("WAF")
                && (e.contains("401")
                    || e.contains("403")
                    || e.contains("410")
                    || e.contains("expired")) =>
        {
            (
                DownloadState::LinkExpired,
                Some(
                    "Download link expired (HTTP 401/403/410). Refresh link to resume.".to_string(),
                ),
                None,
                None,
            )
        }
        Err(ref e) if e.contains("429") => (
            DownloadState::Failed,
            Some("HTTP 429 Rate Limited".to_string()),
            None,
            None,
        ),
        Err(ref e) if e.contains("stalled") => (
            DownloadState::Failed,
            Some("Connection Stalled".to_string()),
            None,
            None,
        ),
        Err(e) => (DownloadState::Failed, Some(e), None, None),
    }
}

/// Persist final state/history, apply MOTW, notify user, trigger post-queue power action, and broadcast WS update.
#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_download_completion(
    id: Uuid,
    ws_id: &str,
    url: &str,
    filename: &str,
    save_path: &PathBuf,
    actual_final_path: Option<PathBuf>,
    referer: Option<&str>,
    effective_size: Option<u64>,
    final_state: DownloadState,
    error_msg: Option<String>,
    verified_size: Option<u64>,
    store: &Store,
    ws_mgr: &Arc<WsManager>,
    power_ctrl: &Arc<PowerCountdownController>,
    app_handle_opt: Option<tauri::AppHandle>,
) {
    // Guard against post-deletion resurrection: only update store if the task still exists
    let task_still_exists = store
        .list_all_downloads()
        .map(|list| list.iter().any(|d| d.id == id))
        .unwrap_or(false);

    if !task_still_exists {
        return;
    }

    let _ = store.update_download_state(id, final_state, error_msg.as_deref());
    if let Some(vs) = verified_size {
        let _ = store.update_download_size_and_progress(id, vs, Some(vs));
    }

    if final_state == DownloadState::Completed {
        let mut full_path = actual_final_path.unwrap_or_else(|| save_path.join(filename));
        let lower_url = url.to_ascii_lowercase();
        let is_torrent = lower_url.starts_with("magnet:")
            || lower_url
                .split('?')
                .next()
                .unwrap_or("")
                .ends_with(".torrent");
        if is_torrent && full_path.is_dir() {
            if let Some((zip_path, zip_size)) = maybe_zip_multifile_torrent(&full_path).await {
                full_path = zip_path;
                if let Some(zip_name) = full_path.file_name().and_then(|n| n.to_str()) {
                    let _ = store.update_download_filename(id, zip_name);
                }
                let _ = store.update_download_size_and_progress(id, zip_size, Some(zip_size));
            }
        }
        let display_name = full_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(filename);
        let _ = edm_core::security::apply_mark_of_the_web(&full_path, Some(url));
        let _ = store.purge_encrypted_cookies(id);
        let _ = crate::os::notifications::send_system_notification_internal(
            "EDM Download Complete",
            &format!("{} has finished downloading.", display_name),
            Some(&full_path.to_string_lossy()),
        );
        if let Ok(records) = store.list_all_downloads() {
            if let Some(r) = records.into_iter().find(|d| d.id == id) {
                let _ = store.add_to_history(&r, Utc::now());
            }
        }
        if let Ok(queues) = store.list_queues() {
            if let Some(def_q) = queues.into_iter().find(|q| q.id == "default") {
                if def_q.post_action != "none" {
                    let all_records = store.list_all_downloads().unwrap_or_default();
                    let all_done = all_records
                        .iter()
                        .all(|d| d.state == DownloadState::Completed);
                    let no_errors = all_records.iter().all(|d| {
                        d.state != DownloadState::Failed && d.state != DownloadState::LinkExpired
                    });
                    if all_done && no_errors && !all_records.is_empty() {
                        power_ctrl.start_countdown(
                            &def_q.post_action,
                            ws_mgr.clone(),
                            app_handle_opt,
                        );
                    }
                }
            }
        }
    } else if final_state == DownloadState::LinkExpired {
        ws_mgr
            .broadcast_refresh_link(&id.to_string(), referer, filename, effective_size)
            .await;
    }

    let final_status_str = match final_state {
        DownloadState::Completed => "completed".to_string(),
        DownloadState::Paused => "paused".to_string(),
        DownloadState::LinkExpired => "needs_fresh_link".to_string(),
        _ => "failed".to_string(),
    };
    let (final_done, final_total) = match store.list_all_downloads() {
        Ok(records) => records
            .into_iter()
            .find(|d| d.id == id)
            .map(|d| (d.size_done, d.size_total))
            .unwrap_or((0, effective_size)),
        Err(_) => (0, effective_size),
    };
    let final_total = verified_size.or(final_total);
    let final_done = verified_size.unwrap_or(final_done);

    let ws_mgr_fin = ws_mgr.clone();
    let ws_id_fin = ws_id.to_string();
    tokio::spawn(async move {
        if final_state == DownloadState::Failed {
            ws_mgr_fin.update_task_error(&ws_id_fin, error_msg).await;
        } else {
            ws_mgr_fin
                .update_task_progress(&ws_id_fin, final_done, final_total, 0, &final_status_str)
                .await;
        }
        ws_mgr_fin.broadcast_snapshot().await;
    });
}

fn count_files_in_dir(dir: &std::path::Path) -> usize {
    let mut count = 0usize;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                count += count_files_in_dir(&p);
            } else if p.is_file() {
                count += 1;
            }
            if count > 1 {
                return count;
            }
        }
    }
    count
}

async fn maybe_zip_multifile_torrent(dir: &std::path::Path) -> Option<(PathBuf, u64)> {
    let dir_owned = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        if count_files_in_dir(&dir_owned) <= 1 {
            return None;
        }
        let parent = dir_owned.parent()?;
        let dir_name = dir_owned.file_name()?.to_str()?;
        let zip_path = parent.join(format!("{}.zip", dir_name));
        let mut cmd = std::process::Command::new("tar");
        cmd.args(["-a", "-c", "-f"])
            .arg(&zip_path)
            .arg("-C")
            .arg(parent)
            .arg(dir_name);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let status = cmd.status().ok()?;
        if !status.success() {
            let _ = std::fs::remove_file(&zip_path);
            return None;
        }
        let meta = std::fs::metadata(&zip_path).ok()?;
        if meta.len() == 0 {
            let _ = std::fs::remove_file(&zip_path);
            return None;
        }
        let _ = std::fs::remove_dir_all(&dir_owned);
        Some((zip_path, meta.len()))
    })
    .await
    .ok()
    .flatten()
}
