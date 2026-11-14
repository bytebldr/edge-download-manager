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
pub fn get_default_install_dir() -> String {
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return format!("{}\\EDM", local);
        }
        if let Ok(user) = std::env::var("USERPROFILE") {
            return format!("{}\\AppData\\Local\\EDM", user);
        }
    }
    std::env::temp_dir().join("EDM").to_string_lossy().to_string()
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

    // 1. Create target directory (with UAC elevation fallback if permission denied)
    if let Err(e) = std::fs::create_dir_all(&target_path) {
        if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(5) {
            let escaped_target = target_path.to_string_lossy().replace('\'', "''");
            let ps_elevate = format!(
                "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile -Command New-Item -ItemType Directory -Force -Path ''{}'''",
                escaped_target
            );
            let status = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_elevate])
                .status()
                .map_err(|se| format!("Failed to request administrator permissions: {}", se))?;

            if !status.success() || !target_path.exists() {
                return Err(format!(
                    "Administrator privileges are required to install to '{}'. Please accept the UAC prompt or choose a user directory (e.g. %LOCALAPPDATA%\\EDM).",
                    target_dir
                ));
            }
        } else {
            return Err(format!("Failed to create target directory: {}", e));
        }
    }

    // 2. Terminate existing background instances of EDM and NMH (excluding our own process)
    // so destination files are unlocked and not in use
    let current_pid = std::process::id();
    let kill_ps = format!(
        "Get-Process -Name 'EDM','edm-desktop-app','edm-nmh' -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {} }} | Stop-Process -Force",
        current_pid
    );
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &kill_ps])
        .output();
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 3. Copy running executable and sibling files to target_dir
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Cannot resolve current exe: {}", e))?;
    let current_dir = current_exe
        .parent()
        .ok_or("Cannot resolve current exe directory")?;

    let mut needs_elevated_copy = false;
    if let Ok(entries) = std::fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let src = entry.path();
            if src.is_file() {
                let dst = target_path.join(entry.file_name());
                if let Err(e) = std::fs::copy(&src, &dst) {
                    if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(5) {
                        needs_elevated_copy = true;
                    } else {
                        tracing::warn!("Installer: could not copy {:?} → {:?}: {}", src, dst, e);
                    }
                }
            }
        }
    }

    // Ensure edm-nmh.exe is found and copied to target_path\edm-nmh.exe
    let target_nmh = target_path.join("edm-nmh.exe");
    if !target_nmh.exists() {
        // Search sibling folders (e.g. resources, _up_\_up_\_up_\target\release\edm-nmh.exe, target\release)
        let candidate_paths = [
            current_dir.join("edm-nmh.exe"),
            current_dir.join("resources").join("edm-nmh.exe"),
            current_dir.join("resources").join("target").join("release").join("edm-nmh.exe"),
            current_dir.join("_up_").join("_up_").join("_up_").join("target").join("release").join("edm-nmh.exe"),
        ];
        let mut copied_nmh = false;
        for c in &candidate_paths {
            if c.exists() {
                if let Ok(_) = std::fs::copy(c, &target_nmh) {
                    copied_nmh = true;
                    break;
                }
            }
        }
        if !copied_nmh {
            // Recursive scan of current_dir
            fn find_file_recursive(dir: &std::path::Path, target_name: &str) -> Option<PathBuf> {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for entry in rd.flatten() {
                        let p = entry.path();
                        if p.is_file() && p.file_name().and_then(|n| n.to_str()) == Some(target_name) {
                            return Some(p);
                        } else if p.is_dir() {
                            if let Some(found) = find_file_recursive(&p, target_name) {
                                return Some(found);
                            }
                        }
                    }
                }
                None
            }
            if let Some(found_nmh) = find_file_recursive(current_dir, "edm-nmh.exe") {
                let _ = std::fs::copy(&found_nmh, &target_nmh);
            }
        }
    }

    // Ensure WebView2Loader.dll is found and copied to target_path\WebView2Loader.dll
    let target_wv2 = target_path.join("WebView2Loader.dll");
    if !target_wv2.exists() {
        let candidate_paths = [
            current_dir.join("WebView2Loader.dll"),
            current_dir.join("resources").join("WebView2Loader.dll"),
            current_dir.join("resources").join("target").join("release").join("WebView2Loader.dll"),
            current_dir.join("_up_").join("_up_").join("_up_").join("target").join("release").join("WebView2Loader.dll"),
        ];
        let mut copied_wv2 = false;
        for c in &candidate_paths {
            if c.exists() && c.metadata().map(|m| m.len()).unwrap_or(0) > 1000 {
                if let Ok(_) = std::fs::copy(c, &target_wv2) {
                    copied_wv2 = true;
                    break;
                }
            }
        }
        if !copied_wv2 {
            if let Some(found_wv2) = find_file_recursive(current_dir, "WebView2Loader.dll") {
                if found_wv2.metadata().map(|m| m.len()).unwrap_or(0) > 1000 {
                    let _ = std::fs::copy(&found_wv2, &target_wv2);
                }
            }
        }
    }

    if needs_elevated_copy {
        let escaped_src = current_dir.to_string_lossy().replace('\'', "''");
        let escaped_target = target_path.to_string_lossy().replace('\'', "''");
        let ps_copy_elevated = format!(
            "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile -Command Copy-Item -Path ''{}\\*'' -Destination ''{}'' -Recurse -Force'",
            escaped_src, escaped_target
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_copy_elevated])
            .status();
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

    // 4. Start Menu shortcut (ALWAYS create so EDM appears in Windows Start section)
    if let Ok(appdata) = std::env::var("APPDATA") {
        let start_menu_dir = PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs");
        let _ = std::fs::create_dir_all(&start_menu_dir);
        let start_menu_lnk = start_menu_dir.join("EDM.lnk");
        let lnk_path = start_menu_lnk.to_string_lossy().to_string();
        let ps_script = format!(
            r#"$WS = New-Object -ComObject WScript.Shell; $SC = $WS.CreateShortcut('{lnk}'); $SC.TargetPath = '{exe}'; $SC.WorkingDirectory = '{workdir}'; $SC.IconLocation = '{exe},0'; $SC.Save()"#,
            lnk = lnk_path.replace('\'', "''"),
            exe = edm_exe_str.replace('\'', "''"),
            workdir = target_path.to_string_lossy().replace('\'', "''"),
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_script])
            .output();
    }

    // 5. Desktop shortcut
    if create_desktop_shortcut {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let desktop = PathBuf::from(userprofile).join("Desktop").join("EDM.lnk");
            let lnk_path = desktop.to_string_lossy().to_string();
            let ps_script = format!(
                r#"$WS = New-Object -ComObject WScript.Shell; $SC = $WS.CreateShortcut('{lnk}'); $SC.TargetPath = '{exe}'; $SC.WorkingDirectory = '{workdir}'; $SC.IconLocation = '{exe},0'; $SC.Save()"#,
                lnk = lnk_path.replace('\'', "''"),
                exe = edm_exe_str.replace('\'', "''"),
                workdir = target_path.to_string_lossy().replace('\'', "''"),
            );
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_script])
                .output();
        }
    }

    // 6. Start-on-boot registry entry
    if start_on_boot {
        if let Ok(key) =
            CURRENT_USER.create(r"Software\Microsoft\Windows\CurrentVersion\Run")
        {
            let value = format!("\"{}\" --background", edm_exe_str);
            let _ = key.set_string("EDM", &value);
        }
    }

    // 7. Browser integration (Native Messaging Host manifests + registry)
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
            if let Err(_) = std::fs::write(&chromium_manifest_path, &content) {
                let temp_m = std::env::temp_dir().join("com.edm.nmh.json");
                if std::fs::write(&temp_m, &content).is_ok() {
                    let _ = std::process::Command::new("powershell")
                        .args(["-NoProfile", "-Command", &format!(
                            "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile -Command Copy-Item -Path ''{}'' -Destination ''{}'' -Force'",
                            temp_m.to_string_lossy().replace('\'', "''"),
                            chromium_manifest_path.to_string_lossy().replace('\'', "''")
                        )])
                        .status();
                }
            }
        }
        if let Ok(content) = serde_json::to_string_pretty(&firefox_manifest) {
            if let Err(_) = std::fs::write(&firefox_manifest_path, &content) {
                let temp_m = std::env::temp_dir().join("com.edm.nmh.firefox.json");
                if std::fs::write(&temp_m, &content).is_ok() {
                    let _ = std::process::Command::new("powershell")
                        .args(["-NoProfile", "-Command", &format!(
                            "Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile -Command Copy-Item -Path ''{}'' -Destination ''{}'' -Force'",
                            temp_m.to_string_lossy().replace('\'', "''"),
                            firefox_manifest_path.to_string_lossy().replace('\'', "''")
                        )])
                        .status();
                }
            }
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

    // 8. Uninstall registry key with full Windows Settings metadata
    // Formats: DisplayVersion = "0.0.13", Publisher = "bytebldr", InstallDate = YYYYMMDD
    // Result in Windows Settings: "0.0.13 | bytebldr | 13-11-2026"
    {
        let uninstall_key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\EDM";
        if let Ok(key) = CURRENT_USER.create(uninstall_key) {
            let _ = key.set_string("DisplayName", "EDM — Edge Download Manager");
            let _ = key.set_string("DisplayIcon", &edm_exe_str);
            let _ = key.set_string("DisplayVersion", env!("CARGO_PKG_VERSION"));
            let _ = key.set_string("Publisher", "bytebldr");
            let now_date = chrono::Local::now().format("%Y%m%d").to_string();
            let _ = key.set_string("InstallDate", &now_date);
            let _ = key.set_string("UninstallString", &format!("\"{}\" --uninstall", edm_exe_str));
            let _ = key.set_string("QuietUninstallString", &format!("\"{}\" --clean-uninstall", edm_exe_str));
            let _ = key.set_string("InstallLocation", target_dir);
            let _ = key.set_u32("NoModify", 1);
            let _ = key.set_u32("NoRepair", 1);
        }
    }

    // 9. Record install dir in EDM software key
    if let Ok(k) = CURRENT_USER.create(r"Software\EDM") {
        let _ = k.set_string("InstallDir", target_dir);
        let _ = k.set_string("AppPath", &edm_exe_str);
    }

    // 10. Spawn installed EDM and exit installer
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

#[derive(serde::Serialize)]
pub struct ExistingInstallationInfo {
    pub is_installed: bool,
    pub install_dir: Option<String>,
    pub version: Option<String>,
    pub publisher: Option<String>,
}

#[tauri::command]
pub fn check_existing_installation() -> ExistingInstallationInfo {
    #[cfg(windows)]
    {
        use std::path::PathBuf;
        use windows_registry::CURRENT_USER;
        let mut info = ExistingInstallationInfo {
            is_installed: false,
            install_dir: None,
            version: None,
            publisher: None,
        };

        if let Ok(key) = CURRENT_USER.open(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\EDM") {
            let mut valid = false;
            if let Ok(loc) = key.get_string("InstallLocation") {
                let clean_loc = loc.trim_matches('"');
                let dir_path = PathBuf::from(clean_loc);
                let lower = clean_loc.to_lowercase();
                if !lower.contains(r"\temp\")
                    && !lower.contains("nsd")
                    && (dir_path.join("EDM.exe").exists() || dir_path.join("edm-desktop-app.exe").exists())
                {
                    valid = true;
                    info.install_dir = Some(clean_loc.to_string());
                }
            }
            if valid {
                info.is_installed = true;
                if let Ok(v) = key.get_string("DisplayVersion") {
                    info.version = Some(v);
                }
                if let Ok(p) = key.get_string("Publisher") {
                    info.publisher = Some(p);
                }
            }
        }

        if !info.is_installed {
            if let Ok(key) = CURRENT_USER.open(r"Software\EDM") {
                if let Ok(dir) = key.get_string("InstallDir") {
                    let dir_path = PathBuf::from(&dir);
                    let lower = dir.to_lowercase();
                    // Ignore transient/temp/extracted paths so they are never detected as existing installs
                    if !lower.contains(r"\temp\")
                        && !lower.contains("nsd")
                        && !lower.contains("target")
                        && (dir_path.join("EDM.exe").exists() || dir_path.join("edm-desktop-app.exe").exists())
                    {
                        info.is_installed = true;
                        info.install_dir = Some(dir);
                    }
                }
            }
        }

        if !info.is_installed {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let p = PathBuf::from(local).join("EDM").join("EDM.exe");
                if p.exists() {
                    info.is_installed = true;
                    info.install_dir = Some(p.parent().unwrap().to_string_lossy().to_string());
                }
            }
        }

        info
    }
    #[cfg(not(windows))]
    {
        ExistingInstallationInfo {
            is_installed: false,
            install_dir: None,
            version: None,
            publisher: None,
        }
    }
}

#[tauri::command]
pub async fn perform_native_uninstallation(purge_user_data: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        perform_native_uninstallation_internal(purge_user_data)
    }
    #[cfg(not(windows))]
    {
        let _ = purge_user_data;
        Err("Uninstallation is only supported on Windows".to_string())
    }
}

#[cfg(windows)]
pub fn perform_silent_uninstallation() {
    let _ = perform_native_uninstallation_internal(false);
}

#[cfg(windows)]
fn perform_native_uninstallation_internal(purge_user_data: bool) -> Result<(), String> {
    use std::path::PathBuf;
    use windows_registry::CURRENT_USER;

    // 1. Kill other running EDM and NMH processes
    let current_pid = std::process::id();
    let kill_ps = format!(
        "Get-Process -Name 'EDM','edm-desktop-app','edm-nmh' -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {} }} | Stop-Process -Force",
        current_pid
    );
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &kill_ps])
        .output();

    // 2. Remove shortcuts
    if let Ok(desktop_dir) = std::env::var("USERPROFILE") {
        let lnk = PathBuf::from(desktop_dir).join("Desktop").join("EDM.lnk");
        let _ = std::fs::remove_file(lnk);
    }
    if let Ok(appdata_dir) = std::env::var("APPDATA") {
        let lnk = PathBuf::from(appdata_dir).join(r"Microsoft\Windows\Start Menu\Programs\EDM.lnk");
        let _ = std::fs::remove_file(lnk);
    }

    // 3. Remove Registry Keys
    let _ = CURRENT_USER.remove_tree(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\EDM");
    if let Ok(run_key) = CURRENT_USER.open(r"Software\Microsoft\Windows\CurrentVersion\Run") {
        let _ = run_key.remove_value("EDM");
    }
    let _ = CURRENT_USER.remove_tree(r"Software\EDM");
    let _ = CURRENT_USER.remove_tree(r"Software\Classes\magnet");
    let _ = CURRENT_USER.remove_tree(r"Software\Classes\.torrent");
    let _ = CURRENT_USER.remove_tree(r"Software\Classes\EDM.TorrentFile");
    let _ = CURRENT_USER.remove_tree(r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh");
    let _ = CURRENT_USER.remove_tree(r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh");
    let _ = CURRENT_USER.remove_tree(r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh");
    let _ = CURRENT_USER.remove_tree(r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh");

    // 4. Resolve install directory to delete
    let mut target_dir: Option<PathBuf> = None;
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let p_str = parent.to_string_lossy().to_lowercase();
            if !p_str.contains("temp") && !p_str.contains("target\\release") && !p_str.contains("target\\debug") {
                target_dir = Some(parent.to_path_buf());
            }
        }
    }
    if target_dir.is_none() {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("EDM");
            if p.exists() {
                target_dir = Some(p);
            }
        }
    }

    // 5. Spawn background detached PowerShell script to remove files after this process terminates
    let target_str = target_dir
        .as_ref()
        .map(|p| p.to_string_lossy().replace('\'', "''"))
        .unwrap_or_default();

    let purge_clause = if purge_user_data {
        "if (Test-Path $localAppData) { Remove-Item -LiteralPath $localAppData -Recurse -Force -ErrorAction SilentlyContinue }; if (Test-Path $localDesktop) { Remove-Item -LiteralPath $localDesktop -Recurse -Force -ErrorAction SilentlyContinue }"
    } else {
        ""
    };

    let cleanup_script = format!(
        "Start-Sleep -Seconds 1; Stop-Process -Name 'EDM','edm-desktop-app','edm-nmh' -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 500; if ('{target}' -ne '' -and (Test-Path '{target}')) {{ Remove-Item -LiteralPath '{target}' -Recurse -Force -ErrorAction SilentlyContinue }}; $localAppData = Join-Path $env:LOCALAPPDATA 'EDM'; $localDesktop = Join-Path $env:LOCALAPPDATA 'com.edm.desktop'; {purge}",
        target = target_str,
        purge = purge_clause
    );

    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &cleanup_script])
        .spawn();

    Ok(())
}
