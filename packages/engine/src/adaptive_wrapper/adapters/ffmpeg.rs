use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use crate::adaptive::TelemetryCallback;
use crate::packs::ffmpeg::FfmpegPack;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct FfmpegAdapter;

#[async_trait]
impl AdaptiveWrapperAdapter for FfmpegAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        _client: &Client,
    ) -> Result<PreFlightPlan, String> {
        let caps = FfmpegPack::get_capabilities().await;
        let mut metadata = HashMap::new();
        metadata.insert(
            "hwaccel_nvenc".to_string(),
            caps.supports_nvenc.to_string(),
        );
        metadata.insert("hwaccel_qsv".to_string(), caps.supports_qsv.to_string());

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
            return Err("FFmpeg output file does not exist".to_string());
        }
        Ok(ctx.final_path)
    }
}
