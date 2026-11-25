use std::path::{Path, PathBuf};

use edm_core::adaptive::types::TorrentPowerConfig;
use edm_core::adaptive::AdaptiveConfig;
use reqwest::Client;

use crate::state::AppState;

/// Resolved execution settings derived from `app_settings` and per-download overrides.
pub(super) struct ExecutionConfig {
    pub adaptive: AdaptiveConfig,
    pub proxy: Option<reqwest::Proxy>,
    pub req_timeout_secs: u64,
    pub max_active_downloads: usize,
    pub torrent_power: Option<TorrentPowerConfig>,
}

impl ExecutionConfig {
    /// Build a configured `reqwest::Client` using the resolved timeout and proxy.
    pub fn build_http_client(&self) -> Client {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(self.req_timeout_secs))
            .redirect(reqwest::redirect::Policy::limited(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36 Edg/135.0.0.0");
        if let Some(ref p) = self.proxy {
            builder = builder.proxy(p.clone());
        }
        builder.build().unwrap_or_default()
    }
}

/// Load `app_settings` JSON from the persistent store, if present and valid.
pub(super) fn load_app_settings(state: &AppState) -> Option<serde_json::Value> {
    state
        .store
        .get_setting("app_settings")
        .ok()
        .flatten()
        .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
}

/// Read the configured maximum active downloads (minimum 1, default 3).
pub(super) fn read_max_active_downloads(settings_val: Option<&serde_json::Value>) -> usize {
    settings_val
        .and_then(|v| v.get("downloads"))
        .and_then(|d| d.get("maxActiveDownloads"))
        .and_then(|m| m.as_u64())
        .unwrap_or(3)
        .max(1) as usize
}

/// Build `AdaptiveConfig`, proxy, timeout, concurrency, and torrent power settings.
pub(super) fn build_execution_config(
    state: &AppState,
    settings_val: Option<&serde_json::Value>,
    max_connections: Option<u32>,
    is_p2p_or_torrent: bool,
) -> ExecutionConfig {
    let mut adaptive = AdaptiveConfig::default();
    if let Some(conns) = max_connections {
        adaptive.max_connections = conns.clamp(1, 64);
    }

    let mut proxy: Option<reqwest::Proxy> = None;
    let mut req_timeout_secs = 45u64;

    if let Some(settings) = settings_val {
        if let Some(dl) = settings.get("downloads") {
            if max_connections.is_none() {
                if let Some(conns) = dl.get("maxConnectionsPerFile").and_then(|v| v.as_u64()) {
                    adaptive.max_connections = (conns.clamp(1, 64)) as u32;
                }
            }
            if let Some(min_mb) = dl
                .get("minFileSizeForSegmentationMb")
                .and_then(|v| v.as_u64())
            {
                adaptive.min_chunk_size = min_mb.max(1) * 1024 * 1024;
            }
            if let Some(retries) = dl.get("maxRetries").and_then(|v| v.as_u64()) {
                adaptive.max_worker_retries = (retries.clamp(1, 20)) as u32;
            }
        }
        if let Some(net) = settings.get("network") {
            if let Some(t) = net.get("connectionTimeoutSeconds").and_then(|v| v.as_u64()) {
                if t > 0 {
                    req_timeout_secs = t;
                }
            }
            let proxy_mode = net
                .get("proxyMode")
                .and_then(|v| v.as_str())
                .unwrap_or("system");
            if proxy_mode == "manual" {
                let proto = net
                    .get("proxyProtocol")
                    .and_then(|v| v.as_str())
                    .unwrap_or("http");
                let host = net.get("proxyHost").and_then(|v| v.as_str()).unwrap_or("");
                let port = net
                    .get("proxyPort")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(8080);
                if !host.trim().is_empty() {
                    let p_url = format!("{}://{}:{}", proto, host.trim(), port);
                    if let Ok(p) = reqwest::Proxy::all(&p_url) {
                        proxy = Some(p);
                    }
                }
            }
        }
    }

    adaptive.max_connections_per_host = adaptive.max_connections;

    let torrent_power = if is_p2p_or_torrent {
        state
            .store
            .get_setting("torrent_power_defaults")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<TorrentPowerConfig>(&s).ok())
    } else {
        None
    };

    ExecutionConfig {
        adaptive,
        proxy,
        req_timeout_secs,
        max_active_downloads: read_max_active_downloads(settings_val),
        torrent_power,
    }
}

/// Classify a download into a canonical category name from its filename and target flags.
pub(super) fn classify_category(filename: &str, is_p2p: bool, is_stream: bool) -> &'static str {
    let ext = filename.split('.').next_back().unwrap_or("").to_lowercase();

    if is_p2p || ext == "torrent" {
        "Compressed"
    } else if is_stream
        || [
            "mp4", "mkv", "webm", "avi", "mov", "flv", "wmv", "m4v", "ts",
        ]
        .contains(&ext.as_str())
    {
        "Video"
    } else if ["mp3", "flac", "wav", "aac", "ogg", "m4a", "wma", "opus"].contains(&ext.as_str()) {
        "Music"
    } else if [
        "jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "bmp", "ico",
    ]
    .contains(&ext.as_str())
    {
        "Pictures"
    } else if [
        "pdf", "doc", "docx", "txt", "xlsx", "pptx", "rtf", "odt", "csv",
    ]
    .contains(&ext.as_str())
    {
        "Documents"
    } else if ["zip", "rar", "7z", "tar", "gz", "iso", "bin", "bz2", "xz"].contains(&ext.as_str()) {
        "Compressed"
    } else {
        "Programs"
    }
}

/// Resolve sanitized filename, destination directory (with category routing + security guard),
/// conflict policy handling, and final category.
pub(super) fn resolve_destination(
    settings_val: Option<&serde_json::Value>,
    url: &str,
    filename: Option<&str>,
    suggested_filename: Option<&str>,
    save_dir: Option<&PathBuf>,
    is_p2p: bool,
    is_stream: bool,
    is_resuming: bool,
) -> Result<(String, PathBuf, &'static str), String> {
    let raw_name = match filename {
        Some(f) if !f.trim().is_empty() && f != "download" && f != "download.bin" => f.to_string(),
        _ => {
            if let Some(suggested) = suggested_filename {
                suggested.to_string()
            } else {
                extract_filename_from_url(url)
            }
        }
    };
    let clean_name = edm_core::sanitizer::Sanitizer::sanitize_filename(&raw_name);

    let default_downloads_path = std::env::var("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Downloads"))
        .unwrap_or_else(|_| std::env::temp_dir());

    let mut save_path = save_dir
        .cloned()
        .unwrap_or_else(|| default_downloads_path.clone());

    let ext = clean_name
        .split('.')
        .next_back()
        .unwrap_or("")
        .to_lowercase();
    let initial_category = classify_category(&clean_name, is_p2p, is_stream);
    let is_p2p_or_archive =
        is_p2p || initial_category.eq_ignore_ascii_case("Compressed") || ext == "torrent";

    if save_dir.is_none() || save_path == default_downloads_path {
        if let Some(target) = resolve_auto_category_dir(settings_val, &ext, is_p2p_or_archive) {
            let _ = std::fs::create_dir_all(&target);
            save_path = target;
        }
    }

    let _ = std::fs::create_dir_all(&save_path);
    guard_restricted_system_dir(&save_path)?;

    let conflict_policy = settings_val
        .and_then(|v| v.get("storage"))
        .and_then(|s| s.get("filenameConflictPolicy"))
        .and_then(|p| p.as_str())
        .unwrap_or("ask");

    let final_clean_name =
        resolve_filename_conflict(&save_path, clean_name, conflict_policy, is_resuming)?;

    if let Some(warn_msg) = edm_core::security::check_double_extension(&final_clean_name) {
        tracing::warn!("[SECURITY] {}", warn_msg);
    }

    let final_category = classify_category(&final_clean_name, is_p2p, is_stream);
    Ok((final_clean_name, save_path, final_category))
}

fn extract_filename_from_url(url: &str) -> String {
    let path_part = url
        .split('/')
        .next_back()
        .unwrap_or("download.bin")
        .split('?')
        .next()
        .unwrap_or("download.bin");
    if path_part.is_empty() || path_part == "download" || path_part == "download.bin" {
        if let Ok(parsed) = url::Url::parse(url) {
            for (k, v) in parsed.query_pairs() {
                if (k == "filename" || k == "file" || k == "title" || k == "name")
                    && !v.trim().is_empty()
                {
                    return v.to_string();
                }
            }
        }
        "download.bin".to_string()
    } else {
        path_part.to_string()
    }
}

fn resolve_auto_category_dir(
    settings_val: Option<&serde_json::Value>,
    ext: &str,
    is_p2p_or_archive: bool,
) -> Option<PathBuf> {
    let mut routed_path: Option<PathBuf> = None;

    if let Some(settings) = settings_val {
        let is_auto = settings
            .get("storage")
            .and_then(|s| s.get("folderMode"))
            .and_then(|m| m.as_str())
            .map(|m| m == "auto")
            .unwrap_or(true);

        if is_auto {
            if is_p2p_or_archive {
                if let Some(cats_obj) = settings.get("categories").and_then(|c| c.as_object()) {
                    if let Some(val) = cats_obj
                        .get("archives")
                        .or_else(|| cats_obj.get("compressed"))
                    {
                        if let Some(s) = val.as_str() {
                            if !s.trim().is_empty() {
                                routed_path = Some(PathBuf::from(s.trim()));
                            }
                        } else if let Some(dest) =
                            val.get("destinationDirectory").and_then(|d| d.as_str())
                        {
                            if !dest.trim().is_empty() {
                                routed_path = Some(PathBuf::from(dest.trim()));
                            }
                        }
                    }
                }

                if routed_path.is_none() {
                    if let Some(cats) = settings.get("categories").and_then(|c| c.as_array()) {
                        for cat in cats {
                            if !cat.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true) {
                                continue;
                            }
                            let id = cat.get("id").and_then(|i| i.as_str()).unwrap_or("");
                            let name = cat.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            if id.eq_ignore_ascii_case("archives")
                                || id.eq_ignore_ascii_case("compressed")
                                || name.eq_ignore_ascii_case("archives")
                                || name.eq_ignore_ascii_case("compressed")
                            {
                                if let Some(dest) =
                                    cat.get("destinationDirectory").and_then(|d| d.as_str())
                                {
                                    if !dest.trim().is_empty() {
                                        routed_path = Some(PathBuf::from(dest.trim()));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if routed_path.is_none() {
                if let Some(cats) = settings.get("categories").and_then(|c| c.as_array()) {
                    for cat in cats {
                        if !cat.get("enabled").and_then(|e| e.as_bool()).unwrap_or(true) {
                            continue;
                        }
                        let matches = cat
                            .get("extensions")
                            .and_then(|exts| exts.as_array())
                            .map(|arr| {
                                arr.iter().any(|e| {
                                    e.as_str()
                                        .map(|s| {
                                            s.trim_start_matches('.').eq_ignore_ascii_case(ext)
                                        })
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false);

                        if matches {
                            if let Some(dest) =
                                cat.get("destinationDirectory").and_then(|d| d.as_str())
                            {
                                if !dest.trim().is_empty() {
                                    routed_path = Some(PathBuf::from(dest.trim()));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if routed_path.is_none() {
        let dirs = crate::os::get_system_user_dirs_internal();
        if is_p2p_or_archive {
            routed_path = Some(PathBuf::from(dirs.downloads).join("Archives"));
        } else if [
            "mp4", "mkv", "webm", "avi", "mov", "flv", "wmv", "m4v", "ts",
        ]
        .contains(&ext)
        {
            routed_path = Some(PathBuf::from(dirs.videos));
        } else if ["mp3", "flac", "wav", "aac", "ogg", "m4a", "wma", "opus"].contains(&ext) {
            routed_path = Some(PathBuf::from(dirs.music));
        } else if [
            "jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "bmp", "ico",
        ]
        .contains(&ext)
        {
            routed_path = Some(PathBuf::from(dirs.pictures));
        } else if [
            "pdf", "doc", "docx", "txt", "xlsx", "pptx", "rtf", "odt", "csv",
        ]
        .contains(&ext)
        {
            routed_path = Some(PathBuf::from(dirs.documents));
        } else if ["zip", "rar", "7z", "tar", "gz", "iso", "bin", "bz2", "xz"].contains(&ext) {
            routed_path = Some(PathBuf::from(dirs.downloads).join("Archives"));
        } else if ["exe", "msi", "apk", "bat", "cmd", "ps1", "appx", "msix"].contains(&ext) {
            routed_path = Some(PathBuf::from(dirs.downloads).join("Programs"));
        }
    }

    routed_path
}

fn guard_restricted_system_dir(save_path: &Path) -> Result<(), String> {
    let path_str = save_path.to_string_lossy().to_lowercase();
    if path_str.contains(r"\windows\system32")
        || path_str.contains(r"\windows\syswow64")
        || path_str.contains(r"\start menu\programs\startup")
        || path_str.contains(r"\microsoft\windows\start menu")
    {
        return Err(
            "Security violation: Target download directory is a restricted Windows system location"
                .into(),
        );
    }
    Ok(())
}

fn auto_rename(save_path: &Path, clean_name: String) -> String {
    let resolved_path = edm_core::sanitizer::Sanitizer::resolve_duplicate(save_path, &clean_name);
    resolved_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or(clean_name)
}

fn resolve_filename_conflict(
    save_path: &Path,
    clean_name: String,
    conflict_policy: &str,
    is_resuming: bool,
) -> Result<String, String> {
    let target_file_path = save_path.join(&clean_name);
    if is_resuming || !target_file_path.exists() {
        return Ok(clean_name);
    }

    match conflict_policy {
        "overwrite" => Ok(clean_name),
        "skip" => Err(
            "Download skipped: target file already exists in destination (conflict policy: skip)"
                .into(),
        ),
        "rename" => Ok(auto_rename(save_path, clean_name)),
        _ => {
            #[cfg(windows)]
            {
                use std::ffi::OsStr;
                use std::os::windows::ffi::OsStrExt;
                #[link(name = "user32")]
                extern "system" {
                    fn MessageBoxW(
                        hWnd: *mut std::ffi::c_void,
                        lpText: *const u16,
                        lpCaption: *const u16,
                        uType: u32,
                    ) -> i32;
                }
                const MB_YESNOCANCEL: u32 = 0x00000003;
                const MB_ICONQUESTION: u32 = 0x00000020;
                const IDYES: i32 = 6;
                const IDNO: i32 = 7;

                let prompt_text = format!(
                    "File Conflict Detected:\n\nA file named '{}' already exists in the destination folder:\n{}\n\n• Click [Yes] to OVERWRITE the existing file.\n• Click [No] to AUTO-RENAME to avoid collision.\n• Click [Cancel] to abort this download.",
                    clean_name,
                    save_path.display()
                );
                let caption = "EDM - File Collision Conflict";

                let w_text: Vec<u16> = OsStr::new(&prompt_text)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let w_cap: Vec<u16> = OsStr::new(caption)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();

                let ret = unsafe {
                    MessageBoxW(
                        std::ptr::null_mut(),
                        w_text.as_ptr(),
                        w_cap.as_ptr(),
                        MB_YESNOCANCEL | MB_ICONQUESTION,
                    )
                };

                match ret {
                    IDYES => Ok(clean_name),
                    IDNO => Ok(auto_rename(save_path, clean_name)),
                    _ => Err("Download cancelled by user (file collision)".into()),
                }
            }
            #[cfg(not(windows))]
            {
                Ok(auto_rename(save_path, clean_name))
            }
        }
    }
}
