use std::path::PathBuf;

pub fn get_user_extension_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Ok(PathBuf::from(appdata).join("EDM").join("extension"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return Ok(PathBuf::from(local).join("EDM").join("extension"));
        }
        Err("Failed to determine %APPDATA% or %LOCALAPPDATA%".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return Ok(PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("EDM")
                .join("extension"));
        }
        Err("Failed to determine $HOME".to_string())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        if let Ok(config) = std::env::var("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(config).join("EDM").join("extension"));
        }
        if let Ok(home) = std::env::var("HOME") {
            return Ok(PathBuf::from(home).join(".config").join("EDM").join("extension"));
        }
        Err("Failed to determine config folder".to_string())
    }
}

pub const EMBEDDED_EXTENSION_FILES: &[(&str, &[u8])] = &[
    ("manifest.json", include_bytes!("../../../../../extension/manifest.json")),
    ("background.js", include_bytes!("../../../../../extension/background.js")),
    ("content.js", include_bytes!("../../../../../extension/content.js")),
    ("popup.html", include_bytes!("../../../../../extension/popup.html")),
    ("popup.js", include_bytes!("../../../../../extension/popup.js")),
    ("popup.css", include_bytes!("../../../../../extension/popup.css")),
    ("options.html", include_bytes!("../../../../../extension/options.html")),
    ("options.js", include_bytes!("../../../../../extension/options.js")),
    ("hashes.json", include_bytes!("../../../../../extension/hashes.json")),
    ("logo.png", include_bytes!("../../../../../extension/logo.png")),
];

pub fn prepare_extension_files_internal() -> Result<String, String> {
    use sha2::{Digest, Sha256};

    let target_dir = get_user_extension_dir()?;
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create target extension directory: {}", e))?;

    let mut source_candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            source_candidates.push(parent.join("extension"));
            source_candidates.push(parent.join("resources").join("extension"));
            source_candidates.push(parent.join("resources").join("chromium"));
        }
    }
    source_candidates.push(PathBuf::from("extension"));
    source_candidates.push(PathBuf::from("../../extension"));
    source_candidates.push(PathBuf::from("../../../extension"));
    source_candidates.push(PathBuf::from("extension/dist/chromium"));
    source_candidates.push(PathBuf::from("../../extension/dist/chromium"));
    source_candidates.push(PathBuf::from("../../../extension/dist/chromium"));

    let mut found_source = None;
    for candidate in &source_candidates {
        if candidate.join("hashes.json").exists() && candidate.join("manifest.json").exists() {
            found_source = Some(candidate.clone());
            break;
        }
    }

    if found_source.is_none() {
        for candidate in &source_candidates {
            if candidate.join("manifest.json").exists() {
                found_source = Some(candidate.clone());
                break;
            }
        }
    }

    let mut copied_from_disk = false;

    if let Some(ref source_dir) = found_source {
        let hashes_path = source_dir.join("hashes.json");
        if hashes_path.exists() {
            if let Ok(hashes_content) = std::fs::read_to_string(&hashes_path) {
                if let Ok(hashes_json) = serde_json::from_str::<serde_json::Value>(&hashes_content) {
                    if let Some(files_map) = hashes_json.get("files").and_then(|f| f.as_object()) {
                        let mut all_ok = true;
                        for (filename, expected_hash_val) in files_map {
                            let expected_hash_str = match expected_hash_val.as_str() {
                                Some(s) => s,
                                None => { all_ok = false; break; }
                            };
                            let expected_sha256 = expected_hash_str
                                .strip_prefix("sha256:")
                                .unwrap_or(expected_hash_str);

                            let src_file = source_dir.join(filename);
                            if !src_file.exists() {
                                all_ok = false;
                                break;
                            }

                            let file_bytes = match std::fs::read(&src_file) {
                                Ok(b) => b,
                                Err(_) => { all_ok = false; break; }
                            };

                            let mut hasher = Sha256::new();
                            hasher.update(&file_bytes);
                            let actual_sha256 = hex::encode(hasher.finalize());

                            if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
                                all_ok = false;
                                break;
                            }

                            let dest_file = target_dir.join(filename);
                            let _ = std::fs::write(&dest_file, &file_bytes);
                        }
                        if all_ok {
                            let _ = std::fs::copy(&hashes_path, target_dir.join("hashes.json"));
                            copied_from_disk = true;
                        }
                    }
                }
            }
        }

        if !copied_from_disk {
            if let Ok(entries) = std::fs::read_dir(source_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(name) = path.file_name() {
                            let dest = target_dir.join(name);
                            let _ = std::fs::copy(&path, &dest);
                        }
                    }
                }
                if target_dir.join("manifest.json").exists() {
                    copied_from_disk = true;
                }
            }
        }
    }

    if !copied_from_disk || !target_dir.join("manifest.json").exists() {
        for (filename, bytes) in EMBEDDED_EXTENSION_FILES {
            let dest = target_dir.join(filename);
            let _ = std::fs::write(&dest, bytes);
        }
    }

    if !target_dir.join("manifest.json").exists() {
        let manifest_bytes = include_bytes!("../../../../../extension/manifest.json");
        let _ = std::fs::write(target_dir.join("manifest.json"), manifest_bytes);
    }

    #[cfg(windows)]
    {
        let mut dirs_to_sync = Vec::new();
        if let Ok(appdata) = std::env::var("APPDATA") {
            dirs_to_sync.push(PathBuf::from(appdata).join("EDM").join("extension"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            dirs_to_sync.push(PathBuf::from(local).join("EDM").join("extension"));
        }

        for sync_target in dirs_to_sync {
            let _ = std::fs::create_dir_all(&sync_target);
            for (filename, bytes) in EMBEDDED_EXTENSION_FILES {
                let dest = sync_target.join(filename);
                let _ = std::fs::write(&dest, bytes);
            }
            if let Ok(entries) = std::fs::read_dir(&target_dir) {
                for entry in entries.flatten() {
                    let src = entry.path();
                    if src.is_file() {
                        if let Some(name) = src.file_name() {
                            let _ = std::fs::copy(&src, sync_target.join(name));
                        }
                    }
                }
            }
        }
    }

    tracing::info!(
        "Extension files successfully verified and provisioned to {}",
        target_dir.display()
    );

    Ok(target_dir.to_string_lossy().to_string())
}

pub fn get_extension_folder_path_internal() -> Result<String, String> {
    let target_dir = get_user_extension_dir()?;
    Ok(target_dir.to_string_lossy().to_string())
}

pub fn remove_extension_folder_internal() -> Result<bool, String> {
    let target_dir = get_user_extension_dir()?;
    let mut removed = false;
    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir)
            .map_err(|e| format!("Failed to remove extension directory: {}", e))?;
        removed = true;
    }
    #[cfg(windows)]
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let local_dir = PathBuf::from(local).join("EDM").join("extension");
        if local_dir.exists() {
            let _ = std::fs::remove_dir_all(&local_dir);
            removed = true;
        }
    }
    tracing::info!("Extension folder cleanup result: removed={}", removed);
    Ok(removed)
}

pub fn get_firefox_xpi_path() -> Option<PathBuf> {
    let local_candidates = [
        PathBuf::from("extension/dist/firefox/edm-firefox.xpi"),
        PathBuf::from("extension/dist/firefox/firefox.xpi"),
        PathBuf::from("resources/firefox.xpi"),
        PathBuf::from("resources/edm-firefox.xpi"),
    ];
    for p in &local_candidates {
        if p.exists() {
            return Some(p.clone());
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            let candidate1 = p.join("resources").join("firefox.xpi");
            if candidate1.exists() {
                return Some(candidate1);
            }
            let candidate2 = p.join("firefox.xpi");
            if candidate2.exists() {
                return Some(candidate2);
            }
        }
    }
    None
}
