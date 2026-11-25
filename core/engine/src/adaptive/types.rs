use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccelerationMode {
    Auto,
    Optimizing,
    Stable,
    Limited,
    FallbackSingleStream,
}

impl AccelerationMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Optimizing => "Optimizing",
            Self::Stable => "Stable",
            Self::Limited => "Limited",
            Self::FallbackSingleStream => "Single Stream",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveConfig {
    pub initial_connections: u32,
    pub min_connections: u32,
    pub max_connections: u32,
    pub max_connections_per_host: u32,
    pub min_chunk_size: u64, // e.g. 1 MiB or 512 KiB
    pub sample_interval: Duration,
    pub cooldown_duration: Duration,
    pub throughput_gain_threshold: f64, // e.g. +8% (0.08)
    pub throughput_drop_threshold: f64, // e.g. -5% (-0.05)
    pub stall_timeout: Duration,        // e.g. 8s
    pub max_worker_retries: u32,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            initial_connections: 8,
            min_connections: 1,
            max_connections: 16,
            max_connections_per_host: 16,
            min_chunk_size: 1024 * 1024, // 1 MiB
            sample_interval: Duration::from_millis(250),
            cooldown_duration: Duration::from_secs(20), // 20s hysteresis window
            throughput_gain_threshold: 0.10,            // +10% sustained gain required
            throughput_drop_threshold: -0.15,           // -15% hysteresis against transient jitter
            stall_timeout: Duration::from_secs(8),
            max_worker_retries: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Connecting,
    Downloading,
    Stalled,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerTelemetry {
    pub idx: u32,
    pub start: u64,
    pub end: u64,
    pub pos: u64,
    pub bytes_downloaded: u64,
    pub speed_bps: u64,
    pub status: WorkerStatus,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveTelemetry {
    pub download_id: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub progress_pct: f64,
    pub instant_throughput_bps: u64,
    pub rolling_throughput_bps: u64,
    #[serde(default)]
    pub upload_throughput_bps: u64,
    #[serde(default)]
    pub uploaded_bytes: u64,
    pub active_connections: u32,
    #[serde(default)]
    pub target_connections: u32,
    #[serde(default)]
    pub live_workers: u32,
    pub max_allowed_connections: u32,
    pub mode: AccelerationMode,
    pub eta_seconds: Option<u64>,
    pub decision_reason: String,
    #[serde(default)]
    pub phase: Option<String>,
    pub workers: Vec<WorkerTelemetry>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveDecision {
    ScaleUp {
        from: u32,
        to: u32,
        reason: String,
    },
    ScaleDown {
        from: u32,
        to: u32,
        reason: String,
    },
    HoldStable {
        connections: u32,
        reason: String,
    },
    LimitRate {
        retry_after: Option<Duration>,
        reason: String,
    },
    FallbackSingleStream {
        reason: String,
    },
}

/// Per-task BitTorrent power options (subfolder, sequential, seeding caps, trackers/DHT/PEX).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TorrentPowerConfig {
    pub sequential: bool,
    pub seed_ratio_limit: Option<f64>,
    pub seed_time_limit_mins: Option<u64>,
    pub create_subfolder: bool,
    pub custom_folder_name: Option<String>,
    pub auto_update_trackers: bool,
    pub enable_dht: bool,
    pub enable_pex: bool,
}

impl Default for TorrentPowerConfig {
    fn default() -> Self {
        Self {
            sequential: false,
            seed_ratio_limit: None,
            seed_time_limit_mins: None,
            create_subfolder: true,
            custom_folder_name: None,
            auto_update_trackers: false,
            enable_dht: true,
            enable_pex: true,
        }
    }
}
