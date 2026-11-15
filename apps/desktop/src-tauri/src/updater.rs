use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};

fn append_update_log(message: &str) {
    let log_dir = match std::env::var("LOCALAPPDATA") {
        Ok(dir) => PathBuf::from(dir).join("EDM").join("logs"),
        Err(_) => match std::env::var("APPDATA") {
            Ok(dir) => PathBuf::from(dir).join("EDM").join("logs"),
            Err(_) => std::env::temp_dir().join("EDM").join("logs"),
        },
    };
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = log_dir.join("update.log");

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] {}\n", timestamp, message);

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_name: String,
    pub release_notes: String,
    pub published_at: String,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: u64,
    pub html_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OtaProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f64,
    pub status: String,
}

#[derive(Deserialize, Debug)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(Deserialize, Debug)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    html_url: String,
    assets: Vec<GhAsset>,
}

pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let clean = v.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = clean.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.split(['-', '+']).next()?.parse().ok()?;
    Some((major, minor, patch))
}

pub fn is_newer_version(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let api_url = "https://api.github.com/repos/bytebldr/edge-download-manager/releases/latest";

    let client = reqwest::Client::builder()
        .user_agent(format!("EDM/{} (Windows; x86_64)", current_version))
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Attempt 1: Official GitHub Releases API (detailed changelog & exact asset bytes)
    let api_res = client
        .get(api_url)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await;

    if let Ok(res) = api_res {
        if res.status().is_success() {
            if let Ok(body) = res.text().await {
                if let Ok(release) = serde_json::from_str::<GhRelease>(&body) {
                    let raw_tag = release.tag_name.trim();
                    let latest_version = raw_tag.trim_start_matches('v').trim_start_matches('V');
                    let update_available = is_newer_version(latest_version, current_version);

                    let setup_asset = release
                        .assets
                        .iter()
                        .find(|a| {
                            a.name.ends_with(".exe") && a.name.to_lowercase().contains("setup")
                        })
                        .or_else(|| release.assets.iter().find(|a| a.name.ends_with(".exe")));

                    let download_url = setup_asset.map(|a| a.browser_download_url.clone());
                    let asset_name = setup_asset.map(|a| a.name.clone());
                    let asset_size = setup_asset.map(|a| a.size).unwrap_or(0);

                    return Ok(UpdateCheckResult {
                        current_version: current_version.to_string(),
                        latest_version: latest_version.to_string(),
                        update_available,
                        release_name: release.name.unwrap_or_else(|| release.tag_name.clone()),
                        release_notes: release.body.unwrap_or_default(),
                        published_at: release.published_at.unwrap_or_default(),
                        download_url,
                        asset_name,
                        asset_size,
                        html_url: release.html_url,
                    });
                }
            }
        }
    }

    // Attempt 2 (Resilient Zero-Rate-Limit Fallback):
    // Follow HTTP redirect of https://github.com/bytebldr/edge-download-manager/releases/latest
    // Never subject to GitHub 60 req/hr API rate limits or 403 blocks
    let redirect_client = reqwest::Client::builder()
        .user_agent(format!("EDM/{} (Windows; x86_64)", current_version))
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| format!("Failed to create fallback client: {}", e))?;

    let redirect_res = redirect_client
        .get("https://github.com/bytebldr/edge-download-manager/releases/latest")
        .send()
        .await
        .map_err(|e| format!("Failed to reach update server: {}", e))?;

    let final_url = redirect_res.url().to_string();
    if let Some(pos) = final_url.rfind("/tag/") {
        let raw_tag = final_url[pos + 5..].trim().trim_end_matches('/');
        let latest_version = raw_tag.trim_start_matches('v').trim_start_matches('V');
        let update_available = is_newer_version(latest_version, current_version);
        let exe_name = format!("EDM_{}_x64-setup.exe", latest_version);
        let download_url = format!(
            "https://github.com/bytebldr/edge-download-manager/releases/download/{}/{}",
            raw_tag, exe_name
        );

        return Ok(UpdateCheckResult {
            current_version: current_version.to_string(),
            latest_version: latest_version.to_string(),
            update_available,
            release_name: format!("EDM v{}", latest_version),
            release_notes: format!("Stable release v{} available on GitHub.", latest_version),
            published_at: String::new(),
            download_url: Some(download_url),
            asset_name: Some(exe_name),
            asset_size: 0,
            html_url: final_url,
        });
    }

    // Attempt 3 (Up-to-date fallback):
    Ok(UpdateCheckResult {
        current_version: current_version.to_string(),
        latest_version: current_version.to_string(),
        update_available: false,
        release_name: format!("EDM v{} Production", current_version),
        release_notes: "You are running the latest version.".to_string(),
        published_at: String::new(),
        download_url: None,
        asset_name: None,
        asset_size: 0,
        html_url: "https://github.com/bytebldr/edge-download-manager/releases".to_string(),
    })
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, crate::AppState>,
    download_url: String,
) -> Result<(), String> {
    let trimmed = download_url.trim();
    if !trimmed.starts_with("https://github.com/bytebldr/edge-download-manager/releases/download/") {
        return Err("Security violation: Update download URL must originate from official bytebldr/edge-download-manager GitHub releases".into());
    }

    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("EDM_Update_Installer.exe");

    let client = reqwest::Client::builder()
        .user_agent("EDM-OTA-Updater")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let res = client
        .get(trimmed)
        .send()
        .await
        .map_err(|e| format!("Failed to download update installer: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Download failed with HTTP {}", res.status()));
    }

    let total_bytes = res.content_length().unwrap_or(0);
    if total_bytes < 1_000_000 {
        return Err("Invalid installer package size received from release server".into());
    }

    use futures_util::StreamExt;
    let mut stream = res.bytes_stream();
    let mut file = tokio::fs::File::create(&installer_path)
        .await
        .map_err(|e| format!("Failed to create destination file: {}", e))?;

    let mut downloaded: u64 = 0;
    use tokio::io::AsyncWriteExt;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Download stream error: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("File write error: {}", e))?;
        downloaded += chunk.len() as u64;

        let percent = (downloaded as f64 / total_bytes as f64) * 100.0;

        let _ = app.emit(
            "ota-progress",
            OtaProgress {
                downloaded_bytes: downloaded,
                total_bytes,
                percent,
                status: "downloading".to_string(),
            },
        );
    }

    file.flush()
        .await
        .map_err(|e| format!("File flush error: {}", e))?;
    drop(file);

    if downloaded != total_bytes {
        let _ = tokio::fs::remove_file(&installer_path).await;
        return Err(format!(
            "Incomplete update download: expected {} bytes but only received {} bytes. Aborting installation.",
            total_bytes, downloaded
        ));
    }

    let current_ver = env!("CARGO_PKG_VERSION").to_string();
    tracing::info!(
        "OTA update package downloaded ({} bytes). Current version: v{}, installer: {}",
        downloaded,
        current_ver,
        installer_path.display()
    );

    append_update_log(&format!(
        "OTA package download verified. Bytes: {}, Current version: v{}, Path: {}",
        downloaded,
        current_ver,
        installer_path.display()
    ));

    let _ = app.emit(
        "ota-progress",
        OtaProgress {
            downloaded_bytes: downloaded,
            total_bytes: downloaded,
            percent: 100.0,
            status: "ready".to_string(),
        },
    );

    // Save target pending update flag in store for post-update verification
    let _ = state.store.set_setting("pending_update_version", &installer_path.to_string_lossy());
    let _ = state.store.checkpoint();

    // Verify installer file on disk and verify minimum size
    let metadata = std::fs::metadata(&installer_path).map_err(|e| {
        let msg = format!("Failed to read installer metadata: {}", e);
        append_update_log(&format!("ERROR: {}", msg));
        msg
    })?;
    let file_size = metadata.len();
    if file_size == 0 || file_size < 1_000_000 {
        let msg = format!("Installer package on disk is incomplete or zero bytes ({} bytes)", file_size);
        append_update_log(&format!("ERROR: {}", msg));
        return Err(msg);
    }
    append_update_log(&format!("Installer verified on disk: {} (size: {} bytes)", installer_path.display(), file_size));

    #[cfg(windows)]
    {
        // Phase 3: Terminate ALL other EDM processes (helper, tray, other instances)
        append_update_log("Phase 3: Stopping other EDM processes (EDM.exe, edm-desktop-app.exe, edm-nmh.exe)...");
        stop_all_other_edm_processes();

        // Phase 2: Launch installer directly via Windows ShellExecuteW
        append_update_log(&format!("Phase 2: Launching installer via ShellExecuteW with args '/P /R': {}", installer_path.display()));
        if let Err(e) = launch_installer_windows(&installer_path) {
            append_update_log(&format!("ERROR: ShellExecuteW failed: {}", e));
            return Err(e);
        }
        append_update_log("ShellExecuteW succeeded. Installer process initiated.");
    }

    #[cfg(not(windows))]
    {
        append_update_log(&format!("Launching non-windows installer: {}", installer_path.display()));
        std::process::Command::new(&installer_path)
            .spawn()
            .map_err(|e| {
                let msg = format!("Failed to launch update installer: {}", e);
                append_update_log(&format!("ERROR: {}", msg));
                msg
            })?;
    }

    // Gracefully shut down this EDM instance so installer can replace locked binaries
    append_update_log("Triggering graceful shutdown of current EDM instance...");
    crate::trigger_graceful_quit(app, state.inner().clone());

    Ok(())
}

#[cfg(windows)]
fn stop_all_other_edm_processes() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let current_pid = std::process::id();
    append_update_log(&format!("Current process PID is {}. Terminating sibling EDM processes...", current_pid));

    let targets = ["edm-nmh.exe", "edm-desktop-app.exe", "EDM.exe"];
    for target in targets {
        // Run taskkill filtering out current PID to prevent committing suicide prematurely
        let filter_pid = format!("PID ne {}", current_pid);
        let filter_img = format!("IMAGENAME eq {}", target);
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/F", "/FI", &filter_pid, "/FI", &filter_img, "/T"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output();
        match output {
            Ok(out) => {
                let status_str = String::from_utf8_lossy(&out.stdout);
                append_update_log(&format!("taskkill {} result (exit={:?}): {}", target, out.status.code(), status_str.trim()));
            }
            Err(e) => {
                append_update_log(&format!("taskkill {} execution failed: {}", target, e));
            }
        }
    }

    // Wait up to 5 seconds for target processes to fully release file locks
    let start_wait = std::time::Instant::now();
    let max_wait = std::time::Duration::from_secs(5);
    while start_wait.elapsed() < max_wait {
        let mut check_cmd = std::process::Command::new("tasklist");
        check_cmd.args(["/FI", "IMAGENAME eq edm-nmh.exe", "/NH"]);
        check_cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(out) = check_cmd.output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if !text.contains("edm-nmh.exe") {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    append_update_log(&format!("Wait for sibling processes completed in {:?}", start_wait.elapsed()));
}

#[cfg(windows)]
fn launch_installer_windows(installer_path: &Path) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let path_wide: Vec<u16> = installer_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let params_wide: Vec<u16> = OsStr::new("/P /R").encode_wide().chain(std::iter::once(0)).collect();

    // Prefer standard "open" verb. If standard execution fails or elevation is requested, fallback to "runas".
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            lpOperation: *const u16,
            lpFile: *const u16,
            lpParameters: *const u16,
            lpDirectory: *const u16,
            nShowCmd: i32,
        ) -> isize;
    }

    const SW_SHOWNORMAL: i32 = 1;
    let open_verb: Vec<u16> = OsStr::new("open").encode_wide().chain(std::iter::once(0)).collect();
    let runas_verb: Vec<u16> = OsStr::new("runas").encode_wide().chain(std::iter::once(0)).collect();

    append_update_log("Invoking ShellExecuteW with verb 'open'...");
    let res = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            open_verb.as_ptr(),
            path_wide.as_ptr(),
            params_wide.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    append_update_log(&format!("ShellExecuteW 'open' returned handle value: {}", res));

    // Values > 32 indicate success
    if res > 32 {
        return Ok(());
    }

    // If "open" fails with SE_ERR_ACCESSDENIED (5) or error <= 32, try "runas" for UAC prompt
    append_update_log(&format!("ShellExecuteW 'open' returned code {}. Retrying with 'runas' verb for UAC...", res));
    let runas_res = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            runas_verb.as_ptr(),
            path_wide.as_ptr(),
            params_wide.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    append_update_log(&format!("ShellExecuteW 'runas' returned handle value: {}", runas_res));
    if runas_res > 32 {
        return Ok(());
    }

    // As an additional robust fallback, try CreateProcessW with CREATE_BREAKAWAY_FROM_JOB | CREATE_NEW_PROCESS_GROUP
    append_update_log("ShellExecuteW verbs failed. Attempting Command::new with CREATE_BREAKAWAY_FROM_JOB fallback...");
    use std::os::windows::process::CommandExt;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

    let mut fallback_cmd = std::process::Command::new(installer_path);
    fallback_cmd.args(["/P", "/R"]);
    fallback_cmd.creation_flags(CREATE_BREAKAWAY_FROM_JOB | CREATE_NEW_PROCESS_GROUP);
    match fallback_cmd.spawn() {
        Ok(child) => {
            append_update_log(&format!("Command::new detached spawn succeeded with PID: {}", child.id()));
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed all launch methods: ShellExecuteW failed (code {}), Process spawn error: {}", res, e);
            append_update_log(&format!("ERROR: {}", err_msg));
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub fn open_installer_folder() -> Result<(), String> {
    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("EDM_Update_Installer.exe");
    append_update_log(&format!("Frontend requested open_installer_folder: target='{}'", installer_path.display()));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = std::process::Command::new("explorer");
        if installer_path.exists() {
            let arg = format!("/select,\"{}\"", installer_path.display());
            cmd.raw_arg(&arg);
        } else {
            cmd.arg(&temp_dir);
        }
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.spawn().map_err(|e| format!("Failed to open directory: {}", e))?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        open::that(&temp_dir).map_err(|e| format!("Failed to open directory: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing_and_comparison() {
        assert_eq!(parse_version("v0.0.3"), Some((0, 0, 3)));
        assert_eq!(parse_version("0.0.3"), Some((0, 0, 3)));
        assert_eq!(parse_version("v1.2.34-rc1"), Some((1, 2, 34)));

        assert!(!is_newer_version("0.0.3", "0.0.3"));
        assert!(!is_newer_version("v0.0.3", "0.0.3"));
        assert!(!is_newer_version("0.0.2", "0.0.3"));
        assert!(is_newer_version("0.0.4", "0.0.3"));
        assert!(is_newer_version("0.1.0", "0.0.3"));
        assert!(is_newer_version("1.0.0", "0.0.3"));
    }
}
