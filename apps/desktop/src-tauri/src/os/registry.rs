use std::path::PathBuf;

#[cfg(windows)]
pub fn register_native_messaging_host() {
    use windows_registry::CURRENT_USER;

    let app_dir = match std::env::var("LOCALAPPDATA") {
        Ok(dir) => PathBuf::from(dir).join("EDM"),
        Err(_) => std::env::temp_dir().join("EDM"),
    };
    let _ = std::fs::create_dir_all(&app_dir);

    let current_exe = std::env::current_exe().unwrap_or_default();
    let current_dir = current_exe.parent().unwrap_or(&app_dir);

    let mut candidates = vec![
        current_dir.join("edm-nmh.exe"),
        current_dir.join("resources").join("edm-nmh.exe"),
        current_dir
            .join("_up_")
            .join("_up_")
            .join("_up_")
            .join("target")
            .join("release")
            .join("edm-nmh.exe"),
        app_dir.join("resources").join("edm-nmh.exe"),
        app_dir
            .join("_up_")
            .join("_up_")
            .join("_up_")
            .join("target")
            .join("release")
            .join("edm-nmh.exe"),
    ];

    if let Ok(key) = CURRENT_USER.open(r"Software\EDM") {
        if let Ok(np) = key.get_string("NmhPath") {
            candidates.push(PathBuf::from(np));
        }
        if let Ok(idir) = key.get_string("InstallDir") {
            let idir_p = PathBuf::from(idir);
            candidates.push(idir_p.join("edm-nmh.exe"));
            candidates.push(idir_p.join("resources").join("edm-nmh.exe"));
            candidates.push(
                idir_p
                    .join("_up_")
                    .join("_up_")
                    .join("_up_")
                    .join("target")
                    .join("release")
                    .join("edm-nmh.exe"),
            );
        }
    }

    let mut walk_dir = current_dir.to_path_buf();
    for _ in 0..6 {
        candidates.push(walk_dir.join("edm-nmh.exe"));
        candidates.push(walk_dir.join("target").join("release").join("edm-nmh.exe"));
        candidates.push(walk_dir.join("target").join("debug").join("edm-nmh.exe"));
        candidates.push(walk_dir.join("resources").join("edm-nmh.exe"));
        if let Some(p) = walk_dir.parent() {
            walk_dir = p.to_path_buf();
        } else {
            break;
        }
    }

    let target_nmh = app_dir.join("edm-nmh.exe");

    let found_source = candidates
        .iter()
        .filter(|p| *p != &target_nmh && p.exists() && p.is_file())
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());

    if let Some(src) = found_source {
        let should_copy = if !target_nmh.exists() {
            true
        } else {
            let src_meta = std::fs::metadata(src).ok();
            let tgt_meta = std::fs::metadata(&target_nmh).ok();
            match (src_meta, tgt_meta) {
                (Some(sm), Some(tm)) => {
                    sm.len() != tm.len()
                        || sm.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                            > tm.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                }
                _ => true,
            }
        };

        if should_copy {
            match std::fs::copy(src, &target_nmh) {
                Ok(_) => tracing::info!(
                    "Updated NMH binary at {} from {}",
                    target_nmh.display(),
                    src.display()
                ),
                Err(e) => tracing::warn!(
                    "Failed to copy NMH binary to {}: {}",
                    target_nmh.display(),
                    e
                ),
            }
        }
    } else if !target_nmh.exists() {
        tracing::warn!(
            "edm-nmh.exe not found in candidate paths near {}",
            current_dir.display()
        );
    }

    let nmh_path = if target_nmh.exists() {
        target_nmh
    } else {
        found_source
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

    if let Ok(content) = serde_json::to_string_pretty(&manifest_json) {
        let _ = std::fs::write(&manifest_path, &content);
        if current_dir != app_dir {
            let _ = std::fs::write(current_dir.join("com.edm.nmh.json"), &content);
        }
    }

    let ff_manifest_path = app_dir.join("com.edm.nmh.firefox.json");
    let ff_manifest_json = serde_json::json!({
        "name": "com.edm.nmh",
        "description": "EDM Native Messaging Host for Mozilla Firefox",
        "path": nmh_path.to_string_lossy(),
        "type": "stdio",
        "allowed_extensions": edm_ipc::extensions::allowed_firefox_ids()
    });
    if let Ok(content) = serde_json::to_string_pretty(&ff_manifest_json) {
        let _ = std::fs::write(&ff_manifest_path, &content);
        if current_dir != app_dir {
            let _ = std::fs::write(current_dir.join("com.edm.nmh.firefox.json"), &content);
        }
    }

    let chromium_reg_targets = [
        r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh",
        r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh",
        r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh",
    ];

    let manifest_str = manifest_path.to_string_lossy().to_string();
    for key in &chromium_reg_targets {
        if let Ok(k) = CURRENT_USER.create(key) {
            let _ = k.set_string("", &manifest_str);
        }
    }

    let ff_manifest_str = ff_manifest_path.to_string_lossy().to_string();
    if let Ok(k) = CURRENT_USER.create(r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh") {
        let _ = k.set_string("", &ff_manifest_str);
    }

    if let Ok(k) = CURRENT_USER.create(r"Software\EDM") {
        let _ = k.set_string("AppPath", current_exe.to_string_lossy());
        let _ = k.set_string("InstallDir", current_dir.to_string_lossy());
        let _ = k.set_string("NmhPath", nmh_path.to_string_lossy());
    }
}

#[cfg(not(windows))]
pub fn register_native_messaging_host() {}

#[cfg(windows)]
pub fn set_app_user_model_id() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    extern "system" {
        fn SetCurrentProcessExplicitAppUserModelID(AppID: *const u16) -> i32;
    }

    let wide: Vec<u16> = OsStr::new("com.edm.desktop")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(wide.as_ptr());
    }
}

#[cfg(not(windows))]
pub fn set_app_user_model_id() {}

#[cfg(windows)]
pub fn purge_all_user_data() {
    use windows_registry::CURRENT_USER;
    let _ = CURRENT_USER.remove_tree(r"Software\EDM");
    let _ = CURRENT_USER.remove_tree(r"Software\Google\Chrome\NativeMessagingHosts\com.edm.nmh");
    let _ = CURRENT_USER.remove_tree(r"Software\Microsoft\Edge\NativeMessagingHosts\com.edm.nmh");
    let _ = CURRENT_USER
        .remove_tree(r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.edm.nmh");
    let _ = CURRENT_USER.remove_tree(r"Software\Mozilla\NativeMessagingHosts\com.edm.nmh");

    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let _ = std::fs::remove_dir_all(PathBuf::from(&local).join("EDM"));
        let _ = std::fs::remove_dir_all(PathBuf::from(&local).join("com.edm.desktop"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let _ = std::fs::remove_dir_all(PathBuf::from(&appdata).join("EDM"));
    }
}
