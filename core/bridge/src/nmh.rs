use crate::config;
use crate::handlers::handle_message;
use chrono::Utc;
use edm_ipc::{AppReply, BrowserDiagnostic, DiagnosticsReport, ExtensionMessage};
use edm_store::Store;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(windows)]
pub mod win_lock {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(
            lpMutexAttributes: *mut std::ffi::c_void,
            bInitialOwner: i32,
            lpName: *const u16,
        ) -> *mut std::ffi::c_void;
        fn WaitForSingleObject(hHandle: *mut std::ffi::c_void, dwMilliseconds: u32) -> u32;
        fn ReleaseMutex(hMutex: *mut std::ffi::c_void) -> i32;
        fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
    }

    pub struct NamedLock {
        handle: *mut std::ffi::c_void,
    }

    impl NamedLock {
        pub fn acquire(name: &str, timeout_ms: u32) -> Option<Self> {
            let wide: Vec<u16> = OsStr::new(name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, wide.as_ptr()) };
            if handle.is_null() {
                return None;
            }
            let wait_res = unsafe { WaitForSingleObject(handle, timeout_ms) };
            if wait_res == 0 || wait_res == 0x80 {
                Some(Self { handle })
            } else {
                unsafe {
                    CloseHandle(handle);
                }
                None
            }
        }
    }

    impl Drop for NamedLock {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.handle);
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
const EMBEDDED_WEBVIEW2_LOADER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/vendor/win-x64/WebView2Loader.dll"
));

/// Ensure WebView2Loader.dll is co-located with the target executable.
/// Tauri/wry on Windows dynamically loads this DLL; without it, Windows PE loader
/// displays "System Error: The code execution cannot proceed because WebView2Loader.dll was not found."
fn ensure_webview2_loader(exe: &Path) {
    let parent = match exe.parent() {
        Some(p) => p,
        None => return,
    };
    let target_dll = parent.join("WebView2Loader.dll");
    if target_dll.exists() && target_dll.metadata().map(|m| m.len()).unwrap_or(0) > 1000 {
        return;
    }

    // Instant self-healing via embedded genuine binary
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        let pid = std::process::id();
        let tmp = parent.join(format!("WebView2Loader.dll.tmp.{}", pid));
        if std::fs::write(&tmp, EMBEDDED_WEBVIEW2_LOADER).is_ok() {
            let _ = std::fs::rename(&tmp, &target_dll);
            let _ = std::fs::remove_file(&tmp);
            tracing::info!(
                "Instantly self-healed WebView2Loader.dll in {}",
                parent.display()
            );
            return;
        }
    }

    let mut candidate_sources = Vec::new();

    // 1. Check nested Tauri release bundle in exe's parent tree
    candidate_sources.push(parent.join(r"_up_\_up_\_up_\target\release\WebView2Loader.dll"));
    candidate_sources.push(parent.join(r"resources\WebView2Loader.dll"));
    candidate_sources.push(parent.join(r"resources\target\release\WebView2Loader.dll"));

    // 2. Check %LOCALAPPDATA%\EDM\_up_... and %LOCALAPPDATA%\EDM
    if let Ok(local) = env::var("LOCALAPPDATA") {
        candidate_sources.push(
            PathBuf::from(&local)
                .join("EDM")
                .join("_up_")
                .join("_up_")
                .join("_up_")
                .join("target")
                .join("release")
                .join("WebView2Loader.dll"),
        );
        candidate_sources.push(
            PathBuf::from(&local)
                .join("EDM")
                .join("resources")
                .join("WebView2Loader.dll"),
        );
        candidate_sources.push(PathBuf::from(&local).join("EDM").join("WebView2Loader.dll"));
    }

    // 3. Check current helper executable directory (canonical when running from target/release or install dir)
    if let Ok(cur_exe) = env::current_exe() {
        if let Some(cur_parent) = cur_exe.parent() {
            candidate_sources.push(cur_parent.join("WebView2Loader.dll"));
            candidate_sources.push(cur_parent.join(r"resources\WebView2Loader.dll"));
            candidate_sources
                .push(cur_parent.join(r"_up_\_up_\_up_\target\release\WebView2Loader.dll"));
        }
    }

    // 4. Check HKCU\Software\EDM InstallDir
    #[cfg(windows)]
    {
        use windows_registry::CURRENT_USER;
        if let Ok(key) = CURRENT_USER.open(r"Software\EDM") {
            if let Ok(idir) = key.get_string("InstallDir") {
                candidate_sources.push(PathBuf::from(&idir).join("WebView2Loader.dll"));
                candidate_sources.push(PathBuf::from(&idir).join(r"resources\WebView2Loader.dll"));
                candidate_sources.push(
                    PathBuf::from(&idir).join(r"_up_\_up_\_up_\target\release\WebView2Loader.dll"),
                );
            }
        }
    }

    for src in candidate_sources {
        if src != target_dll
            && src.exists()
            && src.metadata().map(|m| m.len()).unwrap_or(0) > 1000
            && fs::copy(&src, &target_dll).is_ok()
        {
            tracing::info!("Co-located WebView2Loader.dll to {}", target_dll.display());
            return;
        }
    }
}

pub fn find_edm_exe() -> Option<PathBuf> {
    let current_exe = env::current_exe().ok()?;
    let dir = current_exe.parent()?;

    let exe_names = ["edm-desktop-app.exe", "EDM.exe", "edm.exe"];

    // 1. Registry HKCU\Software\EDM (authoritative installation record)
    #[cfg(windows)]
    {
        use windows_registry::CURRENT_USER;
        if let Ok(key) = CURRENT_USER.open(r"Software\EDM") {
            // First check AppPath
            if let Ok(app_path) = key.get_string("AppPath") {
                let p = PathBuf::from(&app_path);
                if p.exists() && p.is_file() {
                    let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if exe_names
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(fname))
                    {
                        ensure_webview2_loader(&p);
                        return Some(p);
                    }
                }
            }
            // Next check InstallDir
            if let Ok(install_dir) = key.get_string("InstallDir") {
                let idir = PathBuf::from(install_dir);
                for name in &exe_names {
                    let p = idir.join(name);
                    if p.exists() && p.is_file() {
                        ensure_webview2_loader(&p);
                        return Some(p);
                    }
                }
            }
        }
    }

    // 2. Same directory as current helper executable (canonical when edm-nmh is co-located)
    for name in &exe_names {
        let p = dir.join(name);
        if p.exists() && p.is_file() {
            ensure_webview2_loader(&p);
            return Some(p);
        }
    }

    // 3. Parent directory of current helper executable
    if let Some(parent) = dir.parent() {
        for name in &exe_names {
            let p = parent.join(name);
            if p.exists() && p.is_file() {
                ensure_webview2_loader(&p);
                return Some(p);
            }
        }
    }

    // 4. Standard Tauri user install locations (%LOCALAPPDATA%\EDM, %LOCALAPPDATA%\Programs\EDM)
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        let base = PathBuf::from(&local_app_data);
        let check_dirs = [base.join("EDM"), base.join("Programs").join("EDM")];
        for cdir in &check_dirs {
            for name in &exe_names {
                let p = cdir.join(name);
                if p.exists() && p.is_file() {
                    ensure_webview2_loader(&p);
                    return Some(p);
                }
            }
        }
    }

    // 5. Per-machine install (%ProgramFiles%\EDM, %ProgramFiles(x86)%\EDM)
    if let Ok(prog_files) = env::var("ProgramFiles") {
        let base = PathBuf::from(prog_files).join("EDM");
        for name in &exe_names {
            let p = base.join(name);
            if p.exists() && p.is_file() {
                ensure_webview2_loader(&p);
                return Some(p);
            }
        }
    }
    if let Ok(prog_files_x86) = env::var("ProgramFiles(x86)") {
        let base = PathBuf::from(prog_files_x86).join("EDM");
        for name in &exe_names {
            let p = base.join(name);
            if p.exists() && p.is_file() {
                ensure_webview2_loader(&p);
                return Some(p);
            }
        }
    }

    None
}

pub fn launch_detached(exe: &PathBuf, show_window: bool) -> Option<u32> {
    ensure_webview2_loader(exe);
    let dir = exe.parent().unwrap_or(exe.as_path());
    let arg = if show_window {
        "--show-window"
    } else {
        "--background"
    };

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut cmd = std::process::Command::new(exe);
        cmd.arg(arg)
            .current_dir(dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        // For GUI applications (Tauri / WebView2), do NOT pass CREATE_NO_WINDOW as it breaks
        // Chromium/WebView2 thread and message dispatch initialization. The --background CLI
        // argument instructs EDM to hide its window automatically.
        let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        cmd.creation_flags(flags);

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                tracing::info!(
                    "Successfully spawned EDM desktop detached (PID: {}) at {}",
                    pid,
                    exe.display()
                );
                return Some(pid);
            }
            Err(e) => {
                tracing::warn!(
                    "Direct detached spawn of {} failed: {}. Falling back to cmd start...",
                    exe.display(),
                    e
                );
            }
        }

        // Fallback: spawn via cmd /c start "" "exe" arg
        let mut fallback = std::process::Command::new("cmd.exe");
        fallback
            .args(["/c", "start", "", &exe.to_string_lossy(), arg])
            .current_dir(dir)
            .creation_flags(CREATE_NO_WINDOW);
        match fallback.spawn() {
            Ok(_) => tracing::info!(
                "Spawned EDM desktop via cmd start fallback at {}",
                exe.display()
            ),
            Err(e) => tracing::error!("Cmd start fallback failed for {}: {}", exe.display(), e),
        }
        None
    }

    #[cfg(not(windows))]
    {
        let mut cmd = std::process::Command::new(exe);
        cmd.arg(arg)
            .current_dir(dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                tracing::info!("Spawned EDM desktop (PID: {})", pid);
                Some(pid)
            }
            Err(e) => {
                tracing::error!("Failed to spawn EDM desktop: {}", e);
                None
            }
        }
    }
}

pub fn terminate_stale_edm_instance(pid: Option<u32>) {
    #[cfg(windows)]
    if let Some(target_pid) = pid {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let pid_str = target_pid.to_string();
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid_str, "/T"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    #[cfg(not(windows))]
    let _ = pid;
}

pub fn is_ws_server_ready() -> bool {
    use std::net::{SocketAddr, TcpStream};
    if let Ok(addr) = config::WS_ADDR.parse::<SocketAddr>() {
        if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            // Send quick probe header so ws_server peek resolves cleanly without hanging
            if stream
                .write_all(
                    b"GET /token HTTP/1.1\r\nHost: 127.0.0.1:14370\r\nConnection: close\r\n\r\n",
                )
                .is_ok()
            {
                let mut buf = [0u8; 32];
                let _ = stream.read(&mut buf);
            }
            return true;
        }
    }
    false
}

fn get_last_wake_stats() -> (Option<String>, Option<String>, Option<u64>) {
    let p = match env::var("LOCALAPPDATA") {
        Ok(dir) => PathBuf::from(dir).join("EDM").join("last_wake.json"),
        Err(_) => env::temp_dir().join("EDM").join("last_wake.json"),
    };
    if let Ok(content) = fs::read_to_string(&p) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            let err = v
                .get("error")
                .and_then(|e| e.as_str())
                .map(|s| s.to_string());
            let ts = v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());
            let ms = v.get("time_to_ready_ms").and_then(|m| m.as_u64());
            return (err, ts, ms);
        }
    }
    (None, None, None)
}

fn record_wake_result(
    ok: bool,
    error: Option<&str>,
    time_to_ready_ms: Option<u64>,
    exe_path: Option<&PathBuf>,
) {
    let p = match env::var("LOCALAPPDATA") {
        Ok(dir) => PathBuf::from(dir).join("EDM"),
        Err(_) => env::temp_dir().join("EDM"),
    };
    let _ = fs::create_dir_all(&p);
    let target = p.join("last_wake.json");
    let payload = serde_json::json!({
        "timestamp": Utc::now().to_rfc3339(),
        "ok": ok,
        "error": error,
        "time_to_ready_ms": time_to_ready_ms,
        "exe_path": exe_path.map(|p| p.to_string_lossy().to_string())
    });
    if let Ok(json_str) = serde_json::to_string_pretty(&payload) {
        let _ = fs::write(&target, json_str);
    }
}

pub fn wake_desktop(show_window: bool) -> AppReply {
    let start = Instant::now();
    tracing::info!("Wake request received (show_window: {})", show_window);

    #[cfg(windows)]
    let _lock = win_lock::NamedLock::acquire(r"Local\EDM_Wake_Lock", 25000);

    if is_ws_server_ready() {
        tracing::info!(
            "WebSocket server is already ready (elapsed: {:?})",
            start.elapsed()
        );
        if show_window {
            if let Some(exe) = find_edm_exe() {
                launch_detached(&exe, true);
            }
        }
        let token = config::get_ws_token();
        record_wake_result(
            true,
            None,
            Some(start.elapsed().as_millis() as u64),
            find_edm_exe().as_ref(),
        );
        return AppReply::WakeResult {
            ok: true,
            status: "ready".into(),
            token,
            error: None,
        };
    }

    let exe = match find_edm_exe() {
        Some(p) => {
            tracing::info!(
                "Found EDM executable at {} (elapsed: {:?})",
                p.display(),
                start.elapsed()
            );
            p
        }
        None => {
            tracing::error!("EDM executable not found in any candidate or registry location!");
            record_wake_result(false, Some("EDM executable not found"), None, None);
            return AppReply::WakeResult {
                ok: false,
                status: "error".into(),
                token: None,
                error: Some("EDM executable not found".into()),
            };
        }
    };

    let launch_start = Instant::now();
    let mut child_pid = launch_detached(&exe, show_window);
    tracing::info!(
        "Launched detached process for {} in {:?}",
        exe.display(),
        launch_start.elapsed()
    );

    // Cold start allowance up to 20 seconds with zombie self-healing recovery
    let mut ready = false;
    let wake_start = Instant::now();
    let mut healed = false;
    while wake_start.elapsed() < Duration::from_secs(20) {
        if is_ws_server_ready() {
            ready = true;
            break;
        }
        // If port 14370 is not ready after 4s, an unresponsive zombie EDM.exe may be blocking single-instance
        if !healed && wake_start.elapsed() >= Duration::from_secs(4) {
            healed = true;
            tracing::warn!(
                "Port 14370 not ready after 4s; terminating stale EDM instance (PID {:?}) and re-launching...",
                child_pid
            );
            terminate_stale_edm_instance(child_pid);
            std::thread::sleep(Duration::from_millis(400));
            child_pid = launch_detached(&exe, show_window);
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    if ready {
        let ready_ms = start.elapsed().as_millis() as u64;
        tracing::info!("EDM WebSocket port 14370 ready in {} ms", ready_ms);
        record_wake_result(true, None, Some(ready_ms), Some(&exe));
        let token = config::get_ws_token();
        AppReply::WakeResult {
            ok: true,
            status: "ready".into(),
            token,
            error: None,
        }
    } else {
        tracing::error!("Timeout (20s) waiting for EDM WebSocket server to initialize");
        record_wake_result(
            false,
            Some("Timeout waiting for EDM desktop to initialize"),
            None,
            Some(&exe),
        );
        AppReply::WakeResult {
            ok: false,
            status: "error".into(),
            token: None,
            error: Some("Timeout waiting for EDM desktop to initialize".into()),
        }
    }
}

#[cfg(windows)]
pub fn repair_registration() -> Result<String, String> {
    use windows_registry::CURRENT_USER;

    let app_dir = match env::var("LOCALAPPDATA") {
        Ok(dir) => PathBuf::from(dir).join("EDM"),
        Err(_) => env::temp_dir().join("EDM"),
    };
    let _ = fs::create_dir_all(&app_dir);

    let current_exe = env::current_exe().map_err(|e| e.to_string())?;
    let current_dir = current_exe.parent().unwrap_or(&app_dir);

    let nmh_path = if current_exe.file_name().and_then(|s| s.to_str()) == Some("edm-nmh.exe") {
        current_exe.clone()
    } else {
        let candidates = [
            current_dir.join("edm-nmh.exe"),
            current_dir.join("resources").join("edm-nmh.exe"),
            app_dir.join("edm-nmh.exe"),
        ];
        candidates
            .iter()
            .find(|p| p.exists() && p.is_file())
            .cloned()
            .unwrap_or_else(|| current_dir.join("edm-nmh.exe"))
    };

    let manifest_path = app_dir.join("com.edm.nmh.json");
    let manifest_json = serde_json::json!({
        "name": "com.edm.nmh",
        "description": "EDM Native Messaging Host for Edge, Chrome and Brave",
        "path": nmh_path.to_string_lossy(),
        "type": "stdio",
        "allowed_origins": edm_ipc::extensions::allowed_chromium_origins()
    });
    let content = serde_json::to_string_pretty(&manifest_json).map_err(|e| e.to_string())?;
    fs::write(&manifest_path, content).map_err(|e| e.to_string())?;

    let ff_manifest_path = app_dir.join("com.edm.nmh.firefox.json");
    let ff_manifest_json = serde_json::json!({
        "name": "com.edm.nmh",
        "description": "EDM Native Messaging Host for Mozilla Firefox",
        "path": nmh_path.to_string_lossy(),
        "type": "stdio",
        "allowed_extensions": edm_ipc::extensions::allowed_firefox_ids()
    });
    let ff_content = serde_json::to_string_pretty(&ff_manifest_json).map_err(|e| e.to_string())?;
    fs::write(&ff_manifest_path, ff_content).map_err(|e| e.to_string())?;

    let chromium_keys = [
        r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh",
        r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh",
        r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh",
    ];

    let manifest_str = manifest_path.to_string_lossy().to_string();
    for subkey in &chromium_keys {
        CURRENT_USER
            .create(subkey)
            .map_err(|e| format!("Failed to create {}: {}", subkey, e))?
            .set_string("", &manifest_str)
            .map_err(|e| format!("Failed to write {}: {}", subkey, e))?;
    }

    let ff_manifest_str = ff_manifest_path.to_string_lossy().to_string();
    let ff_subkey = r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh";
    CURRENT_USER
        .create(ff_subkey)
        .map_err(|e| format!("Failed to create {}: {}", ff_subkey, e))?
        .set_string("", &ff_manifest_str)
        .map_err(|e| format!("Failed to write {}: {}", ff_subkey, e))?;

    if let Some(app_exe) = find_edm_exe() {
        if let Ok(edm_key) = CURRENT_USER.create(r"Software\EDM") {
            let _ = edm_key.set_string("AppPath", app_exe.to_string_lossy());
            if let Some(parent) = app_exe.parent() {
                let _ = edm_key.set_string("InstallDir", parent.to_string_lossy());
            }
            let _ = edm_key.set_string("NmhPath", nmh_path.to_string_lossy());
            let _ = edm_key.set_string("Version", env!("CARGO_PKG_VERSION"));
        }
    }

    tracing::info!("Repair registration completed successfully");
    Ok("Native Messaging registration repaired successfully".to_string())
}

#[cfg(not(windows))]
pub fn repair_registration() -> Result<String, String> {
    Ok("Not applicable on non-Windows".to_string())
}

#[cfg(windows)]
pub fn collect_diagnostics(ext_id: Option<String>) -> DiagnosticsReport {
    use windows_registry::CURRENT_USER;

    let target_ext_id =
        ext_id.unwrap_or_else(|| edm_ipc::extensions::CHROMIUM_UNPACKED_ID.to_string());

    let browsers_meta = [
        (
            "edge",
            r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh",
            false,
        ),
        (
            "chrome",
            r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh",
            false,
        ),
        (
            "brave",
            r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh",
            false,
        ),
        (
            "firefox",
            r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh",
            true,
        ),
    ];

    let mut browser_diagnostics = Vec::new();

    for (name, subkey, is_firefox) in &browsers_meta {
        let mut diag = BrowserDiagnostic {
            browser: name.to_string(),
            registry_key: format!(r"HKCU\{}", subkey),
            helper_registered: false,
            manifest_path: None,
            manifest_exists: false,
            helper_path: None,
            helper_exists: false,
            origin_matched: false,
            error: None,
            connected_extension_id: None,
        };

        match CURRENT_USER.open(subkey) {
            Ok(key) => {
                diag.helper_registered = true;
                match key.get_string("") {
                    Ok(val) => {
                        diag.manifest_path = Some(val.clone());
                        let mpath = PathBuf::from(&val);
                        if mpath.exists() && mpath.is_file() {
                            diag.manifest_exists = true;
                            if let Ok(content) = fs::read_to_string(&mpath) {
                                if let Ok(json) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                                {
                                    if let Some(hpath_str) =
                                        json.get("path").and_then(|v| v.as_str())
                                    {
                                        let hpath = PathBuf::from(hpath_str);
                                        let full_hpath = if hpath.is_relative() {
                                            mpath.parent().unwrap_or(&mpath).join(hpath)
                                        } else {
                                            hpath
                                        };
                                        diag.helper_path =
                                            Some(full_hpath.to_string_lossy().to_string());
                                        diag.helper_exists =
                                            full_hpath.exists() && full_hpath.is_file();
                                    }

                                    if *is_firefox {
                                        if let Some(exts) = json
                                            .get("allowed_extensions")
                                            .and_then(|v| v.as_array())
                                        {
                                            diag.origin_matched = exts.iter().any(|e| {
                                                e.as_str()
                                                    .map(|s| {
                                                        edm_ipc::extensions::FIREFOX_ALLOWED_IDS
                                                            .contains(&s)
                                                            || s == target_ext_id
                                                    })
                                                    .unwrap_or(false)
                                            });
                                        }
                                    } else if let Some(origins) =
                                        json.get("allowed_origins").and_then(|v| v.as_array())
                                    {
                                        diag.origin_matched = origins.iter().any(|o| {
                                            if let Some(s) = o.as_str() {
                                                edm_ipc::extensions::is_allowed_origin(s)
                                                    || s.contains(&target_ext_id)
                                            } else {
                                                false
                                            }
                                        });
                                    }
                                } else {
                                    diag.error = Some("Failed to parse manifest JSON".to_string());
                                }
                            } else {
                                diag.error = Some("Failed to read manifest file".to_string());
                            }
                        } else {
                            diag.error = Some(format!("Manifest file does not exist at {}", val));
                        }
                    }
                    Err(e) => {
                        diag.error = Some(format!("Registry (Default) value error: {}", e));
                    }
                }
            }
            Err(_) => {
                diag.error = Some("Registry key not registered".to_string());
            }
        }
        browser_diagnostics.push(diag);
    }

    let helper_path = env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string());
    let app_exe = find_edm_exe();
    let app_installed = app_exe.is_some();
    let app_exe_path = app_exe.map(|p| p.to_string_lossy().to_string());
    let ws_port_ready = is_ws_server_ready();

    let (last_err, last_ts, ready_ms) = get_last_wake_stats();

    let connected_ext = if ws_port_ready {
        Some(target_ext_id.clone())
    } else {
        None
    };

    DiagnosticsReport {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: "Windows".to_string(),
        helper_path,
        app_exe_path,
        app_installed,
        ws_port_ready,
        time_to_ready_ms: ready_ms,
        last_wake_error: last_err,
        last_wake_timestamp: last_ts,
        browsers: browser_diagnostics,
        connected_extension_id: connected_ext,
    }
}

#[cfg(not(windows))]
pub fn collect_diagnostics(_ext_id: Option<String>) -> DiagnosticsReport {
    DiagnosticsReport {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: "Non-Windows".to_string(),
        helper_path: None,
        app_exe_path: None,
        app_installed: false,
        ws_port_ready: false,
        time_to_ready_ms: None,
        last_wake_error: None,
        last_wake_timestamp: None,
        browsers: vec![],
        connected_extension_id: None,
    }
}

pub fn read_message<R: Read>(
    mut reader: R,
) -> io::Result<Option<Result<ExtensionMessage, String>>> {
    let mut len_bytes = [0u8; 4];
    match reader.read_exact(&mut len_bytes) {
        Ok(_) => (),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = u32::from_ne_bytes(len_bytes) as usize;
    if len > config::MAX_NMH_MESSAGE_BYTES {
        return Ok(Some(Err("Message too large (>10MB)".to_string())));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;

    match serde_json::from_slice::<ExtensionMessage>(&buf) {
        Ok(msg) => Ok(Some(Ok(msg))),
        Err(e) => Ok(Some(Err(format!("JSON deserialization failed: {}", e)))),
    }
}

pub fn write_reply<W: Write>(mut writer: W, reply: &AppReply) -> io::Result<()> {
    let json_bytes =
        serde_json::to_vec(reply).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let len = (json_bytes.len() as u32).to_ne_bytes();
    writer.write_all(&len)?;
    writer.write_all(&json_bytes)?;
    writer.flush()?;
    Ok(())
}

pub async fn run_stdio_loop(store: &Arc<Mutex<Store>>, host_start: Instant) {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    tracing::info!(
        "NMH ready for length-prefixed stdin/stdout messages (init elapsed: {:?})",
        host_start.elapsed()
    );

    while let Ok(Some(res)) = read_message(&mut reader) {
        match res {
            Ok(msg) => {
                let msg_start = Instant::now();
                let reply = handle_message(msg, store).await;
                if let Err(e) = write_reply(&mut writer, &reply) {
                    tracing::error!("Failed to write reply: {:?}", e);
                    break;
                }
                tracing::info!("Processed and replied in {:?}", msg_start.elapsed());
            }
            Err(parse_err) => {
                tracing::warn!("Malformed message received: {}", parse_err);
                let reply = AppReply::Error {
                    message: format!("Malformed message: {}", parse_err),
                };
                if let Err(e) = write_reply(&mut writer, &reply) {
                    tracing::error!("Failed to write error reply: {:?}", e);
                    break;
                }
            }
        }
    }

    tracing::info!("NMH stdin closed (EOF). Exiting cleanly.");
}
