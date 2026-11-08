use tauri::{command, State};

use crate::state::AppState;

#[command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Option<serde_json::Value>, String> {
    match state.store.get_setting("app_settings") {
        Ok(Some(json_str)) => {
            let val: serde_json::Value =
                serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
            Ok(Some(val))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let json_str = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    state
        .store
        .set_setting("app_settings", &json_str)
        .map_err(|e| e.to_string())?;

    let theme = settings
        .get("general")
        .and_then(|g| g.get("theme"))
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("theme").and_then(|v| v.as_str()))
        .map(String::from);
    let palette = settings
        .get("general")
        .and_then(|g| g.get("palette"))
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("palette").and_then(|v| v.as_str()))
        .map(String::from);

    state.ws_manager.update_theme_palette(theme, palette).await;
    state.ws_manager.broadcast_snapshot().await;

    let keep_awake = settings
        .get("general")
        .and_then(|g| g.get("keepAwake"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let idle_exit_mins = settings
        .get("general")
        .and_then(|g| g.get("idleExitMinutes"))
        .and_then(|v| {
            if v.is_null() || v.as_str() == Some("never") {
                None
            } else {
                v.as_u64()
            }
        });
    state
        .lifecycle
        .update_settings(keep_awake, idle_exit_mins)
        .await;

    if let Some(limit) = settings
        .get("downloads")
        .and_then(|d| d.get("globalSpeedLimitBytesPerSecond"))
        .and_then(|v| v.as_u64())
    {
        state.global_limiter.set_limit(limit);
    } else {
        state.global_limiter.set_limit(0);
    }

    Ok(serde_json::json!({ "status": "saved" }))
}

#[command]
pub fn is_wizard_completed(state: State<'_, AppState>) -> Result<bool, String> {
    if let Ok(Some(val)) = state.store.get_setting("setup_completed") {
        return Ok(val == "true");
    }
    match state.store.get_setting("wizard_completed") {
        Ok(Some(val)) => Ok(val == "true"),
        _ => Ok(false),
    }
}

#[command]
pub fn set_wizard_completed(state: State<'_, AppState>, completed: bool) -> Result<(), String> {
    let val = if completed { "true" } else { "false" };
    state
        .store
        .set_setting("setup_completed", val)
        .map_err(|e| e.to_string())?;
    let _ = state.store.set_setting("wizard_completed", val);
    Ok(())
}
