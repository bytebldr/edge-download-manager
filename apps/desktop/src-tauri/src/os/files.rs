use std::path::{Path, PathBuf};

pub fn validate_url_security(raw_url: &str) -> Result<url::Url, String> {
    let trimmed = raw_url.trim();
    let lower = trimmed.to_ascii_lowercase();

    // 1. BitTorrent Magnet URIs (handled by bittorrent_pack)
    if lower.starts_with("magnet:") {
        return url::Url::parse(trimmed)
            .or_else(|_| {
                url::Url::parse(&format!(
                    "https://magnet.internal/?raw={}",
                    urlencoding::encode(trimmed)
                ))
            })
            .map_err(|e| format!("Invalid magnet URI: {}", e));
    }

    // 1b. BitTorrent Local .torrent File Path or file:// URI
    let local_path = if lower.starts_with("file://") {
        url::Url::parse(trimmed)
            .ok()
            .and_then(|u| u.to_file_path().ok())
    } else {
        Some(PathBuf::from(trimmed))
    };

    if let Some(ref lp) = local_path {
        if lp.is_absolute()
            && lp
                .extension()
                .map_or(false, |e| e.eq_ignore_ascii_case("torrent"))
            && lp.exists()
        {
            return url::Url::from_file_path(lp)
                .map_err(|_| "Failed to resolve file path to URL".to_string());
        }
    }

    // 2. eDonkey2000 URIs (handled by ed2k_pack)
    if lower.starts_with("ed2k://") {
        return url::Url::parse(trimmed)
            .or_else(|_| {
                url::Url::parse(&format!(
                    "https://ed2k.internal/?raw={}",
                    urlencoding::encode(trimmed)
                ))
            })
            .map_err(|e| format!("Invalid eD2k URI: {}", e));
    }

    let parsed = url::Url::parse(trimmed)
        .map_err(|e| format!("Security violation: Invalid URL format: {}", e))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" && scheme != "ftp" && scheme != "ftps" {
        return Err(format!(
            "Security violation: Protocol '{}' is not supported",
            scheme
        ));
    }

    if let Some(host) = parsed.host() {
        match host {
            url::Host::Ipv4(ip) => {
                if ip.is_link_local() {
                    return Err("Security violation: Blocked access to link-local address".into());
                }
            }
            url::Host::Ipv6(ip) => {
                if ip.is_unicast_link_local() || format!("{}", ip) == "fd00:ec2::254" {
                    return Err(
                        "Security violation: Blocked access to IPv6 metadata or link-local address"
                            .into(),
                    );
                }
            }
            url::Host::Domain(domain) => {
                let h = domain.to_lowercase();
                if h == "169.254.169.254"
                    || h == "metadata.google.internal"
                    || h == "instance-data"
                    || h.ends_with(".internal")
                {
                    return Err(
                        "Security violation: Blocked access to cloud metadata endpoint".into(),
                    );
                }
            }
        }
    }
    Ok(parsed)
}

pub fn open_file(save_dir: &str, filename: &str) {
    let file_path = PathBuf::from(save_dir).join(filename);
    edm_core::security::open_file_safely(&file_path);
}

pub fn open_folder(save_dir: &str, filename: &str) {
    let save_path = Path::new(save_dir);
    if let Err(e) = secure_open_folder(save_path, filename) {
        tracing::warn!("Failed to open folder securely: {}", e);
        let _ = std::process::Command::new("explorer.exe")
            .arg(save_dir)
            .spawn();
    }
}

pub fn secure_open_file(
    save_dir: &std::path::Path,
    filename: &str,
    status: &str,
) -> Result<(), String> {
    if status != "completed" {
        return Err("Cannot open file: download is not completed".into());
    }

    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err("Invalid file path".into());
    }

    let target = save_dir.join(filename);
    if !target.exists() {
        return Err("File does not exist on disk".into());
    }

    let canonical_target = target
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path: {}", e))?;
    let canonical_dir = save_dir
        .canonicalize()
        .map_err(|e| format!("Cannot resolve dir: {}", e))?;

    if !canonical_target.starts_with(&canonical_dir) {
        return Err("Access denied: path traversal detected".into());
    }

    open::that(&canonical_target).map_err(|e| format!("Failed to open file: {}", e))?;
    Ok(())
}

pub fn secure_open_folder(save_dir: &std::path::Path, filename: &str) -> Result<(), String> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err("Invalid file path".into());
    }

    let target = save_dir.join(filename);
    if !target.exists() {
        if save_dir.exists() {
            open::that(save_dir).map_err(|e| format!("Failed to open directory: {}", e))?;
            return Ok(());
        }
        return Err("Directory does not exist".into());
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = std::process::Command::new("explorer");
        let arg = format!("/select,\"{}\"", target.display());
        cmd.raw_arg(&arg);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd
            .spawn()
            .map_err(|e| format!("Failed to highlight file: {}", e))?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        open::that(save_dir).map_err(|e| format!("Failed to open directory: {}", e))?;
        Ok(())
    }
}

/// Send file or directory to OS Recycle Bin safely instead of permanent unlink
#[cfg(windows)]
pub fn send_to_recycle_bin(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    wide.push(0);

    #[repr(C, packed(2))]
    #[allow(clippy::upper_case_acronyms)]
    struct SHFILEOPSTRUCTW {
        hwnd: *mut std::ffi::c_void,
        w_func: u32,
        p_from: *const u16,
        p_to: *const u16,
        f_flags: u16,
        f_any_operations_aborted: i32,
        h_name_mappings: *mut std::ffi::c_void,
        lpsz_progress_title: *const u16,
    }

    const FO_DELETE: u32 = 0x0003;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_SILENT: u16 = 0x0004;

    extern "system" {
        fn SHFileOperationW(lpFileOp: *mut SHFILEOPSTRUCTW) -> i32;
    }

    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        w_func: FO_DELETE,
        p_from: wide.as_ptr(),
        p_to: std::ptr::null(),
        f_flags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT,
        f_any_operations_aborted: 0,
        h_name_mappings: std::ptr::null_mut(),
        lpsz_progress_title: std::ptr::null(),
    };

    let ret = unsafe { SHFileOperationW(&mut op) };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(ret))
    }
}

#[cfg(not(windows))]
pub fn send_to_recycle_bin(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

pub fn get_system_downloads_dir_internal() -> String {
    #[cfg(windows)]
    {
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let p = PathBuf::from(user_profile).join("Downloads");
            if p.exists() {
                return p.to_string_lossy().to_string();
            }
        }
    }
    std::env::temp_dir().to_string_lossy().to_string()
}

pub async fn select_download_directory_internal(
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let initial = default_path.as_deref().unwrap_or("").replace('\'', "''");
            let script = format!(
                "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description = 'Select EDM Downloads Folder'; if ('{}' -ne '') {{ $d.SelectedPath = '{}' }}; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ Write-Output $d.SelectedPath }}",
                initial, initial
            );
            let output = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| e.to_string())?;

            let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if res.is_empty() {
                Ok(None)
            } else {
                Ok(Some(res))
            }
        }
        #[cfg(not(windows))]
        {
            Ok(None)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn select_torrent_file_internal() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let script = "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Title = 'Select .torrent File'; $d.Filter = 'BitTorrent Files (*.torrent)|*.torrent|All Files (*.*)|*.*'; if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $d.FileName }";
            let output = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| e.to_string())?;

            let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if res.is_empty() {
                Ok(None)
            } else {
                Ok(Some(res))
            }
        }
        #[cfg(not(windows))]
        {
            Ok(None)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn get_disk_free_space_internal(path: Option<String>) -> Result<serde_json::Value, String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let target_path = path.unwrap_or_else(|| "C:\\".to_string());
        let root = if target_path.len() >= 2 && target_path.chars().nth(1) == Some(':') {
            format!("{}:\\", &target_path[0..1])
        } else {
            "C:\\".to_string()
        };
        let wide: Vec<u16> = std::ffi::OsStr::new(&root)
            .encode_wide()
            .chain(Some(0))
            .collect();

        #[link(name = "kernel32")]
        extern "system" {
            fn GetDiskFreeSpaceExW(
                lpDirectoryName: *const u16,
                lpFreeBytesAvailableToCaller: *mut u64,
                lpTotalNumberOfBytes: *mut u64,
                lpTotalNumberOfFreeBytes: *mut u64,
            ) -> i32;
        }

        let mut free_bytes_avail: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free: u64 = 0;

        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_bytes_avail,
                &mut total_bytes,
                &mut total_free,
            )
        };

        if ok != 0 && total_bytes > 0 {
            let pct_free = ((total_free as f64 / total_bytes as f64) * 100.0).round() as u64;
            return Ok(serde_json::json!({
                "drive": root.trim_end_matches('\\'),
                "total_bytes": total_bytes,
                "free_bytes": total_free,
                "pct_free": pct_free
            }));
        }
    }
    Ok(serde_json::json!({
        "drive": "C:",
        "total_bytes": 0,
        "free_bytes": 0,
        "pct_free": 50
    }))
}
