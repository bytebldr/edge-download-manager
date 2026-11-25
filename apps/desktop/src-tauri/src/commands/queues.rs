use tauri::{command, State};

use crate::state::AppState;

#[command]
pub fn get_queues(state: State<'_, AppState>) -> Result<Vec<edm_store::QueueRecord>, String> {
    state.store.list_queues().map_err(|e| e.to_string())
}
