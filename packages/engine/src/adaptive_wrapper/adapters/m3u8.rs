use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use crate::adaptive::TelemetryCallback;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct M3u8Adapter;

#[async_trait]
impl AdaptiveWrapperAdapter for M3u8Adapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        _client: &Client,
    ) -> Result<PreFlightPlan, String> {
        Ok(PreFlightPlan {
            execution: PreFlightExecution::DirectProtocol {
                task: task.clone(),
                custom_metadata: HashMap::new(),
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
            return Err("M3U8 rendered file does not exist".to_string());
        }
        Ok(ctx.final_path)
    }
}
