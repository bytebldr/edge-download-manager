#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
use std::path::{Path, PathBuf};

#[cfg(not(windows))]
pub fn create_com_shortcut(
    _shortcut_path: &Path,
    _target_exe: &Path,
    _work_dir: &Path,
    _icon_path: Option<&Path>,
    _icon_index: i32,
    _args: Option<&str>,
) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn terminate_sibling_instances() -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn check_file_locked(_path: &Path) -> bool {
    false
}

#[cfg(not(windows))]
pub fn check_process_file_locks(_paths: &[PathBuf]) -> bool {
    false
}

#[cfg(not(windows))]
pub fn register_autostart(_edm_exe: &Path, _enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn register_uninstall_entry(_target_dir: &Path, _edm_exe: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn remove_uninstall_entry() {}

#[cfg(not(windows))]
pub fn register_protocol_handlers(_edm_exe: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn remove_protocol_handlers() {}

#[cfg(not(windows))]
pub fn register_browser_nmh(_target_dir: &Path, _nmh_exe: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn remove_browser_nmh() {}
