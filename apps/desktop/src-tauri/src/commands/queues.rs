use tauri::{command, State};

use crate::state::AppState;

#[command]
pub fn get_queues(state: State<'_, AppState>) -> Result<Vec<edm_store::QueueRecord>, String> {
    state.store.list_queues().map_err(|e| e.to_string())
}

#[command]
pub fn save_queue(state: State<'_, AppState>, queue: edm_store::QueueRecord) -> Result<(), String> {
    state.store.upsert_queue(&queue).map_err(|e| e.to_string())
}

#[command]
pub fn delete_queue(state: State<'_, AppState>, queue_id: String) -> Result<(), String> {
    state
        .store
        .delete_queue(&queue_id)
        .map_err(|e| e.to_string())
}
