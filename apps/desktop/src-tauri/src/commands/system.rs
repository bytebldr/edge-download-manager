use std::path::PathBuf;
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
pub fn get_system_user_dirs() -> SystemUserDirs {
    get_system_user_dirs_internal()
}

#[command]
pub async fn send_system_notification(
    title: String,
    message: String,
    path: Option<String>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        send_system_notification_internal(&title, &message, path.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub async fn select_download_directory(
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    select_download_directory_internal(default_path).await
}

#[command]
pub async fn select_torrent_file() -> Result<Option<String>, String> {
    select_torrent_file_internal().await
}

#[command]
pub fn get_disk_free_space(path: Option<String>) -> Result<serde_json::Value, String> {
    get_disk_free_space_internal(path)
}

pub use crate::lifecycle::{ExistingInstallationInfo, RepairReport};

#[command]
pub fn get_default_install_dir() -> String {
    crate::lifecycle::get_default_install_dir()
}

#[command]
pub async fn perform_native_installation(
    target_dir: String,
    create_desktop_shortcut: bool,
    start_on_boot: bool,
    enable_browser_integration: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        crate::lifecycle::perform_native_installation(crate::lifecycle::InstallOptions {
            target_dir,
            create_desktop_shortcut,
            start_on_boot,
            enable_browser_integration,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub fn check_existing_installation() -> ExistingInstallationInfo {
    crate::lifecycle::check_existing_installation()
}

#[command]
pub async fn perform_native_uninstallation(purge_user_data: bool) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        crate::lifecycle::perform_native_uninstallation(purge_user_data)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn perform_silent_uninstallation() {
    crate::lifecycle::perform_silent_uninstallation();
}

#[command]
pub async fn perform_native_repair(
    repair_browser: bool,
    repair_shortcuts: bool,
    verify_binaries: bool,
) -> Result<RepairReport, String> {
    crate::lifecycle::perform_native_repair(crate::lifecycle::RepairPlan {
        repair_browser,
        repair_shortcuts,
        verify_binaries,
    })
}

#[command]
pub fn launch_main_app() -> Result<(), String> {
    let install_dir = PathBuf::from(crate::lifecycle::get_default_install_dir());
    let edm_exe = install_dir.join("EDM.exe");
    let target = if edm_exe.exists() {
        edm_exe
    } else {
        std::env::current_exe().map_err(|e| e.to_string())?
    };

    let _ = std::process::Command::new(target)
        .args(["--show-window"])
        .spawn()
        .map_err(|e| format!("Failed to spawn EDM: {}", e))?;

    std::process::exit(0);
}
