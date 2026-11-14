//! Process spawning logic for edm-nmh to wake EDM.exe.
//!
//! Windows Invariant: Must use DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP.
//! Must NEVER use CREATE_NO_WINDOW when launching EDM.exe, as WebView2 requires
//! standard Windows subsystem initialization or it hangs silently.

use std::path::Path;
use std::process::{Child, Command};
use tracing::info;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const DETACHED_PROCESS: u32 = 0x00000008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

/// Launches EDM.exe in a detached process group safe for WebView2.
pub fn spawn_detached_edm(exe_path: &Path, background: bool) -> std::io::Result<Child> {
    info!("[Handoff Spawn] Launching EDM.exe at {:?}", exe_path);

    let mut cmd = Command::new(exe_path);
    if background {
        cmd.arg("--background");
    }

    #[cfg(windows)]
    {
        // Use DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP.
        // DO NOT add CREATE_NO_WINDOW (0x08000000) - causes WebView2 GUI init locks.
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    cmd.spawn()
}

/// Terminates any unresponsive/stale background EDM instances blocking single-instance.
pub fn terminate_stale_edm_instances() {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "EDM.exe", "/T"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "edm-desktop-app.exe", "/T"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
}
