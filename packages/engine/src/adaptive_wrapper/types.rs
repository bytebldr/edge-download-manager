use crate::adaptive::AdaptiveDownloadTask;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkingStrategy {
    StandardAdaptive { initial_chunk_size: u64, max_connections: usize },
    CdnAntiThrottling { slice_size: u64, max_connections: usize },
    FixedPartSize { part_size: u64, align_boundaries: bool }, // eD2k: 9,728,000 bytes
    DirectStreamPiped,
}

#[derive(Debug, Clone)]
pub struct EndpointCandidate {
    pub name: String,
    pub url: String,
    pub priority: u8,
}

#[derive(Clone)]
pub struct DualStreamBundle {
    pub video_task: AdaptiveDownloadTask,
    pub audio_task: AdaptiveDownloadTask,
    pub output_container: String, // "mp4", "mkv", "webm"
}

#[derive(Clone)]
pub enum PreFlightExecution {
    SingleStream(AdaptiveDownloadTask),
    DualParallelStream(DualStreamBundle),
    DirectProtocol {
        task: AdaptiveDownloadTask,
        custom_metadata: HashMap<String, String>,
    },
}

#[derive(Clone)]
pub struct PreFlightPlan {
    pub execution: PreFlightExecution,
    pub chunk_strategy: ChunkingStrategy,
    pub expected_size: Option<u64>,
    pub expected_hash: Option<(String, String)>, // ("sha256" | "blake3" | "md4", Hash)
    pub suggested_filename: Option<String>,
    pub fallback_urls: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PostFlightContext {
    pub final_path: PathBuf,
    pub temp_paths: Vec<PathBuf>,
    pub expected_hash: Option<(String, String)>,
}
