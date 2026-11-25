use super::platform;
use super::types::{ExistingInstallationInfo, InstallOptions};
use std::path::{Path, PathBuf};
use tracing::info;

pub fn get_default_install_dir() -> String {
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return format!("{}\\EDM", local);
        }
        if let Ok(user) = std::env::var("USERPROFILE") {
            return format!("{}\\AppData\\Local\\EDM", user);
        }
    }
    std::env::temp_dir()
        .join("EDM")
        .to_string_lossy()
        .to_string()
}

pub fn check_existing_installation() -> ExistingInstallationInfo {
    #[cfg(windows)]
    {
        use windows_registry::CURRENT_USER;
        let mut info = ExistingInstallationInfo {
            is_installed: false,
            install_dir: None,
            version: None,
            publisher: None,
        };

        if let Ok(key) =
            CURRENT_USER.open(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\EDM")
        {
            let mut valid = false;
            if let Ok(loc) = key.get_string("InstallLocation") {
                let clean_loc = loc.trim_matches('"');
                let dir_path = PathBuf::from(clean_loc);
                let lower = clean_loc.to_lowercase();
                if !lower.contains(r"\temp\")
                    && !lower.contains("nsd")
                    && (dir_path.join("EDM.exe").exists()
                        || dir_path.join("edm-desktop-app.exe").exists())
                {
                    valid = true;
                    info.install_dir = Some(clean_loc.to_string());
                }
            }
            if valid {
                info.is_installed = true;
                if let Ok(v) = key.get_string("DisplayVersion") {
                    info.version = Some(v);
                }
                if let Ok(p) = key.get_string("Publisher") {
                    info.publisher = Some(p);
                }
            }
        }

        if !info.is_installed {
            if let Ok(key) = CURRENT_USER.open(r"Software\EDM") {
                if let Ok(dir) = key.get_string("InstallDir") {
                    let dir_path = PathBuf::from(&dir);
                    let lower = dir.to_lowercase();
                    if !lower.contains(r"\temp\")
                        && !lower.contains("nsd")
                        && !lower.contains("target")
                        && (dir_path.join("EDM.exe").exists()
                            || dir_path.join("edm-desktop-app.exe").exists())
                    {
                        info.is_installed = true;
                        info.install_dir = Some(dir);
                    }
                }
            }
        }

        if !info.is_installed {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let p = PathBuf::from(local).join("EDM").join("EDM.exe");
                if p.exists() {
                    info.is_installed = true;
                    info.install_dir = Some(p.parent().unwrap().to_string_lossy().to_string());
                }
            }
        }

        info
    }
    #[cfg(not(windows))]
    {
        ExistingInstallationInfo {
            is_installed: false,
            install_dir: None,
            version: None,
            publisher: None,
        }
    }
}

pub fn perform_native_installation(options: InstallOptions) -> Result<(), String> {
    #[cfg(windows)]
    {
        perform_windows_installation(options)
    }
    #[cfg(not(windows))]
    {
        let _ = options;
        Err("Native installation is only supported on Windows".into())
    }
}

#[cfg(windows)]
fn perform_windows_installation(options: InstallOptions) -> Result<(), String> {
    let target_path = PathBuf::from(&options.target_dir);

    // 1. Create target directory
    std::fs::create_dir_all(&target_path)
        .map_err(|e| format!("Failed to create target installation directory: {}", e))?;

    // 2. Terminate background EDM/NMH processes via Win32 Toolhelp snapshot
    let _ = platform::terminate_sibling_instances();

    // 3. Copy running executable and sibling binaries to target_dir
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Cannot resolve current exe: {}", e))?;
    let current_dir = current_exe
        .parent()
        .ok_or("Cannot resolve current exe directory")?;

    if let Ok(entries) = std::fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let src = entry.path();
            if src.is_file() {
                let dst = target_path.join(entry.file_name());
                let _ = std::fs::copy(&src, &dst);
            }
        }
    }

    // Helper to find file in candidate trees
    fn find_file_recursive(dir: &Path, target_name: &str) -> Option<PathBuf> {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_file() && p.file_name().and_then(|n| n.to_str()) == Some(target_name) {
                    return Some(p);
                } else if p.is_dir() {
                    if let Some(found) = find_file_recursive(&p, target_name) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    // Ensure edm-nmh.exe is found and copied to target_path\edm-nmh.exe
    let target_nmh = target_path.join("edm-nmh.exe");
    if !target_nmh.exists() {
        let candidate_paths = [
            current_dir.join("edm-nmh.exe"),
            current_dir.join("resources").join("edm-nmh.exe"),
            current_dir
                .join("resources")
                .join("target")
                .join("release")
                .join("edm-nmh.exe"),
            current_dir
                .join("_up_")
                .join("_up_")
                .join("_up_")
                .join("target")
                .join("release")
                .join("edm-nmh.exe"),
        ];
        let mut copied_nmh = false;
        for c in &candidate_paths {
            if c.exists() {
                if std::fs::copy(c, &target_nmh).is_ok() {
                    copied_nmh = true;
                    break;
                }
            }
        }
        if !copied_nmh {
            if let Some(found_nmh) = find_file_recursive(current_dir, "edm-nmh.exe") {
                let _ = std::fs::copy(&found_nmh, &target_nmh);
            }
        }
    }

    // Ensure WebView2Loader.dll is self-healed in target_path
    crate::os::assets::self_heal_dll_in_dir(&target_path);

    let edm_exe = target_path.join("EDM.exe");
    let edm_desktop_exe = target_path.join("edm-desktop-app.exe");

    // Ensure bidirectional co-location between EDM.exe and edm-desktop-app.exe
    if edm_desktop_exe.exists() && !edm_exe.exists() {
        let _ = std::fs::copy(&edm_desktop_exe, &edm_exe);
    } else if edm_exe.exists() && !edm_desktop_exe.exists() {
        let _ = std::fs::copy(&edm_exe, &edm_desktop_exe);
    } else if !edm_exe.exists() && !edm_desktop_exe.exists() {
        let _ = std::fs::copy(&current_exe, &edm_exe);
        let _ = std::fs::copy(&current_exe, &edm_desktop_exe);
    }

    // 4. Create Start Menu shortcut using Pure Win32 COM
    if let Ok(appdata) = std::env::var("APPDATA") {
        let start_menu_dir = PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs");
        let start_menu_lnk = start_menu_dir.join("EDM.lnk");
        let _ =
            platform::create_com_shortcut(&start_menu_lnk, &edm_exe, &target_path, None, 0, None);
        info!(
            "Created Start Menu COM shortcut at {}",
            start_menu_lnk.display()
        );
    }

    // 5. Desktop shortcut using Pure Win32 COM
    if options.create_desktop_shortcut {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let desktop_lnk = PathBuf::from(userprofile).join("Desktop").join("EDM.lnk");
            let _ =
                platform::create_com_shortcut(&desktop_lnk, &edm_exe, &target_path, None, 0, None);
            info!("Created Desktop COM shortcut at {}", desktop_lnk.display());
        }
    }

    // 6. Start-on-boot registry entry
    if options.start_on_boot {
        let _ = platform::register_autostart(&edm_exe, true);
    }

    // 7. Browser integration (NMH manifests + registry)
    if options.enable_browser_integration {
        let _ = platform::register_browser_nmh(&target_path, &target_nmh);
    }

    // 8. Protocol handlers & file associations
    let _ = platform::register_protocol_handlers(&edm_exe);

    // 9. Uninstall registry key with full metadata
    let _ = platform::register_uninstall_entry(&target_path, &edm_exe);

    info!(
        "Native installation completed successfully in {}",
        target_path.display()
    );

    // 10. Spawn installed EDM and exit installer
    let _ = std::process::Command::new(&edm_exe)
        .args(["--show-window"])
        .spawn();
    std::process::exit(0);
}
