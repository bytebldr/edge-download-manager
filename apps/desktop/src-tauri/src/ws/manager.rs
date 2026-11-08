use super::types::*;
use edm_store::Store;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

pub struct WsManager {
    token: String,
    pub allowed_extension_id: String,
    pub store: Store,
    pub tasks: Arc<RwLock<HashMap<String, WsTaskItem>>>,
    pub epoch: String,
    pub rev: Arc<AtomicU64>,
    pub theme: Arc<RwLock<Option<String>>>,
    pub palette: Arc<RwLock<Option<String>>>,
    pub lifecycle_state: Arc<RwLock<Option<String>>>,
    pub connected_clients: Arc<RwLock<HashMap<String, ConnectedClientInfo>>>,
    pub tx_broadcast: broadcast::Sender<String>,
    pub tx_create_task: mpsc::Sender<CreateTaskCommand>,
    pub tx_action: mpsc::Sender<ActionCommand>,
    pub tx_refresh_task: mpsc::Sender<RefreshTaskCommand>,
    pub tx_probe_media: mpsc::Sender<ProbeMediaCommand>,
}

impl WsManager {
    pub fn new(
        token: String,
        allowed_extension_id: String,
        store: Store,
        tx_create_task: mpsc::Sender<CreateTaskCommand>,
        tx_action: mpsc::Sender<ActionCommand>,
        tx_refresh_task: mpsc::Sender<RefreshTaskCommand>,
        tx_probe_media: mpsc::Sender<ProbeMediaCommand>,
    ) -> Self {
        let (tx_broadcast, _) = broadcast::channel(128);
        Self {
            token,
            allowed_extension_id,
            store,
            tasks: Arc::new(RwLock::new(HashMap::new())),
            epoch: Uuid::new_v4().to_string(),
            rev: Arc::new(AtomicU64::new(1)),
            theme: Arc::new(RwLock::new(None)),
            palette: Arc::new(RwLock::new(None)),
            lifecycle_state: Arc::new(RwLock::new(Some("active".to_string()))),
            connected_clients: Arc::new(RwLock::new(HashMap::new())),
            tx_broadcast,
            tx_create_task,
            tx_action,
            tx_refresh_task,
            tx_probe_media,
        }
    }

    #[allow(dead_code)]
    pub async fn get_connected_browsers(&self) -> Vec<String> {
        let clients = self.connected_clients.read().await;
        clients.values().map(|c| c.browser.clone()).collect()
    }

    pub async fn is_browser_connected(&self, browser: &str) -> bool {
        let clients = self.connected_clients.read().await;
        clients
            .values()
            .any(|c| c.browser.eq_ignore_ascii_case(browser))
    }

    pub async fn get_browser_connected_id(&self, browser: &str) -> Option<String> {
        let clients = self.connected_clients.read().await;
        clients
            .values()
            .find(|c| c.browser.eq_ignore_ascii_case(browser))
            .and_then(|c| c.extension_id.clone())
    }

    pub async fn get_any_connected_extension_id(&self) -> Option<String> {
        let clients = self.connected_clients.read().await;
        clients.values().find_map(|c| c.extension_id.clone())
    }

    pub fn bump_rev(&self) -> u64 {
        self.rev.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub async fn update_theme_palette(&self, theme: Option<String>, palette: Option<String>) {
        if let Some(t) = theme {
            *self.theme.write().await = Some(t);
        }
        if let Some(p) = palette {
            *self.palette.write().await = Some(p);
        }
        self.bump_rev();
    }

    pub async fn update_lifecycle_state(&self, state: Option<String>) {
        *self.lifecycle_state.write().await = state;
        self.bump_rev();
    }

    pub async fn build_snapshot(&self) -> OutboundWsMessage {
        let tasks: Vec<WsTaskItem> = {
            let map = self.tasks.read().await;
            map.values().cloned().collect()
        };
        let theme = self.theme.read().await.clone();
        let palette = self.palette.read().await.clone();
        let lifecycle_state = self.lifecycle_state.read().await.clone();
        OutboundWsMessage::Snapshot {
            tasks,
            epoch: self.epoch.clone(),
            rev: self.rev.load(Ordering::Relaxed),
            theme,
            palette,
            lifecycle_state,
            extension_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }
    }

    pub async fn broadcast_snapshot(&self) {
        let msg = self.build_snapshot().await;
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = self.tx_broadcast.send(json);
        }
    }

    pub async fn broadcast_refresh_link(
        &self,
        task_id: &str,
        page_url: Option<&str>,
        filename: &str,
        size: Option<u64>,
    ) {
        let msg = OutboundWsMessage::RefreshLink {
            task_id: task_id.to_string(),
            page_url: page_url.map(String::from),
            filename: filename.to_string(),
            size,
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = self.tx_broadcast.send(json);
        }
    }

    pub async fn broadcast_power_countdown(&self, action: &str, seconds_remaining: u32) {
        let msg = OutboundWsMessage::PowerCountdown {
            action: action.to_string(),
            seconds_remaining,
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = self.tx_broadcast.send(json);
        }
    }

    pub async fn broadcast_task_removed(&self, id: &str) {
        let rev = self.bump_rev();
        {
            let mut tasks = self.tasks.write().await;
            tasks.remove(id);
        }
        let msg = OutboundWsMessage::TaskRemoved {
            id: id.to_string(),
            epoch: self.epoch.clone(),
            rev,
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = self.tx_broadcast.send(json);
        }
    }

    pub fn get_token(&self) -> &str {
        &self.token
    }

    pub fn get_live_tasks(&self) -> Arc<RwLock<HashMap<String, WsTaskItem>>> {
        self.tasks.clone()
    }

    pub async fn register_task(
        &self,
        id: String,
        filename: String,
        save_dir: PathBuf,
        total: Option<u64>,
        status: String,
    ) {
        let mut map = self.tasks.write().await;
        map.insert(
            id.clone(),
            WsTaskItem {
                id,
                filename,
                save_dir,
                total,
                downloaded: 0,
                speed: 0,
                status,
                error_msg: None,
            },
        );
    }

    /// Constant-time token verification to defend against timing attacks
    pub fn verify_token_constant_time(&self, candidate: &str) -> bool {
        if candidate.len() != self.token.len() {
            return false;
        }
        candidate.as_bytes().ct_eq(self.token.as_bytes()).into()
    }

    pub async fn update_task_progress(
        &self,
        id: &str,
        downloaded: u64,
        total: Option<u64>,
        speed: u64,
        status: &str,
    ) {
        let mut map = self.tasks.write().await;
        if let Some(item) = map.get_mut(id) {
            item.downloaded = downloaded;
            if total.is_some() {
                item.total = total;
            }
            item.speed = speed;
            item.status = status.to_string();
        }
    }

    #[allow(dead_code)]
    pub async fn remove_task(&self, id: &str) {
        let mut map = self.tasks.write().await;
        map.remove(id);
    }

    pub async fn update_task_error(&self, id: &str, error_msg: Option<String>) {
        let mut map = self.tasks.write().await;
        if let Some(item) = map.get_mut(id) {
            item.error_msg = error_msg;
            item.status = "failed".to_string();
            item.speed = 0;
        }
    }

    pub async fn sync_from_store(&self) {
        if let Ok(records) = self.store.list_all_downloads() {
            let mut map = self.tasks.write().await;
            for r in records {
                let status = match r.state {
                    edm_ipc::DownloadState::Queued => "queued",
                    edm_ipc::DownloadState::Downloading => "downloading",
                    edm_ipc::DownloadState::Paused => "paused",
                    edm_ipc::DownloadState::Completed => "completed",
                    edm_ipc::DownloadState::Failed => "failed",
                    edm_ipc::DownloadState::Cancelled => "cancelled",
                    _ => "queued",
                };
                map.insert(
                    r.id.to_string(),
                    WsTaskItem {
                        id: r.id.to_string(),
                        filename: r.filename,
                        save_dir: PathBuf::from(r.save_dir),
                        total: r.size_total,
                        downloaded: r.size_done,
                        speed: 0,
                        status: status.to_string(),
                        error_msg: r.error_msg,
                    },
                );
            }
        }
    }
}

pub fn get_or_create_token() -> String {
    let config_dir = match std::env::var("APPDATA") {
        Ok(appdata) => PathBuf::from(appdata).join("EDM").join("config"),
        Err(_) => std::env::temp_dir().join("EDM").join("config"),
    };
    let _ = std::fs::create_dir_all(&config_dir);
    let token_file = config_dir.join("token.txt");

    if let Ok(existing) = std::fs::read_to_string(&token_file) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() && trimmed.len() >= 32 {
            return trimmed.to_string();
        }
    }

    let raw_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let _ = std::fs::write(&token_file, &raw_token);
    raw_token
}

/// Validate that Host header matches 127.0.0.1 or localhost (DNS rebinding defense)
pub fn is_valid_loopback_host(host_header: Option<&str>, port: u16) -> bool {
    if let Some(host) = host_header {
        let host_trimmed = host.trim();
        let expected_ip = format!("127.0.0.1:{}", port);
        let expected_lh = format!("localhost:{}", port);
        host_trimmed == expected_ip
            || host_trimmed == expected_lh
            || host_trimmed == "127.0.0.1"
            || host_trimmed == "localhost"
    } else {
        false
    }
}
