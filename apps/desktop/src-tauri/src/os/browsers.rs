use std::path::PathBuf;
use std::sync::Arc;

#[cfg(windows)]
pub fn find_browser_exe(
    app_paths_key: &str,
    common_subpaths: &[&str],
) -> Option<String> {
    use windows_registry::{CURRENT_USER, LOCAL_MACHINE};

    // 1. Try HKLM App Paths
    if let Ok(k) = LOCAL_MACHINE.open(app_paths_key) {
        if let Ok(val) = k.get_string("") {
            let p = PathBuf::from(val.trim_matches('"'));
            if p.exists() {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }

    // 2. Try HKCU App Paths
    if let Ok(k) = CURRENT_USER.open(app_paths_key) {
        if let Ok(val) = k.get_string("") {
            let p = PathBuf::from(val.trim_matches('"'));
            if p.exists() {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }

    // 3. Try standard Program Files & LocalAppData paths
    let prefixes = [
        std::env::var("ProgramFiles").ok().map(PathBuf::from),
        std::env::var("ProgramFiles(x86)").ok().map(PathBuf::from),
        std::env::var("LOCALAPPDATA").ok().map(PathBuf::from),
    ];

    for prefix in prefixes.iter().flatten() {
        for sub in common_subpaths {
            let candidate = prefix.join(sub);
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    None
}

#[cfg(not(windows))]
pub fn find_browser_exe(_app_paths_key: &str, _common_subpaths: &[&str]) -> Option<String> {
    None
}

pub async fn collect_browser_diagnostics(
    ext_id: Option<String>,
    ws_manager: Option<&Arc<crate::ws_server::WsManager>>,
) -> edm_ipc::DiagnosticsReport {
    #[cfg(windows)]
    {
        use windows_registry::CURRENT_USER;

        let target_ext_id = ext_id.unwrap_or_else(|| edm_ipc::extensions::CHROMIUM_UNPACKED_ID.to_string());

        let browsers_meta = [
            ("edge", r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh", false),
            ("chrome", r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh", false),
            ("brave", r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh", false),
            ("firefox", r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh", true),
        ];

        let mut browser_diagnostics = Vec::new();

        for (name, subkey, is_firefox) in &browsers_meta {
            let mut diag = edm_ipc::BrowserDiagnostic {
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

            if let Some(mgr) = ws_manager {
                if let Some(conn_id) = mgr.get_browser_connected_id(name).await {
                    diag.connected_extension_id = Some(conn_id);
                }
            }

            match CURRENT_USER.open(subkey) {
                Ok(key) => {
                    diag.helper_registered = true;
                    match key.get_string("") {
                        Ok(val) => {
                            diag.manifest_path = Some(val.clone());
                            let mpath = PathBuf::from(&val);
                            if mpath.exists() && mpath.is_file() {
                                diag.manifest_exists = true;
                                if let Ok(content) = std::fs::read_to_string(&mpath) {
                                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                        if let Some(hpath_str) = json.get("path").and_then(|v| v.as_str()) {
                                            let hpath = PathBuf::from(hpath_str);
                                            let full_hpath = if hpath.is_relative() {
                                                mpath.parent().unwrap_or(&mpath).join(hpath)
                                            } else {
                                                hpath
                                            };
                                            diag.helper_path = Some(full_hpath.to_string_lossy().to_string());
                                            diag.helper_exists = full_hpath.exists() && full_hpath.is_file();
                                        }

                                        if *is_firefox {
                                            if let Some(exts) = json.get("allowed_extensions").and_then(|v| v.as_array()) {
                                                diag.origin_matched = exts.iter().any(|e| {
                                                    e.as_str().map(|s| edm_ipc::extensions::FIREFOX_ALLOWED_IDS.contains(&s) || s == target_ext_id).unwrap_or(false)
                                                });
                                            }
                                        } else if let Some(origins) = json.get("allowed_origins").and_then(|v| v.as_array()) {
                                            diag.origin_matched = origins.iter().any(|o| {
                                                if let Some(s) = o.as_str() {
                                                    edm_ipc::extensions::is_allowed_origin(s) || s.contains(&target_ext_id)
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

        let current_exe = std::env::current_exe().ok();
        let dot = PathBuf::from(".");
        let app_dir = current_exe.as_ref().and_then(|p| p.parent()).unwrap_or(&dot);
        let helper_path = [
            app_dir.join("edm-nmh.exe"),
            app_dir.join("resources").join("edm-nmh.exe"),
        ].into_iter().find(|p| p.exists()).map(|p| p.to_string_lossy().to_string());

        let (last_err, last_ts, ready_ms) = {
            let p = match std::env::var("LOCALAPPDATA") {
                Ok(dir) => PathBuf::from(dir).join("EDM").join("last_wake.json"),
                Err(_) => std::env::temp_dir().join("EDM").join("last_wake.json"),
            };
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    let err = v.get("error").and_then(|e| e.as_str()).map(|s| s.to_string());
                    let ts = v.get("timestamp").and_then(|t| t.as_str()).map(|s| s.to_string());
                    let ms = v.get("time_to_ready_ms").and_then(|m| m.as_u64());
                    (err, ts, ms)
                } else {
                    (None, None, None)
                }
            } else {
                (None, None, None)
            }
        };

        let any_connected = if let Some(mgr) = ws_manager {
            mgr.get_any_connected_extension_id().await
        } else {
            None
        };

        edm_ipc::DiagnosticsReport {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: "Windows".to_string(),
            helper_path,
            app_exe_path: current_exe.map(|p| p.to_string_lossy().to_string()),
            app_installed: true,
            ws_port_ready: true,
            time_to_ready_ms: ready_ms,
            last_wake_error: last_err,
            last_wake_timestamp: last_ts,
            browsers: browser_diagnostics,
            connected_extension_id: any_connected,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = ext_id;
        let _ = ws_manager;
        edm_ipc::DiagnosticsReport {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: "Non-Windows".to_string(),
            helper_path: None,
            app_exe_path: None,
            app_installed: true,
            ws_port_ready: true,
            time_to_ready_ms: None,
            last_wake_error: None,
            last_wake_timestamp: None,
            browsers: Vec::new(),
            connected_extension_id: None,
        }
    }
}

pub async fn detect_installed_browsers_internal(
    state: &crate::state::AppState,
) -> Result<Vec<edm_ipc::BrowserSetupInfo>, String> {
    let mut results = Vec::new();

    #[cfg(windows)]
    {
        use windows_registry::CURRENT_USER;

        let (chrome_store_url, edge_store_url, brave_store_url, firefox_xpi_url) = {
            let mut c_url = Some(edm_ipc::extensions::DEFAULT_CHROME_STORE_URL.to_string());
            let mut e_url = Some(edm_ipc::extensions::DEFAULT_EDGE_STORE_URL.to_string());
            let mut b_url = Some(edm_ipc::extensions::DEFAULT_CHROME_STORE_URL.to_string());
            let mut f_url = Some(edm_ipc::extensions::DEFAULT_FIREFOX_XPI_URL.to_string());

            if let Ok(Some(json_str)) = state.store.get_setting("app_settings") {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    if let Some(urls) = val.get("extension_store_urls") {
                        if let Some(u) = urls.get("chrome").and_then(|v| v.as_str()) {
                            c_url = if u.trim().is_empty() { None } else { Some(u.trim().to_string()) };
                        }
                        if let Some(u) = urls.get("edge").and_then(|v| v.as_str()) {
                            e_url = if u.trim().is_empty() { None } else { Some(u.trim().to_string()) };
                        }
                        if let Some(u) = urls.get("brave").and_then(|v| v.as_str()) {
                            b_url = if u.trim().is_empty() { None } else { Some(u.trim().to_string()) };
                        }
                        if let Some(u) = urls.get("firefox").and_then(|v| v.as_str()) {
                            f_url = if u.trim().is_empty() { None } else { Some(u.trim().to_string()) };
                        }
                    }
                }
            }
            (c_url, e_url, b_url, f_url)
        };

        // 1. Chrome
        let chrome_path = find_browser_exe(
            r"Software\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe",
            &[r"Google\Chrome\Application\chrome.exe"],
        );
        let chrome_helper = CURRENT_USER
            .open(r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh")
            .is_ok();
        let chrome_connected = state.ws_manager.is_browser_connected("chrome").await;

        if chrome_path.is_some() || chrome_helper || chrome_connected {
            results.push(edm_ipc::BrowserSetupInfo {
                id: "chrome".to_string(),
                name: "Google Chrome".to_string(),
                installed: chrome_path.is_some(),
                exe_path: chrome_path,
                helper_registered: chrome_helper,
                extension_connected: chrome_connected,
                store_url: chrome_store_url,
                extension_page_url: "chrome://extensions".to_string(),
            });
        }

        // 2. Edge
        let edge_path = find_browser_exe(
            r"Software\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
            &[r"Microsoft\Edge\Application\msedge.exe"],
        );
        let edge_helper = CURRENT_USER
            .open(r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh")
            .is_ok();
        let edge_connected = state.ws_manager.is_browser_connected("edge").await;

        if edge_path.is_some() || edge_helper || edge_connected {
            results.push(edm_ipc::BrowserSetupInfo {
                id: "edge".to_string(),
                name: "Microsoft Edge".to_string(),
                installed: edge_path.is_some(),
                exe_path: edge_path,
                helper_registered: edge_helper,
                extension_connected: edge_connected,
                store_url: edge_store_url,
                extension_page_url: "edge://extensions".to_string(),
            });
        }

        // 3. Brave
        let brave_path = find_browser_exe(
            r"Software\Microsoft\Windows\CurrentVersion\App Paths\brave.exe",
            &[r"BraveSoftware\Brave-Browser\Application\brave.exe"],
        );
        let brave_helper = CURRENT_USER
            .open(r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh")
            .is_ok();
        let brave_connected = state.ws_manager.is_browser_connected("brave").await;

        if brave_path.is_some() || brave_helper || brave_connected {
            results.push(edm_ipc::BrowserSetupInfo {
                id: "brave".to_string(),
                name: "Brave Browser".to_string(),
                installed: brave_path.is_some(),
                exe_path: brave_path,
                helper_registered: brave_helper,
                extension_connected: brave_connected,
                store_url: brave_store_url,
                extension_page_url: "brave://extensions".to_string(),
            });
        }

        // 4. Firefox
        let ff_path = find_browser_exe(
            r"Software\Microsoft\Windows\CurrentVersion\App Paths\firefox.exe",
            &[r"Mozilla Firefox\firefox.exe"],
        );
        let ff_helper = CURRENT_USER
            .open(r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh")
            .is_ok();
        let ff_connected = state.ws_manager.is_browser_connected("firefox").await;

        let xpi_exists = crate::os::extension_files::get_firefox_xpi_path()
            .map(|p| p.exists())
            .unwrap_or(false);

        if ff_path.is_some() && xpi_exists {
            results.push(edm_ipc::BrowserSetupInfo {
                id: "firefox".to_string(),
                name: "Mozilla Firefox".to_string(),
                installed: true,
                exe_path: ff_path,
                helper_registered: ff_helper,
                extension_connected: ff_connected,
                store_url: firefox_xpi_url,
                extension_page_url: "about:addons".to_string(),
            });
        }
    }

    Ok(results)
}

pub fn open_browser_url_internal(browser: String, url: String) -> Result<(), String> {
    // Validate scheme to prevent argument injection (--load-extension, --user-data-dir, etc.)
    let parsed = url::Url::parse(&url).map_err(|e| format!("Invalid URL: {}", e))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("Security error: only http and https URLs can be opened in browser".to_string());
    }

    #[cfg(windows)]
    {
        let browser_id = browser.to_lowercase();
        let exe_path = match browser_id.as_str() {
            "chrome" => find_browser_exe(r"Software\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe", &[r"Google\Chrome\Application\chrome.exe"]),
            "edge" => find_browser_exe(r"Software\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe", &[r"Microsoft\Edge\Application\msedge.exe"]),
            "brave" => find_browser_exe(r"Software\Microsoft\Windows\CurrentVersion\App Paths\brave.exe", &[r"BraveSoftware\Brave-Browser\Application\brave.exe"]),
            "firefox" => find_browser_exe(r"Software\Microsoft\Windows\CurrentVersion\App Paths\firefox.exe", &[r"Mozilla Firefox\firefox.exe"]),
            _ => None,
        };

        if let Some(exe) = exe_path {
            let mut cmd = std::process::Command::new(&exe);
            cmd.arg(&url);
            let _ = cmd.spawn().map_err(|e| format!("Failed to launch browser {}: {}", exe, e))?;
            return Ok(());
        }
    }

    open::that(&url).map_err(|e| e.to_string())?;
    Ok(())
}
