mod config;
mod telemetry;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use edm_core::adaptive::AdaptiveDownloadTask;
use edm_ipc::DownloadState;
use edm_store::DownloadRecord;
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
) -> Result<(Uuid, bool), String> {
    execute_download_with_selected_files(
        state,
        id,
        url,
        filename,
        save_dir,
        max_connections,
        size_total,
        cookies,
        referer,
        user_agent,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn execute_download_with_selected_files(
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
    selected_files: Option<Vec<usize>>,
) -> Result<(Uuid, bool), String> {
    let lc = state.lifecycle.clone();
    state.rt_handle.spawn(async move {
        lc.record_activity().await;
    });
    crate::os::validate_url_security(&url)?;
    let id_str = id.to_string();

    let pack_target = edm_core::FeaturePackManager::classify_target(&url);
    let settings_val = config::load_app_settings(state);

    let existing_record = state
        .store
        .list_all_downloads()
        .ok()
        .and_then(|records| records.into_iter().find(|r| r.id == id));
    let is_resuming = existing_record.is_some();

    let (final_clean_name, save_path, category) = config::resolve_destination(
        settings_val.as_ref(),
        &url,
        filename.as_deref(),
        pack_target.suggested_filename.as_deref(),
        save_dir.as_ref(),
        pack_target.is_p2p,
        pack_target.is_stream,
        is_resuming,
    )?;

    let ext = final_clean_name
        .split('.')
        .next_back()
        .unwrap_or("")
        .to_lowercase();
    let exec_cfg = config::build_execution_config(
        state,
        settings_val.as_ref(),
        max_connections,
        pack_target.is_p2p || ext == "torrent",
    );

    let effective_size = size_total.or(pack_target.estimated_size);

    // Save session cookies before queue check so queued tasks retain cookies,
    // and decrypt stored cookies if none were passed directly (e.g. resume or queued execution).
    let mut cookies = cookies;
    if let Some(ref c) = cookies {
        if let Ok(enc) = edm_core::security::encrypt_dpapi(c.as_bytes()) {
            let _ = state.store.save_encrypted_cookies(id, &enc);
        }
    } else if let Ok(Some(enc)) = state.store.get_encrypted_cookies(id) {
        if let Ok(dec) = edm_core::security::decrypt_dpapi(&enc) {
            if let Ok(s) = String::from_utf8(dec) {
                cookies = Some(s);
            }
        }
    }

    let active_count = {
        let tasks = crate::lock_unpoisoned(&state.tasks);
        tasks
            .values()
            .filter(|h| !h.cancelled.load(Ordering::Relaxed))
            .count()
    };

    if active_count >= exec_cfg.max_active_downloads {
        let record = DownloadRecord {
            id,
            url: url.clone(),
            final_url: None,
            filename: final_clean_name.clone(),
            save_dir: save_path.to_string_lossy().to_string(),
            category: category.into(),
            state: DownloadState::Queued,
            size_total: effective_size,
            size_done: 0,
            resumable: true,
            etag: None,
            last_modified: None,
            content_type: None,
            referer: referer.clone(),
            user_agent: user_agent.clone(),
            connections: exec_cfg.adaptive.max_connections,
            created_at: Utc::now(),
            error_msg: None,
        };
        let _ = state.store.insert_download(&record);

        telemetry::spawn_ws_register(
            &state.rt_handle,
            state.ws_manager.clone(),
            id_str.clone(),
            final_clean_name,
            save_path,
            effective_size,
            "queued",
            true,
        );
        telemetry::emit_downloads_changed(&state.app_handle, &id_str, "queued");
        return Ok((id, false));
    }

    let cancelled = Arc::new(AtomicBool::new(false));
    let telemetry_store = Arc::new(Mutex::new(None));
    let handle = ActiveTaskHandle {
        cancelled: cancelled.clone(),
        telemetry: telemetry_store.clone(),
    };
    crate::lock_unpoisoned(&state.tasks).insert(id_str.clone(), handle);

    if let Some(sz) = effective_size {
        if let Err(e) = edm_core::security::check_disk_space(&save_path, sz) {
            tracing::warn!("[DISK SPACE] {}", e);
        }
    }

    if let Some(ref existing) = existing_record {
        let seg_count = state.store.get_segments(id).map(|s| s.len()).unwrap_or(0);
        tracing::info!(
            "[RESUME] Resuming download {} ({}) with {} saved segment(s), size_done={}, etag={:?}, last_modified={:?}",
            id,
            final_clean_name,
            seg_count,
            existing.size_done,
            existing.etag,
            existing.last_modified
        );
    }

    let record = DownloadRecord {
        id,
        url: url.clone(),
        final_url: existing_record.as_ref().and_then(|r| r.final_url.clone()),
        filename: final_clean_name.clone(),
        save_dir: save_path.to_string_lossy().to_string(),
        category: category.into(),
        state: DownloadState::Downloading,
        size_total: effective_size.or_else(|| existing_record.as_ref().and_then(|r| r.size_total)),
        size_done: existing_record.as_ref().map(|r| r.size_done).unwrap_or(0),
        resumable: existing_record
            .as_ref()
            .map(|r| r.resumable)
            .unwrap_or(true),
        etag: existing_record.as_ref().and_then(|r| r.etag.clone()),
        last_modified: existing_record
            .as_ref()
            .and_then(|r| r.last_modified.clone()),
        content_type: existing_record
            .as_ref()
            .and_then(|r| r.content_type.clone()),
        referer: referer.clone(),
        user_agent: user_agent.clone(),
        connections: exec_cfg.adaptive.max_connections,
        created_at: existing_record
            .as_ref()
            .map(|r| r.created_at)
            .unwrap_or_else(Utc::now),
        error_msg: None,
    };
    let _ = state.store.insert_download(&record);

    telemetry::spawn_ws_register(
        &state.rt_handle,
        state.ws_manager.clone(),
        id_str.clone(),
        final_clean_name.clone(),
        save_path.clone(),
        effective_size,
        "downloading",
        false,
    );

    let client = exec_cfg.build_http_client();
    let task = AdaptiveDownloadTask {
        id,
        url: url.clone(),
        filename: final_clean_name.clone(),
        save_dir: save_path.clone(),
        config: exec_cfg.adaptive,
        limiter: Some((*state.global_limiter).clone()),
        cancelled,
        cookies,
        referer: referer.clone(),
        user_agent,
        selected_files,
        torrent_power: exec_cfg.torrent_power,
    };

    let engine = state.engine.clone();
    let store_clone = state.store.clone();
    let ws_mgr_cb = state.ws_manager.clone();
    let ws_id_cb = id_str.clone();
    let tasks_map = state.tasks.clone();
    let power_ctrl = state.power_countdown.clone();
    let app_handle_opt = crate::lock_unpoisoned(&state.app_handle).clone();
    let state_drain = state.clone();

    state.rt_handle.spawn(async move {
        let cb = telemetry::build_telemetry_callback(
            id,
            ws_id_cb.clone(),
            telemetry_store,
            store_clone.clone(),
            ws_mgr_cb.clone(),
        );
        let result = edm_core::PackExecutor::execute(task, engine, client, Some(cb)).await;
        let (final_state, error_msg, verified_size) = telemetry::classify_execution_result(result);

        telemetry::handle_download_completion(
            id,
            &ws_id_cb,
            &url,
            &final_clean_name,
            &save_path,
            referer.as_deref(),
            effective_size,
            final_state,
            error_msg,
            verified_size,
            &store_clone,
            &ws_mgr_cb,
            &power_ctrl,
            app_handle_opt,
        )
        .await;

        // Evict active task handle and telemetry buffers from live memory
        crate::lock_unpoisoned(&tasks_map).remove(&ws_id_cb);
        telemetry::emit_downloads_changed(&state_drain.app_handle, &ws_id_cb, "finished");

        // Auto-drain next queued tasks from the persistent database in FIFO order
        try_start_next_queued(&state_drain);
    });

    Ok((id, true))
}

pub fn try_start_next_queued(state: &AppState) {
    let settings_val = config::load_app_settings(state);
    let max_active = config::read_max_active_downloads(settings_val.as_ref());

    let active_count = {
        let tasks = crate::lock_unpoisoned(&state.tasks);
        tasks
            .values()
            .filter(|h| !h.cancelled.load(Ordering::Relaxed))
            .count()
    };

    let slots_available = max_active.saturating_sub(active_count);
    if slots_available == 0 {
        return;
    }

    if let Ok(records) = state.store.list_all_downloads() {
        let mut queued_items: Vec<_> = records
            .into_iter()
            .filter(|d| d.state == DownloadState::Queued)
            .collect();
        // FIFO order: oldest created first
        queued_items.sort_by_key(|d| d.created_at);

        for queued in queued_items.into_iter().take(slots_available) {
            let state_clone = state.clone();
            let decrypted_cookies = state
                .store
                .get_encrypted_cookies(queued.id)
                .ok()
                .flatten()
                .and_then(|enc| edm_core::security::decrypt_dpapi(&enc).ok())
                .and_then(|dec| String::from_utf8(dec).ok());

            state.rt_handle.spawn(async move {
                let _ = execute_download(
                    &state_clone,
                    queued.id,
                    queued.url,
                    Some(queued.filename),
                    Some(PathBuf::from(queued.save_dir)),
                    Some(queued.connections),
                    queued.size_total,
                    decrypted_cookies,
                    queued.referer,
                    queued.user_agent,
                );
            });
        }
    }
}

async fn delete_file_with_recycle_fallback(path: &Path) {
    if !path.exists() {
        return;
    }

    let mut removed = false;
    for attempt in 1..=3 {
        if !path.exists() {
            removed = true;
            break;
        }
        match crate::ws_server::send_to_recycle_bin(path) {
            Ok(()) => {
                tracing::info!(
                    "Moved file {:?} to Recycle Bin on attempt {}",
                    path,
                    attempt
                );
                removed = true;
                break;
            }
            Err(e) => {
                tracing::warn!(
                    "Attempt {} send_to_recycle_bin failed for {:?}: {}. Retrying...",
                    attempt,
                    path,
                    e
                );
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    if !removed && path.exists() {
        tracing::warn!(
            "Recycle Bin failed for {:?}. Falling back to permanent std::fs::remove_file",
            path
        );
        for attempt in 1..=3 {
            if !path.exists() {
                break;
            }
            if let Err(e) = std::fs::remove_file(path) {
                tracing::warn!(
                    "Permanent remove_file attempt {} failed for {:?}: {}",
                    attempt,
                    path,
                    e
                );
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            } else {
                tracing::info!("Permanently unlinked file {:?}", path);
                break;
            }
        }
    }
}

pub async fn remove_download_task(
    state: &AppState,
    id_str: &str,
    delete_file: bool,
) -> Result<(), String> {
    // 1. Cancel and evict in-memory active task
    {
        let mut tasks = crate::lock_unpoisoned(&state.tasks);
        if let Some(t) = tasks.remove(id_str) {
            t.cancelled.store(true, Ordering::SeqCst);
        }
    }

    // Yield control briefly to allow any closing file handles on Windows to release
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

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

    // 5. If delete_file, safely send target file and .edmpart to Recycle Bin with guaranteed fallback
    if delete_file {
        if let Some(r) = maybe_record {
            let base_dir = PathBuf::from(r.save_dir);
            let target_file = base_dir.join(&r.filename);
            let part_file = base_dir.join(format!("{}.edmpart", r.filename));
            delete_file_with_recycle_fallback(&target_file).await;
            delete_file_with_recycle_fallback(&part_file).await;
        }
    }

    // 6. Notify desktop frontend window
    telemetry::emit_downloads_changed(&state.app_handle, id_str, "removed");

    // 7. Auto-drain queued tasks now that a slot may have freed up
    try_start_next_queued(state);

    Ok(())
}
