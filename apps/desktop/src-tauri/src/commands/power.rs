use tauri::{command, State};

use crate::features;
use crate::state::AppState;

#[command]
pub fn cancel_power_countdown(state: State<'_, AppState>) -> Result<(), String> {
    state.power_countdown.cancel();
    Ok(())
}

#[command]
pub fn get_power_countdown_status(
    state: State<'_, AppState>,
) -> Result<features::PowerCountdownInfo, String> {
    Ok(state.power_countdown.status())
}
