use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use crate::adaptive::TelemetryCallback;
use crate::packs::ytdlp::YtDlpPack;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct YtDlpAdapter;

#[async_trait]
impl AdaptiveWrapperAdapter for YtDlpAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        _client: &Client,
    ) -> Result<PreFlightPlan, String> {
        let mut metadata = HashMap::new();
        if let Some(ffmpeg) = YtDlpPack::locate_ffmpeg() {
            metadata.insert(
                "ffmpeg_bin".to_string(),
                ffmpeg.to_string_lossy().to_string(),
            );
        }
        if let Some(node) = YtDlpPack::locate_node() {
            metadata.insert("node_bin".to_string(), node.to_string_lossy().to_string());
        }

        // Direct protocol execution invokes PackExecutor::execute_ytdlp -> YtDlpPack::download_media
        // with full streaming telemetry, player challenge solving, and automated FFmpeg transmuxing.
        Ok(PreFlightPlan {
            execution: PreFlightExecution::DirectProtocol {
                task: task.clone(),
                custom_metadata: metadata,
            },
            chunk_strategy: ChunkingStrategy::DirectStreamPiped,
            expected_size: None,
            expected_hash: None,
            suggested_filename: None,
            fallback_urls: Vec::new(),
        })
    }

    async fn post_flight(
        &self,
        ctx: PostFlightContext,
        _telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        if !ctx.final_path.exists() {
            return Err("Media destination file does not exist on disk".to_string());
        }
        Ok(ctx.final_path)
    }
}
