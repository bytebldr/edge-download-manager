use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use crate::adaptive::TelemetryCallback;
use async_trait::async_trait;
use reqwest::Client;
use std::path::PathBuf;

pub struct HttpAdapter;

#[async_trait]
impl AdaptiveWrapperAdapter for HttpAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        client: &Client,
    ) -> Result<PreFlightPlan, String> {
        let clean_task = task.clone();
        let max_conns = clean_task.config.max_connections.clamp(1, 32) as usize;

        // Probe range capability with a lightweight Range: bytes=0-0 probe
        let probe_res = client
            .get(&clean_task.url)
            .header("Range", "bytes=0-0")
            .header("Accept-Encoding", "identity")
            .send()
            .await;

        let (supports_range, expected_size) = match probe_res {
            Ok(resp) => {
                let status = resp.status();
                let ranges = resp
                    .headers()
                    .get("Accept-Ranges")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.to_ascii_lowercase())
                    .unwrap_or_default();

                let is_partial = status.as_u16() == 206;
                let accepts_bytes = ranges.contains("bytes");

                let content_range_len = resp
                    .headers()
                    .get("Content-Range")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|cr| cr.split('/').next_back()?.trim().parse::<u64>().ok());

                let content_len = resp.content_length();
                (is_partial || accepts_bytes, content_range_len.or(content_len))
            }
            _ => (true, None),
        };

        let chunk_strategy = if supports_range {
            ChunkingStrategy::StandardAdaptive {
                initial_chunk_size: 4 * 1024 * 1024,
                max_connections: max_conns,
            }
        } else {
            ChunkingStrategy::StandardAdaptive {
                initial_chunk_size: 0,
                max_connections: 1,
            }
        };

        Ok(PreFlightPlan {
            execution: PreFlightExecution::SingleStream(clean_task),
            chunk_strategy,
            expected_size,
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
