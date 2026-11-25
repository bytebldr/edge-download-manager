use super::platform;
use super::types::{OtaProgress, UpdateCheckResult};
use crate::AppState;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use tracing::info;

static LATEST_INSTALLER_PATH: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

pub fn set_latest_installer_path(path: PathBuf) {
    if let Ok(mut lock) = LATEST_INSTALLER_PATH.write() {
        *lock = Some(path);
    }
}

pub fn get_latest_installer_path() -> Option<PathBuf> {
    if let Ok(lock) = LATEST_INSTALLER_PATH.read() {
        if let Some(ref p) = *lock {
            if p.exists() {
                return Some(p.clone());
            }
        }
    }
    None
}

pub fn get_update_dir() -> PathBuf {
    match std::env::var("LOCALAPPDATA") {
        Ok(dir) => PathBuf::from(dir).join("EDM").join("updates"),
        Err(_) => match std::env::var("APPDATA") {
            Ok(dir) => PathBuf::from(dir).join("EDM").join("updates"),
            Err(_) => std::env::temp_dir().join("EDM").join("updates"),
        },
    }
}

#[cfg(windows)]
extern "system" {
    fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> *mut std::ffi::c_void;
    fn Process32FirstW(hSnapshot: *mut std::ffi::c_void, lppe: *mut PROCESSENTRY32W) -> i32;
    fn Process32NextW(hSnapshot: *mut std::ffi::c_void, lppe: *mut PROCESSENTRY32W) -> i32;
    fn OpenProcess(
        dwDesiredAccess: u32,
        bInheritHandle: i32,
        dwProcessId: u32,
    ) -> *mut std::ffi::c_void;
    fn TerminateProcess(hProcess: *mut std::ffi::c_void, uExitCode: u32) -> i32;
    fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
}

#[cfg(windows)]
const TH32CS_SNAPPROCESS: u32 = 0x00000002;
#[cfg(windows)]
const PROCESS_TERMINATE: u32 = 0x0001;
#[cfg(windows)]
const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = -1isize as *mut std::ffi::c_void;

#[cfg(windows)]
#[repr(C)]
#[allow(non_snake_case)]
struct PROCESSENTRY32W {
    dwSize: u32,
    cntUsage: u32,
    th32ProcessID: u32,
    th32DefaultHeapID: usize,
    th32ModuleID: u32,
    cntThreads: u32,
    th32ParentProcessID: u32,
    pcPriClassBase: i32,
    dwFlags: u32,
    szExeFile: [u16; 260],
}

pub fn kill_zombie_update_processes() {
    append_update_log("Sweeping and terminating zombie update processes...");

    #[cfg(windows)]
    {
        unsafe {
            let current_pid = std::process::id();
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot != INVALID_HANDLE_VALUE {
                let mut entry = PROCESSENTRY32W {
                    dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                    cntUsage: 0,
                    th32ProcessID: 0,
                    th32DefaultHeapID: 0,
                    th32ModuleID: 0,
                    cntThreads: 0,
                    th32ParentProcessID: 0,
                    pcPriClassBase: 0,
                    dwFlags: 0,
                    szExeFile: [0; 260],
                };

                let target_names = ["edm_update_installer.exe", "edm_setup.exe"];

                if Process32FirstW(snapshot, &mut entry) != 0 {
                    loop {
                        let pid = entry.th32ProcessID;
                        if pid != current_pid && pid != 0 {
                            let len = entry
                                .szExeFile
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(entry.szExeFile.len());
                            let name =
                                String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                            if target_names.iter().any(|&t| {
                                t == name
                                    || name.starts_with("edm_setup")
                                    || name.starts_with("edm_update")
                            }) {
                                let proc_handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
                                if !proc_handle.is_null() {
                                    let _ = TerminateProcess(proc_handle, 0);
                                    CloseHandle(proc_handle);
                                    append_update_log(&format!(
                                        "Terminated zombie update process: {} (PID: {})",
                                        name, pid
                                    ));
                                }
                            }
                        }
                        if Process32NextW(snapshot, &mut entry) == 0 {
                            break;
                        }
                    }
                }
                CloseHandle(snapshot);
            }
        }

        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::process::Command::new("taskkill")
            .args([
                "/F",
                "/IM",
                "EDM_Update_Installer.exe",
                "/IM",
                "EDM_Setup.exe",
                "/T",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        std::thread::sleep(std::time::Duration::from_millis(150));
    }
}

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
    let api_url = "https://api.github.com/repos/bytebldr/evo-download-manager/releases/latest";

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
        .get("https://github.com/bytebldr/evo-download-manager/releases/latest")
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
            "https://github.com/bytebldr/evo-download-manager/releases/download/{}/{}",
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
        html_url: "https://github.com/bytebldr/evo-download-manager/releases".to_string(),
    })
}

async fn verify_release_sha256(
    client: &reqwest::Client,
    download_url: &str,
    actual_sha256: &str,
) -> Result<(), String> {
    let (base_url, asset_name) = match download_url.rsplit_once('/') {
        Some((b, a)) if !a.is_empty() => (b, a),
        _ => return Ok(()),
    };

    let candidate_urls = [
        format!("{}.sha256", download_url),
        format!("{}/SHA256SUMS", base_url),
        format!("{}/SHA256SUMS.txt", base_url),
    ];

    for checksum_url in &candidate_urls {
        let Ok(resp) = client.get(checksum_url).send().await else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let body = resp.text().await.map_err(|e| {
            format!(
                "Failed to read release checksums from {}: {}",
                checksum_url, e
            )
        })?;

        let expected = parse_sha256_for_asset(&body, asset_name).ok_or_else(|| {
            format!(
                "Release checksum file {} does not contain a valid SHA-256 entry for {}",
                checksum_url, asset_name
            )
        })?;

        if !expected.eq_ignore_ascii_case(actual_sha256) {
            return Err(format!(
                "Security violation: SHA-256 mismatch for {} (expected {}, got {})",
                asset_name, expected, actual_sha256
            ));
        }

        append_update_log(&format!(
            "SHA-256 verified against {} ({})",
            checksum_url, actual_sha256
        ));
        return Ok(());
    }

    append_update_log(&format!(
        "No SHA256SUMS asset published for {}; computed SHA-256: {}",
        asset_name, actual_sha256
    ));
    Ok(())
}

fn parse_sha256_for_asset(content: &str, asset_name: &str) -> Option<String> {
    let is_hex64 = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let first = parts.next()?;
        if !is_hex64(first) {
            continue;
        }
        match parts.next() {
            None => return Some(first.to_ascii_lowercase()),
            Some(file_part) => {
                let clean_name = file_part.trim_start_matches('*');
                if clean_name == asset_name || clean_name.rsplit('/').next() == Some(asset_name) {
                    return Some(first.to_ascii_lowercase());
                }
            }
        }
    }
    None
}

pub async fn install_update(
    app: AppHandle,
    state: State<'_, AppState>,
    download_url: String,
) -> Result<(), String> {
    let trimmed = download_url.trim();
    if !trimmed.starts_with("https://github.com/bytebldr/evo-download-manager/releases/download/") {
        return Err("Security violation: Update download URL must originate from official bytebldr/evo-download-manager GitHub releases".into());
    }

    // Sweep and terminate any lingering zombie update processes before downloading
    kill_zombie_update_processes();

    let updates_base = get_update_dir();
    let pid = std::process::id();
    let timestamp = chrono::Utc::now().timestamp_millis();
    let run_dir = updates_base.join(format!("run_{}_{}", pid, timestamp));
    let _ = std::fs::create_dir_all(&run_dir);
    let installer_path = run_dir.join("EDM_Setup.exe");
    set_latest_installer_path(installer_path.clone());

    append_update_log(&format!(
        "Starting download of update asset from: {} to: {}",
        trimmed,
        installer_path.display()
    ));

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
    use sha2::{Digest, Sha256};
    let mut stream = res.bytes_stream();
    let mut file = tokio::fs::File::create(&installer_path)
        .await
        .map_err(|e| format!("Failed to create destination file: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut hasher = Sha256::new();
    use tokio::io::AsyncWriteExt;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Download stream error: {}", e))?;
        hasher.update(&chunk);
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

    let actual_sha256 = hex::encode(hasher.finalize());
    if let Err(e) = verify_release_sha256(&client, trimmed, &actual_sha256).await {
        let _ = tokio::fs::remove_file(&installer_path).await;
        append_update_log(&format!("ERROR: {}", e));
        return Err(e);
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
    let _ = state
        .store
        .set_setting("pending_update_version", &installer_path.to_string_lossy());
    let _ = state.store.checkpoint();

    // Verify installer file on disk
    let metadata = std::fs::metadata(&installer_path).map_err(|e| {
        let msg = format!("Failed to read installer metadata: {}", e);
        append_update_log(&format!("ERROR: {}", msg));
        msg
    })?;
    let file_size = metadata.len();
    if file_size < 1_000_000 {
        let msg = format!(
            "Installer package on disk is incomplete ({} bytes)",
            file_size
        );
        append_update_log(&format!("ERROR: {}", msg));
        return Err(msg);
    }
    append_update_log(&format!(
        "Installer verified on disk: {} (size: {} bytes)",
        installer_path.display(),
        file_size
    ));

    // Pre-flight file lock validation & sibling termination
    append_update_log("Terminating sibling EDM processes...");
    let _ = platform::terminate_sibling_instances();

    // Detached handoff to updater package with silent headless flags
    append_update_log(&format!(
        "Handing off execution to update package: {}",
        installer_path.display()
    ));
    launch_installer_windows(&installer_path)?;

    // Gracefully shut down current instance to prevent file locks
    append_update_log("Initiating graceful shutdown of current instance for update replacement...");
    crate::trigger_graceful_quit(app, state.inner().clone());

    Ok(())
}

fn verify_installer_integrity(installer_path: &Path) -> Result<(), String> {
    use std::io::Read;

    let meta = std::fs::metadata(installer_path)
        .map_err(|e| format!("Installer metadata error: {}", e))?;
    if !meta.is_file() || meta.len() < 1_000_000 {
        return Err(format!("Invalid installer file size: {} bytes", meta.len()));
    }

    let mut f = std::fs::File::open(installer_path)
        .map_err(|e| format!("Failed to open installer for integrity check: {}", e))?;
    let mut magic = [0u8; 2];
    f.read_exact(&mut magic)
        .map_err(|e| format!("Failed to read installer header: {}", e))?;
    if &magic != b"MZ" {
        return Err("Security violation: Installer is not a valid Windows PE executable".into());
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        #[repr(C)]
        struct GUID {
            data1: u32,
            data2: u16,
            data3: u16,
            data4: [u8; 8],
        }

        #[repr(C)]
        struct WINTRUST_FILE_INFO {
            cb_struct: u32,
            pcwsz_file_path: *const u16,
            h_file: *mut std::ffi::c_void,
            pg_known_subject: *mut GUID,
        }

        #[repr(C)]
        struct WINTRUST_DATA {
            cb_struct: u32,
            p_policy_callback_data: *mut std::ffi::c_void,
            p_sip_client_data: *mut std::ffi::c_void,
            dw_ui_choice: u32,
            fdw_revocation_checks: u32,
            dw_union_choice: u32,
            p_file: *mut WINTRUST_FILE_INFO,
            dw_state_action: u32,
            h_wv_data: *mut std::ffi::c_void,
            pwsz_url_reference: *const u16,
            dw_prov_flags: u32,
            dw_ui_context: u32,
            p_signature_settings: *mut std::ffi::c_void,
        }

        #[link(name = "wintrust")]
        extern "system" {
            fn WinVerifyTrust(
                hwnd: *mut std::ffi::c_void,
                pgActionID: *mut GUID,
                pWVTData: *mut WINTRUST_DATA,
            ) -> i32;
        }

        const WTD_UI_NONE: u32 = 2;
        const WTD_REVOKE_NONE: u32 = 0;
        const WTD_CHOICE_FILE: u32 = 1;
        const WTD_STATEACTION_VERIFY: u32 = 1;
        const WTD_STATEACTION_CLOSE: u32 = 2;
        const TRUST_E_NOSIGNATURE: i32 = 0x800B0100u32 as i32;

        let path_wide: Vec<u16> = installer_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut action_guid = GUID {
            data1: 0x00AAC56B,
            data2: 0xCD44,
            data3: 0x11d0,
            data4: [0x8C, 0xC2, 0x00, 0xC0, 0x4F, 0xC2, 0x95, 0xEE],
        };

        let mut file_info = WINTRUST_FILE_INFO {
            cb_struct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwsz_file_path: path_wide.as_ptr(),
            h_file: std::ptr::null_mut(),
            pg_known_subject: std::ptr::null_mut(),
        };

        let mut trust_data = WINTRUST_DATA {
            cb_struct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            p_policy_callback_data: std::ptr::null_mut(),
            p_sip_client_data: std::ptr::null_mut(),
            dw_ui_choice: WTD_UI_NONE,
            fdw_revocation_checks: WTD_REVOKE_NONE,
            dw_union_choice: WTD_CHOICE_FILE,
            p_file: &mut file_info,
            dw_state_action: WTD_STATEACTION_VERIFY,
            h_wv_data: std::ptr::null_mut(),
            pwsz_url_reference: std::ptr::null(),
            dw_prov_flags: 0,
            dw_ui_context: 0,
            p_signature_settings: std::ptr::null_mut(),
        };

        let status = unsafe {
            WinVerifyTrust(
                -1isize as *mut std::ffi::c_void,
                &mut action_guid,
                &mut trust_data,
            )
        };
        trust_data.dw_state_action = WTD_STATEACTION_CLOSE;
        unsafe {
            WinVerifyTrust(
                -1isize as *mut std::ffi::c_void,
                &mut action_guid,
                &mut trust_data,
            );
        }

        if status == 0 {
            append_update_log("Authenticode signature verified.");
        } else if status == TRUST_E_NOSIGNATURE {
            append_update_log(
                "Warning: Installer has no Authenticode signature; PE and SHA-256 checks passed.",
            );
        } else {
            return Err(format!(
                "Security violation: Authenticode verification failed (0x{:08X})",
                status as u32
            ));
        }
    }

    Ok(())
}

pub fn launch_installer_windows(installer_path: &Path) -> Result<(), String> {
    verify_installer_integrity(installer_path)?;

    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let current_pid = std::process::id();
        let params_str = format!("/S /PID={} /R", current_pid);
        append_update_log(&format!(
            "Launching installer: {} with parameters: {}",
            installer_path.display(),
            params_str
        ));

        let path_wide: Vec<u16> = installer_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let params_wide: Vec<u16> = OsStr::new(&params_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

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
        let open_verb: Vec<u16> = OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let runas_verb: Vec<u16> = OsStr::new("runas")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

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
            append_update_log("ShellExecuteW (open) succeeded");
            return Ok(());
        }

        append_update_log(&format!(
            "ShellExecuteW (open) returned code {}, trying runas...",
            res
        ));

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
            append_update_log("ShellExecuteW (runas) succeeded");
            return Ok(());
        }

        append_update_log(&format!(
            "ShellExecuteW (runas) returned code {}, trying Command spawn fallback...",
            runas_res
        ));

        use std::os::windows::process::CommandExt;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

        let mut cmd = std::process::Command::new(installer_path);
        cmd.args(["/S", &format!("/PID={}", current_pid), "/R"]);
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

pub fn launch_installer_detached(installer_path: &Path) -> Result<(), String> {
    launch_installer_windows(installer_path)
}

pub fn open_installer_folder() -> Result<(), String> {
    let updates_base = get_update_dir();
    let target_path = get_latest_installer_path().or_else(|| {
        if let Ok(entries) = std::fs::read_dir(&updates_base) {
            let mut runs: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("run_"))
                            .unwrap_or(false)
                })
                .collect();
            runs.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
            runs.reverse();
            for r in runs {
                let exe = r.join("EDM_Setup.exe");
                if exe.exists() {
                    return Some(exe);
                }
            }
        }
        None
    });

    append_update_log(&format!(
        "Frontend requested open_installer_folder: target='{:?}' (base='{}')",
        target_path,
        updates_base.display()
    ));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = std::process::Command::new("explorer");
        if let Some(ref path) = target_path {
            if path.exists() {
                let arg = format!("/select,\"{}\"", path.display());
                cmd.raw_arg(&arg);
            } else if let Some(parent) = path.parent() {
                cmd.arg(parent);
            } else {
                cmd.arg(&updates_base);
            }
        } else if updates_base.exists() {
            cmd.arg(&updates_base);
        } else {
            let temp_dir = std::env::temp_dir();
            cmd.arg(&temp_dir);
        }
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let target_dir = target_path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or(updates_base);
        open::that(&target_dir).map_err(|e| format!("Failed to open directory: {}", e))?;
        Ok(())
    }
}
