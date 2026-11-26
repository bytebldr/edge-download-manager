use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive::TelemetryCallback;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use async_trait::async_trait;
use reqwest::Client;
use std::path::PathBuf;

pub struct HttpAdapter;

#[async_trait]
impl AdaptiveWrapperAdapter for HttpAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        _client: &Client,
    ) -> Result<PreFlightPlan, String> {
        let clean_task = task.clone();
        let max_conns = clean_task.config.max_connections.clamp(1, 64) as usize;

        // Defer authoritative Range/header/ALPN probing to AdaptiveEngine (with full cookies/UA/referer).
        let chunk_strategy = ChunkingStrategy::StandardAdaptive {
            initial_chunk_size: clean_task.config.min_chunk_size,
            max_connections: max_conns,
        };

        Ok(PreFlightPlan {
            execution: PreFlightExecution::SingleStream(clean_task),
            chunk_strategy,
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
            return Err("Target file does not exist after download".to_string());
        }
        Ok(ctx.final_path)
    }
}
