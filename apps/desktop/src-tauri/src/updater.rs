pub use crate::lifecycle::{is_newer_version, parse_version, OtaProgress, UpdateCheckResult};
use tauri::{AppHandle, State};

#[tauri::command]
pub fn get_app_version() -> String {
    crate::lifecycle::get_app_version()
}

#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    crate::lifecycle::check_for_updates().await
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, crate::AppState>,
    download_url: String,
) -> Result<(), String> {
    crate::lifecycle::install_update(app, state, download_url).await
}

#[tauri::command]
pub fn open_installer_folder() -> Result<(), String> {
    crate::lifecycle::open_installer_folder()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing_and_comparison() {
        assert_eq!(parse_version("v0.0.3"), Some((0, 0, 3)));
        assert_eq!(parse_version("0.0.3"), Some((0, 0, 3)));
        assert_eq!(parse_version("v1.2.34-rc1"), Some((1, 2, 34)));

        assert!(!is_newer_version("0.0.3", "0.0.3"));
        assert!(!is_newer_version("v0.0.3", "0.0.3"));
        assert!(!is_newer_version("0.0.2", "0.0.3"));
        assert!(is_newer_version("0.0.4", "0.0.3"));
        assert!(is_newer_version("0.1.0", "0.0.3"));
        assert!(is_newer_version("1.0.0", "0.0.3"));
    }
}
