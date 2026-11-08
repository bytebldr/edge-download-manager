use tauri::{command, State};

use crate::state::AppState;

#[command]
pub fn get_history(
    state: State<'_, AppState>,
    limit: Option<usize>,
    query: Option<String>,
) -> Result<Vec<edm_store::HistoryRecord>, String> {
    state
        .store
        .list_history(limit, query.as_deref())
        .map_err(|e| e.to_string())
}

#[command]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.store.clear_history().map_err(|e| e.to_string())
}

#[command]
pub fn export_history_csv(state: State<'_, AppState>) -> Result<String, String> {
    state.store.export_history_csv().map_err(|e| e.to_string())
}
