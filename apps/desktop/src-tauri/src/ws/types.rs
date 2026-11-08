use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;

#[allow(dead_code)]
pub const WS_PORT: u16 = 14370;
pub const WS_BIND_ADDR: &str = "127.0.0.1:14370";
pub const EXTENSION_ID: &str = "ginpdimhcdbojbhemgjedghkeolbcmgj";
#[allow(dead_code)]
pub const FIREFOX_EXTENSION_ID: &str = "edm@suvesh.dev";

// Security invariants
pub const MAX_WS_MESSAGE_SIZE: usize = 64 * 1024; // 64 KiB
pub const MAX_CONCURRENT_CONNECTIONS: usize = 32;

pub static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WsTaskItem {
    pub id: String,
    pub filename: String,
    pub save_dir: PathBuf,
    pub total: Option<u64>,
    pub downloaded: u64,
    pub speed: u64,
    pub status: String, // "queued" | "downloading" | "paused" | "completed" | "failed"
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error_msg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsCookie {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InboundWsMessage {
    Hello {
        token: String,
        #[serde(default)]
        browser: Option<String>,
        #[serde(default)]
        version: Option<String>,
        #[serde(default)]
        extension_id: Option<String>,
    },
    Ping,
    Subscribe,
    CreateTask {
        req_id: String,
        url: String,
        #[serde(default)]
        cookies: Vec<WsCookie>,
        #[serde(default)]
        referrer: Option<String>,
        #[serde(default)]
        user_agent: Option<String>,
        #[serde(default)]
        filename: Option<String>,
    },
    TaskAction {
        #[serde(default)]
        req_id: Option<String>,
        id: String,
        action: String, // "pause" | "resume" | "cancel" | "remove" | "open_file" | "open_folder"
        #[serde(default)]
        delete_file: Option<bool>,
    },
    RefreshTask {
        #[serde(default)]
        req_id: Option<String>,
        task_id: String,
        new_url: String,
        #[serde(default)]
        headers: Option<HashMap<String, String>>,
    },
    ProbeMedia {
        req_id: String,
        media_url: String,
    },
    BatchAdd {
        req_id: String,
        items: Vec<BatchAddItem>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAddItem {
    pub url: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub referrer: Option<String>,
}

impl InboundWsMessage {
    #[cfg(test)]
    pub fn hello(token: impl Into<String>) -> Self {
        Self::Hello {
            token: token.into(),
            browser: None,
            version: None,
            extension_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedClientInfo {
    pub client_id: String,
    pub browser: String,
    pub version: String,
    pub extension_id: Option<String>,
    pub connected_at_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundWsMessage {
    Ready {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extension_version: Option<String>,
    },
    Pong,
    Ack {
        req_id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Snapshot {
        tasks: Vec<WsTaskItem>,
        #[serde(default)]
        epoch: String,
        #[serde(default)]
        rev: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        theme: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        palette: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        lifecycle_state: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        extension_version: Option<String>,
    },
    TaskRemoved {
        id: String,
        epoch: String,
        rev: u64,
    },
    RefreshLink {
        task_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        page_url: Option<String>,
        filename: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        size: Option<u64>,
    },
    MediaVariants {
        req_id: String,
        variants: Vec<edm_ipc::MediaVariant>,
        is_drm: bool,
    },
    PowerCountdown {
        action: String,
        seconds_remaining: u32,
    },
}

#[derive(Debug)]
pub struct CreateTaskCommand {
    #[allow(dead_code)]
    pub req_id: String,
    pub url: String,
    pub cookies: Vec<WsCookie>,
    pub referrer: Option<String>,
    pub user_agent: Option<String>,
    pub filename: Option<String>,
    pub resp_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
}

#[derive(Debug)]
pub struct ActionCommand {
    #[allow(dead_code)]
    pub req_id: Option<String>,
    pub id: String,
    pub action: String,
    pub delete_file: bool,
    pub resp_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
}

#[derive(Debug)]
pub struct RefreshTaskCommand {
    #[allow(dead_code)]
    pub req_id: Option<String>,
    pub task_id: String,
    pub new_url: String,
    pub headers: Option<HashMap<String, String>>,
    pub resp_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
}

#[derive(Debug)]
pub struct ProbeMediaCommand {
    #[allow(dead_code)]
    pub req_id: String,
    pub media_url: String,
    pub resp_tx: tokio::sync::oneshot::Sender<Result<(Vec<edm_ipc::MediaVariant>, bool), String>>,
}
