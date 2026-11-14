use edm_ipc::DownloadState;
use std::sync::atomic::Ordering;
use tauri::{command, Manager, State};

use crate::state::AppState;

#[command]
pub fn app_minimize(window: tauri::WebviewWindow) {
    let _ = window.minimize();
}

#[command]
pub fn app_toggle_maximize(window: tauri::WebviewWindow) {
    if let Ok(is_max) = window.is_maximized() {
        if is_max {
            let _ = window.unmaximize();
        } else {
            let _ = window.maximize();
        }
    } else {
        let _ = window.maximize();
    }
}

#[command]
pub async fn app_close(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--setup" || a == "--uninstall" || a == "-uninstall") {
        std::process::exit(0);
    }

    let close_to_tray = if let Ok(Some(json_str)) = state.store.get_setting("app_settings") {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
            val.get("general")
                .and_then(|g| g.get("minimizeToTray"))
                .and_then(|m| m.as_bool())
                .unwrap_or(true)
        } else {
            true
        }
    } else {
        true
    };

    if close_to_tray {
        tracing::info!("Hiding main window to tray");
        let _ = window.hide();
        state.lifecycle.check_lifecycle_step(0, false).await;
    } else {
        trigger_graceful_quit(window.app_handle().clone(), state.inner().clone());
    }
    Ok(())
}

#[command]
pub fn app_start_dragging(window: tauri::WebviewWindow) {
    let _ = window.start_dragging();
}

pub fn trigger_graceful_quit(app_handle: tauri::AppHandle, state: AppState) {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        std::process::exit(0);
    });

    let task_handles = {
        let tasks = state.tasks.lock().unwrap();
        tasks.values().cloned().collect::<Vec<_>>()
    };
    for h in task_handles {
        h.cancelled.store(true, Ordering::SeqCst);
    }

    if let Ok(records) = state.store.list_all_downloads() {
        for r in records {
            if r.state == DownloadState::Downloading {
                let _ = state
                    .store
                    .update_download_state(r.id, DownloadState::Paused, None);
            }
        }
    }

    app_handle.exit(0);
}
