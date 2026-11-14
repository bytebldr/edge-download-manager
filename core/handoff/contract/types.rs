//! EDM Communication Contract - Rust Definitions
//! Shared message types, invariants, and constants for Extension <-> Bridge <-> Desktop IPC.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::path::PathBuf;

pub const WS_PORT: u16 = 14370;
pub const WS_BIND_ADDR: &str = "127.0.0.1:14370";
pub const MAX_WS_MESSAGE_SIZE: usize = 64 * 1024; // 64 KiB
pub const MAX_CONCURRENT_CONNECTIONS: usize = 32;

pub const CHROMIUM_EXTENSION_IDS: &[&str] = &[
    "ginpdimhcdbojbhemgjedghkeolbcmgj",
    "ebdcbpghekpbgjiclaclbgmdnimeheci",
    "knldjmfmopnpolahpmmgbagdohdnhkik",
    "cphjfljhggknldkppchfobogknclmblg",
];

pub const FIREFOX_EXTENSION_ID: &str = "edm@suvesh.dev";

/// Parse a semver string (e.g. "0.0.14", "v0.0.13", "1.2.3-beta") into (major, minor, patch).
pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let clean = v.trim().trim_start_matches(['v', 'V']);
    let core = clean.split(&['-', '+'][..]).next()?.trim();
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch = parts[2].parse::<u64>().ok()?;
    Some((major, minor, patch))
}

/// Compare two semver strings numerically.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (parse_semver(a), parse_semver(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        _ => a.trim().cmp(b.trim()),
    }
}

/// Returns true if the given extension ID is in the recognized whitelist.
pub fn is_allowed_extension_id(id: &str) -> bool {
    id == FIREFOX_EXTENSION_ID || CHROMIUM_EXTENSION_IDS.contains(&id)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WsTaskItem {
    pub id: String,
    pub filename: String,
    pub save_dir: PathBuf,
    pub total: Option<u64>,
    pub downloaded: u64,
    pub speed: u64,
    pub status: String,
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
    },
    CreateTask {
        req_id: String,
        url: String,
        filename: Option<String>,
        referrer: Option<String>,
        user_agent: Option<String>,
        cookies: Option<Vec<WsCookie>>,
    },
    Action {
        req_id: String,
        id: String,
        action: String,
        #[serde(default)]
        delete_file: Option<bool>,
    },
    RefreshTask {
        req_id: String,
        task_id: String,
        new_url: String,
    },
    ProbeMedia {
        req_id: String,
        url: String,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundWsMessage {
    Ack {
        req_id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Snapshot {
        epoch: String,
        rev: u64,
        tasks: Vec<WsTaskItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        theme: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        palette: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        lifecycle_state: Option<String>,
    },
    Pong,
}
