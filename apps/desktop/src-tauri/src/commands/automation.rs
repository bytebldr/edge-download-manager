use edm_core::automation::{dispatch_completion_webhook, validate_webhook_url, AutomationConfig};
use serde_json::json;
use tauri::{command, State};

use crate::state::AppState;

const AUTOMATION_SETTING_KEY: &str = "edm_automation_config";

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
    fn SetProcessWorkingSetSize(
        h_process: *mut std::ffi::c_void,
        dw_minimum_working_set_size: usize,
        dw_maximum_working_set_size: usize,
    ) -> i32;
}

#[command]
pub fn get_automation_config(state: State<'_, AppState>) -> Result<AutomationConfig, String> {
    match state.store.get_setting(AUTOMATION_SETTING_KEY) {
        Ok(Some(raw)) if !raw.trim().is_empty() => {
            serde_json::from_str::<AutomationConfig>(&raw).map_err(|e| e.to_string())
        }
        Ok(_) => Ok(AutomationConfig::default()),
        Err(e) => Err(e.to_string()),
    }
}

#[command]
pub fn save_automation_config(
    state: State<'_, AppState>,
    mut config: AutomationConfig,
) -> Result<AutomationConfig, String> {
    config.rpc_secret = config.rpc_secret.trim().to_string();
    config.webhook_url = config
        .webhook_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| validate_webhook_url(s).map(|u| u.to_string()))
        .transpose()?;

    let json_str = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    state
        .store
        .set_setting(AUTOMATION_SETTING_KEY, &json_str)
        .map_err(|e| e.to_string())?;

    Ok(config)
}

#[command]
pub async fn test_automation_webhook(url: String) -> Result<u16, String> {
    let payload = json!({
        "event": "edm.test",
        "source": "EDM",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    dispatch_completion_webhook(&url, payload).await
}

#[command]
pub fn trim_idle_webview_memory() -> Result<bool, String> {
    #[cfg(windows)]
    unsafe {
        let ok = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX) != 0;
        Ok(ok)
    }

    #[cfg(not(windows))]
    {
        Ok(false)
    }
}
