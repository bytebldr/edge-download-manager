use tauri::{command, State};

use crate::os::*;
use crate::state::AppState;

#[command]
pub fn get_extension_token(state: State<'_, AppState>) -> String {
    state.ws_manager.get_token().to_string()
}

#[command]
pub async fn get_browser_diagnostics(
    state: State<'_, AppState>,
    ext_id: Option<String>,
) -> Result<edm_ipc::DiagnosticsReport, String> {
    Ok(collect_browser_diagnostics(ext_id, Some(&state.ws_manager)).await)
}

#[command]
pub fn repair_native_host_registration() -> Result<String, String> {
    #[cfg(windows)]
    {
        register_native_messaging_host();
        Ok("Native messaging host registration repaired successfully".into())
    }
    #[cfg(not(windows))]
    {
        Err("Windows only".into())
    }
}

#[command]
pub async fn detect_installed_browsers(
    state: State<'_, AppState>,
) -> Result<Vec<edm_ipc::BrowserSetupInfo>, String> {
    detect_installed_browsers_internal(&state).await
}

#[command]
pub fn prepare_extension_files() -> Result<String, String> {
    prepare_extension_files_internal()
}

#[command]
pub fn get_extension_folder_path() -> Result<String, String> {
    get_extension_folder_path_internal()
}

#[command]
pub fn remove_extension_folder() -> Result<bool, String> {
    remove_extension_folder_internal()
}

#[command]
pub fn open_browser_url(browser: String, url: String) -> Result<(), String> {
    open_browser_url_internal(browser, url)
}

#[command]
pub fn get_firefox_xpi_url() -> String {
    "http://127.0.0.1:14370/firefox.xpi".to_string()
}
