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

pub struct FtpAdapter;

#[async_trait]
impl AdaptiveWrapperAdapter for FtpAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        _client: &Client,
    ) -> Result<PreFlightPlan, String> {
        let mut clean_task = task.clone();
        // Clamp concurrent FTP sockets to 3 to prevent server 421 IP lockouts
        clean_task.config.max_connections = clean_task.config.max_connections.clamp(1, 3);

        Ok(PreFlightPlan {
            execution: PreFlightExecution::DirectProtocol {
                task: clean_task,
                custom_metadata: HashMap::new(),
            },
            chunk_strategy: ChunkingStrategy::StandardAdaptive {
                initial_chunk_size: 2 * 1024 * 1024,
                max_connections: 3,
            },
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
            return Err("FTP downloaded file does not exist".to_string());
        }
        Ok(ctx.final_path)
    }
}
