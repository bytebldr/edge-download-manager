use edm_ipc::{DownloadState, ExtensionMessage};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{command, State};
use uuid::Uuid;

use crate::os::{open_file, open_folder};
use crate::services::download_manager::{execute_download_with_selected_files, remove_download_task};
use crate::services::extension_bridge::handle_extension_message;
use crate::state::AppState;

#[command]
pub async fn inspect_torrent_files(
    url: String,
    save_dir: Option<String>,
) -> Result<edm_core::packs::bittorrent::TorrentInspectionResult, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL or torrent path cannot be empty".into());
    }
    crate::os::validate_url_security(trimmed)?;
    let dir = save_dir
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    edm_core::packs::bittorrent::BitTorrentPack::inspect_torrent(trimmed, &dir).await
}

#[command]
pub async fn start_download(
    state: State<'_, AppState>,
    url: String,
    filename: Option<String>,
    save_dir: Option<String>,
    max_connections: Option<u32>,
    selected_files: Option<Vec<usize>>,
) -> Result<serde_json::Value, String> {
    let id = Uuid::new_v4();
    let save_path = save_dir.map(PathBuf::from);
    let (download_id, is_started) = execute_download_with_selected_files(
        &state,
        id,
        url,
        filename,
        save_path,
        max_connections,
        None,
        None,
        None,
        None,
        selected_files,
    )?;

    Ok(serde_json::json!({
        "status": if is_started { "started" } else { "queued" },
        "id": download_id.to_string()
    }))
}

#[command]
pub async fn list_downloads(
    state: State<'_, AppState>,
    filter: Option<String>,
) -> Result<serde_json::Value, String> {
    let reply = handle_extension_message(ExtensionMessage::ListDownloads { filter }, &state).await;
    serde_json::to_value(reply).map_err(|e| e.to_string())
}

#[command]
pub async fn resume_download(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    if let Ok(uuid) = Uuid::parse_str(&id) {
        let records = state.store.list_all_downloads().unwrap_or_default();
        if let Some(_) = records.into_iter().find(|d| d.id == uuid) {
            let _ = state
                .store
                .update_download_state(uuid, DownloadState::Queued, None);

            crate::services::download_manager::try_start_next_queued(&state);

            if let Ok(guard) = state.app_handle.lock() {
                if let Some(ref handle) = *guard {
                    use tauri::Emitter;
                    let _ = handle.emit(
                        "downloads-changed",
                        serde_json::json!({ "id": id, "action": "resumed" }),
                    );
                }
            }
            return Ok(serde_json::json!({ "status": "resumed", "id": id }));
        }
    }
    Err(format!("Task {} not found", id))
}

#[command]
pub fn cancel_download(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    {
        let mut tasks = state.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = tasks.remove(&id) {
            task.cancelled.store(true, Ordering::Relaxed);
        }
    }
    if let Ok(uuid) = Uuid::parse_str(&id) {
        let _ = state.store.update_download_state(
            uuid,
            DownloadState::Cancelled,
            Some("Cancelled by user"),
        );
    }
    crate::services::download_manager::try_start_next_queued(&state);

    if let Ok(guard) = state.app_handle.lock() {
        if let Some(ref handle) = *guard {
            use tauri::Emitter;
            let _ = handle.emit(
                "downloads-changed",
                serde_json::json!({ "id": id, "action": "cancelled" }),
            );
        }
    }
    Ok(serde_json::json!({ "status": "cancelled", "id": id }))
}

#[command]
pub async fn delete_download(
    state: State<'_, AppState>,
    id: String,
    delete_file: Option<bool>,
) -> Result<serde_json::Value, String> {
    remove_download_task(&state, &id, delete_file.unwrap_or(false)).await?;
    Ok(serde_json::json!({ "status": "deleted", "id": id }))
}

#[command]
pub fn open_file_path(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    if let Ok(uuid) = Uuid::parse_str(&id) {
        if let Ok(records) = state.store.list_all_downloads() {
            if let Some(r) = records.into_iter().find(|d| d.id == uuid) {
                open_file(&r.save_dir, &r.filename);
                return Ok(serde_json::json!({ "status": "opened" }));
            }
        }
    }
    Err("Not found".into())
}

#[command]
pub fn open_dir_path(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    if let Ok(uuid) = Uuid::parse_str(&id) {
        if let Ok(records) = state.store.list_all_downloads() {
            if let Some(r) = records.into_iter().find(|d| d.id == uuid) {
                open_folder(&r.save_dir, &r.filename);
                return Ok(serde_json::json!({ "status": "opened" }));
            }
        }
    }
    Err("Not found".into())
}

#[command]
pub fn pause_download(state: State<'_, AppState>, id: String) -> Result<serde_json::Value, String> {
    {
        let mut tasks = state.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = tasks.remove(&id) {
            task.cancelled.store(true, Ordering::Relaxed);
        }
    }
    if let Ok(uuid) = Uuid::parse_str(&id) {
        let _ = state
            .store
            .update_download_state(uuid, DownloadState::Paused, None);
    }
    crate::services::download_manager::try_start_next_queued(&state);

    if let Ok(guard) = state.app_handle.lock() {
        if let Some(ref handle) = *guard {
            use tauri::Emitter;
            let _ = handle.emit(
                "downloads-changed",
                serde_json::json!({ "id": id, "action": "paused" }),
            );
        }
    }
    Ok(serde_json::json!({ "status": "paused", "id": id }))
}

#[command]
pub fn get_download_telemetry(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let tasks = state.tasks.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(task) = tasks.get(&id) {
        let lock = task.telemetry.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ref telem) = *lock {
            serde_json::to_value(telem).map_err(|e| e.to_string())
        } else {
            Ok(serde_json::json!({ "status": "initializing" }))
        }
    } else {
        Err(format!("Task {} not found", id))
    }
}

#[command]
pub fn get_stats(state: State<'_, AppState>) -> serde_json::Value {
    let tasks = state.tasks.lock().unwrap_or_else(|e| e.into_inner());
    let active_count = tasks.len();
    serde_json::json!({
        "status": "ready",
        "active_downloads": active_count,
        "engine": "adaptive_v1",
    })
}

#[command]
pub fn get_feature_packs() -> serde_json::Value {
    let packs = edm_core::FeaturePackManager::list_all_packs();
    serde_json::to_value(packs).unwrap_or_else(|_| serde_json::json!([]))
}

#[command]
pub fn set_speed_limit(
    state: State<'_, AppState>,
    bytes_per_sec: Option<u64>,
) -> Result<(), String> {
    state.global_limiter.set_limit(bytes_per_sec.unwrap_or(0));
    Ok(())
}
