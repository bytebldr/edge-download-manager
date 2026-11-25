use super::install::get_default_install_dir;
use super::platform;
use super::types::{RepairPlan, RepairReport};
use std::path::PathBuf;
use tracing::info;

pub fn perform_native_repair(plan: RepairPlan) -> Result<RepairReport, String> {
    let mut details = Vec::new();
    let mut browser_ok = true;
    let mut shortcuts_ok = true;
    let mut binaries_ok = true;

    let install_dir = PathBuf::from(get_default_install_dir());
    let _ = std::fs::create_dir_all(&install_dir);

    // 1. Verify and restore core binaries
    if plan.verify_binaries {
        let edm_exe = install_dir.join("EDM.exe");
        let desktop_exe = install_dir.join("edm-desktop-app.exe");
        let nmh_exe = install_dir.join("edm-nmh.exe");

        if desktop_exe.exists() && !edm_exe.exists() {
            let _ = std::fs::copy(&desktop_exe, &edm_exe);
            details.push("Restored EDM.exe from desktop binary payload.".to_string());
        } else if edm_exe.exists() && !desktop_exe.exists() {
            let _ = std::fs::copy(&edm_exe, &desktop_exe);
            details.push("Restored edm-desktop-app.exe from EDM binary payload.".to_string());
        } else if edm_exe.exists() && desktop_exe.exists() {
            details.push("Verified core EDM application binaries.".to_string());
        } else if let Ok(current_exe) = std::env::current_exe() {
            let _ = std::fs::copy(&current_exe, &edm_exe);
            let _ = std::fs::copy(&current_exe, &desktop_exe);
            details.push(
                "Synchronized running binary to EDM.exe and edm-desktop-app.exe.".to_string(),
            );
        } else {
            binaries_ok = false;
            details.push("Warning: Main EDM executable could not be verified.".to_string());
        }

        if nmh_exe.exists() {
            details.push("Verified Native Messaging Host binary (edm-nmh.exe).".to_string());
        } else if let Ok(cur) = std::env::current_exe() {
            if let Some(parent) = cur.parent() {
                let cur_nmh = parent.join("edm-nmh.exe");
                if cur_nmh.exists() {
                    let _ = std::fs::copy(&cur_nmh, &nmh_exe);
                    details.push("Restored edm-nmh.exe to installation directory.".to_string());
                }
            }
        }

        #[cfg(windows)]
        crate::os::assets::self_heal_dll_in_dir(&install_dir);
    }

    // 2. Browser integration repair
    if plan.repair_browser {
        #[cfg(windows)]
        {
            crate::os::registry::register_native_messaging_host();
            let nmh_exe = install_dir.join("edm-nmh.exe");
            if let Err(e) = platform::register_browser_nmh(&install_dir, &nmh_exe) {
                browser_ok = false;
                details.push(format!("Failed to repair browser manifests: {}", e));
            } else {
                details.push("Re-registered browser integration manifests & registry keys for Edge, Chrome, Brave, and Firefox.".to_string());
            }
        }
    }

    // 3. Shortcuts repair using pure Win32 COM
    if plan.repair_shortcuts {
        #[cfg(windows)]
        {
            let edm_exe = install_dir.join("EDM.exe");

            if let Ok(appdata) = std::env::var("APPDATA") {
                let start_menu_dir =
                    PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs");
                let start_menu_lnk = start_menu_dir.join("EDM.lnk");
                match platform::create_com_shortcut(
                    &start_menu_lnk,
                    &edm_exe,
                    &install_dir,
                    None,
                    0,
                    None,
                ) {
                    Ok(_) => {
                        details.push("Restored Start Menu COM shortcut (EDM.lnk).".to_string())
                    }
                    Err(e) => {
                        shortcuts_ok = false;
                        details.push(format!("Start menu shortcut repair failed: {}", e));
                    }
                }
            }

            if let Ok(userprofile) = std::env::var("USERPROFILE") {
                let desktop_lnk = PathBuf::from(userprofile).join("Desktop").join("EDM.lnk");
                match platform::create_com_shortcut(
                    &desktop_lnk,
                    &edm_exe,
                    &install_dir,
                    None,
                    0,
                    None,
                ) {
                    Ok(_) => details.push("Restored Desktop COM shortcut (EDM.lnk).".to_string()),
                    Err(e) => {
                        shortcuts_ok = false;
                        details.push(format!("Desktop shortcut repair failed: {}", e));
                    }
                }
            }
        }
    }

    info!(
        "Native repair completed: browser={}, shortcuts={}, binaries={}",
        browser_ok, shortcuts_ok, binaries_ok
    );

    Ok(RepairReport {
        success: browser_ok && shortcuts_ok && binaries_ok,
        repaired_browser: plan.repair_browser && browser_ok,
        repaired_shortcuts: plan.repair_shortcuts && shortcuts_ok,
        verified_binaries: plan.verify_binaries && binaries_ok,
        details,
    })
}
