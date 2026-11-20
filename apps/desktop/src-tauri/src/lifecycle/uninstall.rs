use std::path::PathBuf;
use tracing::info;
use super::platform;

pub fn perform_silent_uninstallation() {
    let _ = perform_native_uninstallation(false);
}

pub fn perform_native_uninstallation(purge_user_data: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        perform_windows_uninstallation(purge_user_data)
    }
    #[cfg(not(windows))]
    {
        let _ = purge_user_data;
        Err("Uninstallation is only supported on Windows".into())
    }
}

#[cfg(windows)]
fn perform_windows_uninstallation(purge_user_data: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // 1. Terminate sibling processes
    let _ = platform::terminate_sibling_instances();

    // 2. Remove Shortcuts
    if let Ok(desktop_dir) = std::env::var("USERPROFILE") {
        let lnk = PathBuf::from(desktop_dir).join("Desktop").join("EDM.lnk");
        let _ = std::fs::remove_file(lnk);
    }
    if let Ok(appdata_dir) = std::env::var("APPDATA") {
        let lnk = PathBuf::from(appdata_dir).join(r"Microsoft\Windows\Start Menu\Programs\EDM.lnk");
        let _ = std::fs::remove_file(lnk);
    }

    // 3. Remove Registry Entries
    platform::remove_uninstall_entry();
    let _ = platform::register_autostart(std::path::Path::new(""), false);
    platform::remove_protocol_handlers();
    platform::remove_browser_nmh();

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
        "$localAppData = Join-Path $env:LOCALAPPDATA 'EDM'; $localDesktop = Join-Path $env:LOCALAPPDATA 'com.edm.desktop'; if (Test-Path $localAppData) { Remove-Item -LiteralPath $localAppData -Recurse -Force -ErrorAction SilentlyContinue }; if (Test-Path $localDesktop) { Remove-Item -LiteralPath $localDesktop -Recurse -Force -ErrorAction SilentlyContinue }"
    } else {
        ""
    };

    let cleanup_script = format!(
        "Start-Sleep -Seconds 1; Stop-Process -Name 'EDM','edm-desktop-app','edm-nmh' -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 500; if ('{target}' -ne '' -and (Test-Path '{target}')) {{ Remove-Item -LiteralPath '{target}' -Recurse -Force -ErrorAction SilentlyContinue }}; {purge}",
        target = target_str,
        purge = purge_clause
    );

    let _ = std::process::Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &cleanup_script])
        .spawn();

    info!("Native uninstallation dispatched. Exiting.");
    Ok(())
}
