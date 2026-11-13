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

#[command]
pub async fn perform_native_installation(
    target_dir: String,
    create_desktop_shortcut: bool,
    start_on_boot: bool,
    enable_browser_integration: bool,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        perform_native_installation_internal(
            &target_dir,
            create_desktop_shortcut,
            start_on_boot,
            enable_browser_integration,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(windows)]
fn perform_native_installation_internal(
    target_dir: &str,
    create_desktop_shortcut: bool,
    start_on_boot: bool,
    enable_browser_integration: bool,
) -> Result<(), String> {
    use std::path::PathBuf;
    use windows_registry::CURRENT_USER;

    let target_path = PathBuf::from(target_dir);

    // 1. Create target directory
    std::fs::create_dir_all(&target_path)
        .map_err(|e| format!("Failed to create target directory: {}", e))?;

    // 2. Copy running executable and sibling files to target_dir
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Cannot resolve current exe: {}", e))?;
    let current_dir = current_exe
        .parent()
        .ok_or("Cannot resolve current exe directory")?;

    // Copy every file from the running exe's directory to target_dir
    if let Ok(entries) = std::fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let src = entry.path();
            if src.is_file() {
                let dst = target_path.join(entry.file_name());
                if let Err(e) = std::fs::copy(&src, &dst) {
                    tracing::warn!("Installer: could not copy {:?} → {:?}: {}", src, dst, e);
                }
            }
        }
    }

    let edm_exe = target_path.join("EDM.exe");
    let edm_desktop_exe = target_path.join("edm-desktop-app.exe");

    // Ensure bidirectional co-location between EDM.exe and edm-desktop-app.exe
    if edm_exe.exists() && !edm_desktop_exe.exists() {
        let _ = std::fs::copy(&edm_exe, &edm_desktop_exe);
    } else if edm_desktop_exe.exists() && !edm_exe.exists() {
        let _ = std::fs::copy(&edm_desktop_exe, &edm_exe);
    }

    let edm_exe_str = edm_exe.to_string_lossy().to_string();

    // 3. Desktop shortcut
    if create_desktop_shortcut {
        let desktop = std::env::var("USERPROFILE")
            .map(|p| PathBuf::from(p).join("Desktop").join("EDM.lnk"))
            .map_err(|e| format!("USERPROFILE not set: {}", e))?;

        let lnk_path = desktop.to_string_lossy().to_string();
        let ps_script = format!(
            r#"$WS = New-Object -ComObject WScript.Shell; $SC = $WS.CreateShortcut('{lnk}'); $SC.TargetPath = '{exe}'; $SC.Save()"#,
            lnk = lnk_path.replace('\'', "''"),
            exe = edm_exe_str.replace('\'', "''"),
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_script])
            .output();
    }

    // 4. Start-on-boot registry entry
    if start_on_boot {
        if let Ok(key) =
            CURRENT_USER.create(r"Software\Microsoft\Windows\CurrentVersion\Run")
        {
            let value = format!("\"{}\" --background", edm_exe_str);
            let _ = key.set_string("EDM", &value);
        }
    }

    // 5. Browser integration (Native Messaging Host manifests + registry)
    if enable_browser_integration {
        let nmh_exe = target_path.join("edm-nmh.exe");
        let nmh_str = nmh_exe.to_string_lossy().to_string();

        let chromium_manifest = serde_json::json!({
            "name": "com.edm.nmh",
            "description": "EDM Native Messaging Host for Chrome, Edge and Brave",
            "path": nmh_str,
            "type": "stdio",
            "allowed_origins": edm_ipc::extensions::allowed_chromium_origins()
        });
        let firefox_manifest = serde_json::json!({
            "name": "com.edm.nmh",
            "description": "EDM Native Messaging Host for Mozilla Firefox",
            "path": nmh_str,
            "type": "stdio",
            "allowed_extensions": edm_ipc::extensions::allowed_firefox_ids()
        });

        let chromium_manifest_path = target_path.join("com.edm.nmh.json");
        let firefox_manifest_path = target_path.join("com.edm.nmh.firefox.json");

        if let Ok(content) = serde_json::to_string_pretty(&chromium_manifest) {
            let _ = std::fs::write(&chromium_manifest_path, &content);
        }
        if let Ok(content) = serde_json::to_string_pretty(&firefox_manifest) {
            let _ = std::fs::write(&firefox_manifest_path, &content);
        }

        let chromium_manifest_str = chromium_manifest_path.to_string_lossy().to_string();
        let firefox_manifest_str = firefox_manifest_path.to_string_lossy().to_string();

        let chromium_reg_keys = [
            r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh",
            r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh",
            r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh",
        ];
        for key in &chromium_reg_keys {
            if let Ok(k) = CURRENT_USER.create(key) {
                let _ = k.set_string("", &chromium_manifest_str);
            }
        }
        if let Ok(k) = CURRENT_USER.create(r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh") {
            let _ = k.set_string("", &firefox_manifest_str);
        }
    }

    // 6. Uninstall registry key
    {
        let uninstall_key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\EDM";
        if let Ok(key) = CURRENT_USER.create(uninstall_key) {
            let _ = key.set_string("DisplayName", "EDM — Edge Download Manager");
            let _ = key.set_string("DisplayIcon", &edm_exe_str);
            let _ = key.set_string("UninstallString", &format!("\"{}\" --clean-uninstall", edm_exe_str));
            let _ = key.set_string("QuietUninstallString", &format!("\"{}\" --clean-uninstall", edm_exe_str));
            let _ = key.set_string("InstallLocation", target_dir);
            let _ = key.set_string("Publisher", "EDM Project");
            let _ = key.set_u32("NoModify", 1);
            let _ = key.set_u32("NoRepair", 1);
        }
    }

    // 7. Record install dir in EDM software key
    if let Ok(k) = CURRENT_USER.create(r"Software\EDM") {
        let _ = k.set_string("InstallDir", target_dir);
        let _ = k.set_string("AppPath", &edm_exe_str);
    }

    // 8. Spawn installed EDM and exit
    let _ = std::process::Command::new(&edm_exe).spawn();
    std::process::exit(0);
}

#[cfg(not(windows))]
fn perform_native_installation_internal(
    _target_dir: &str,
    _create_desktop_shortcut: bool,
    _start_on_boot: bool,
    _enable_browser_integration: bool,
) -> Result<(), String> {
    Err("Native installation is only supported on Windows".to_string())
}
