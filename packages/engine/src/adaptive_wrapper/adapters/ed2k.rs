use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use crate::adaptive::TelemetryCallback;
use crate::packs::ed2k::ED2K_CHUNK_SIZE;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Ed2kAdapter;

#[async_trait]
impl AdaptiveWrapperAdapter for Ed2kAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        _client: &Client,
    ) -> Result<PreFlightPlan, String> {
        let (file_name, file_size, file_hash) =
            if let Ok(info) = crate::packs::ed2k::Ed2kPack::parse_link(&task.url) {
                (Some(info.name), Some(info.size), Some(info.hash_hex))
            } else {
                (None, None, None)
            };

        Ok(PreFlightPlan {
            execution: PreFlightExecution::DirectProtocol {
                task: task.clone(),
                custom_metadata: HashMap::new(),
            },
            chunk_strategy: ChunkingStrategy::FixedPartSize {
                part_size: ED2K_CHUNK_SIZE as u64,
                align_boundaries: true,
            },
            expected_size: file_size,
            expected_hash: file_hash.map(|h| ("md4".to_string(), h)),
            suggested_filename: file_name,
            fallback_urls: Vec::new(),
        })
    }

    async fn post_flight(
        &self,
        ctx: PostFlightContext,
        _telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        if !ctx.final_path.exists() {
            return Err("eD2k target file does not exist".to_string());
        }
        Ok(ctx.final_path)
    }
}
