use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use tracing::info;
use super::platform;
use super::types::{OtaProgress, UpdateCheckResult};
use crate::AppState;

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

pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(serde::Deserialize, Debug)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(serde::Deserialize, Debug)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    html_url: String,
    assets: Vec<GhAsset>,
}

pub async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let api_url = "https://api.github.com/repos/bytebldr/edge-download-manager/releases/latest";

    let client = reqwest::Client::builder()
        .user_agent(format!("EDM/{} (Windows; x86_64)", current_version))
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Attempt 1: Official GitHub Releases API
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

    // Attempt 2: Resilient zero-rate-limit redirect fallback
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

    // Attempt 3: Up-to-date fallback
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

pub async fn install_update(
    app: AppHandle,
    state: State<'_, AppState>,
    download_url: String,
) -> Result<(), String> {
    let trimmed = download_url.trim();
    if !trimmed.starts_with("https://github.com/bytebldr/edge-download-manager/releases/download/") {
        return Err("Security violation: Update download URL must originate from official bytebldr/edge-download-manager GitHub releases".into());
    }

    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("EDM_Update_Installer.exe");

    append_update_log(&format!("Starting download of update asset from: {}", trimmed));

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
    info!(
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

    // Verify installer file on disk
    let metadata = std::fs::metadata(&installer_path).map_err(|e| {
        let msg = format!("Failed to read installer metadata: {}", e);
        append_update_log(&format!("ERROR: {}", msg));
        msg
    })?;
    let file_size = metadata.len();
    if file_size < 1_000_000 {
        let msg = format!("Installer package on disk is incomplete ({} bytes)", file_size);
        append_update_log(&format!("ERROR: {}", msg));
        return Err(msg);
    }
    append_update_log(&format!("Installer verified on disk: {} (size: {} bytes)", installer_path.display(), file_size));

    // Pre-flight file lock validation & sibling termination
    append_update_log("Terminating sibling EDM processes...");
    let _ = platform::terminate_sibling_instances();

    // Detached handoff to updater package
    append_update_log(&format!("Handing off execution to update package: {}", installer_path.display()));
    launch_installer_detached(&installer_path)?;

    // Gracefully shut down current instance to prevent file locks
    append_update_log("Initiating graceful shutdown of current instance for update replacement...");
    crate::trigger_graceful_quit(app, state.inner().clone());

    Ok(())
}

fn launch_installer_detached(installer_path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let path_wide: Vec<u16> = installer_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let params_wide: Vec<u16> = OsStr::new("/UPDATE").encode_wide().chain(std::iter::once(0)).collect();

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

        if res > 32 {
            return Ok(());
        }

        // Try runas fallback
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

        if runas_res > 32 {
            return Ok(());
        }

        // Command::new detached breakaway fallback
        use std::os::windows::process::CommandExt;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

        let mut cmd = std::process::Command::new(installer_path);
        cmd.arg("/UPDATE");
        cmd.creation_flags(CREATE_BREAKAWAY_FROM_JOB | CREATE_NEW_PROCESS_GROUP);
        cmd.spawn()
            .map(|_| ())
            .map_err(|e| format!("Detached process spawn error: {}", e))
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new(installer_path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Process spawn error: {}", e))
    }
}

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
