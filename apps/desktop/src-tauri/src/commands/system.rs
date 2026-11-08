use std::sync::atomic::Ordering;
use tauri::{command, State};

use crate::lifecycle::LifecycleStatus;
use crate::os::*;
use crate::state::AppState;

#[command]
pub async fn get_lifecycle_status(state: State<'_, AppState>) -> Result<LifecycleStatus, String> {
    let active_count = {
        let mut count = 0;
        if let Ok(tasks) = state.tasks.lock() {
            for handle in tasks.values() {
                if !handle.cancelled.load(Ordering::Relaxed) {
                    count += 1;
                }
            }
        }
        count
    };
    Ok(state.lifecycle.get_status(active_count).await)
}

#[command]
pub async fn wake_engine(
    state: State<'_, AppState>,
    show_window: Option<bool>,
) -> Result<(), String> {
    state
        .lifecycle
        .record_wake(show_window.unwrap_or(false))
        .await;
    Ok(())
}

#[command]
pub fn get_system_downloads_dir() -> String {
    get_system_downloads_dir_internal()
}

#[command]
pub async fn select_download_directory(
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    select_download_directory_internal(default_path).await
}

#[command]
pub fn get_disk_free_space(path: Option<String>) -> Result<serde_json::Value, String> {
    get_disk_free_space_internal(path)
}
