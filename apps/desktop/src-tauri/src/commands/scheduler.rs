use chrono::Local;
use edm_core::scheduler::ScheduleWindow;
use serde::{Deserialize, Serialize};
use tauri::{command, State};

use crate::state::AppState;

const SCHEDULER_SETTING_KEY: &str = "edm_queues";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledQueue {
    pub id: String,
    pub name: String,
    pub max_concurrent: u32,
    pub max_speed_bps: Option<u64>,
    #[serde(default)]
    pub task_ids: Vec<String>,
    pub enabled: bool,
    #[serde(default)]
    pub schedule: ScheduleWindow,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueScheduleEvaluation {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub within_window: bool,
    pub active_allowed: bool,
    pub max_concurrent: u32,
    pub max_speed_bps: Option<u64>,
}

fn load_queues(state: &AppState) -> Result<Vec<ScheduledQueue>, String> {
    match state.store.get_setting(SCHEDULER_SETTING_KEY) {
        Ok(Some(raw)) if !raw.trim().is_empty() => {
            serde_json::from_str::<Vec<ScheduledQueue>>(&raw).map_err(|e| e.to_string())
        }
        Ok(_) => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}

fn persist_queues(state: &AppState, queues: &[ScheduledQueue]) -> Result<(), String> {
    let json = serde_json::to_string(queues).map_err(|e| e.to_string())?;
    state
        .store
        .set_setting(SCHEDULER_SETTING_KEY, &json)
        .map_err(|e| e.to_string())
}

#[command]
pub fn list_queues(state: State<'_, AppState>) -> Result<Vec<ScheduledQueue>, String> {
    load_queues(&state)
}

#[command]
pub fn save_queue(
    state: State<'_, AppState>,
    mut queue: ScheduledQueue,
) -> Result<Vec<ScheduledQueue>, String> {
    queue.max_concurrent = queue.max_concurrent.max(1);
    let mut queues = load_queues(&state)?;
    if let Some(existing) = queues.iter_mut().find(|q| q.id == queue.id) {
        *existing = queue;
    } else {
        queues.push(queue);
    }
    persist_queues(&state, &queues)?;
    Ok(queues)
}

#[command]
pub fn delete_queue(
    state: State<'_, AppState>,
    queue_id: String,
) -> Result<Vec<ScheduledQueue>, String> {
    let mut queues = load_queues(&state)?;
    queues.retain(|q| q.id != queue_id);
    persist_queues(&state, &queues)?;
    Ok(queues)
}

#[command]
pub fn evaluate_queue_schedules(
    state: State<'_, AppState>,
) -> Result<Vec<QueueScheduleEvaluation>, String> {
    let queues = load_queues(&state)?;
    let now = Local::now();
    Ok(queues
        .into_iter()
        .map(|q| {
            let within_window = q.schedule.is_within_window(now);
            QueueScheduleEvaluation {
                id: q.id,
                name: q.name,
                enabled: q.enabled,
                within_window,
                active_allowed: q.enabled && within_window,
                max_concurrent: q.max_concurrent,
                max_speed_bps: q.max_speed_bps,
            }
        })
        .collect())
}
