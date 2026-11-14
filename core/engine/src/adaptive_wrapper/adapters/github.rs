use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive_wrapper::scout::Scout;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, EndpointCandidate, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use crate::adaptive::TelemetryCallback;
use crate::packs::github::GithubPack;
use async_trait::async_trait;
use reqwest::Client;
use std::path::PathBuf;

pub struct GithubAdapter;

#[async_trait]
impl AdaptiveWrapperAdapter for GithubAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        client: &Client,
    ) -> Result<PreFlightPlan, String> {
        let clean_url = task.url.trim().to_string();

        let candidates = vec![
            EndpointCandidate {
                name: "origin".to_string(),
                url: clean_url.clone(),
                priority: 1,
            },
            EndpointCandidate {
                name: "ghproxy".to_string(),
                url: GithubPack::accelerate_url(&clean_url, "ghproxy"),
                priority: 2,
            },
            EndpointCandidate {
                name: "kgithub".to_string(),
                url: GithubPack::accelerate_url(&clean_url, "kgithub"),
                priority: 3,
            },
        ];

        let winning_url = Scout::race_endpoints(&candidates, &clean_url, client).await;

        let mut accelerated_task = task.clone();
        accelerated_task.url = winning_url.clone();

        let fallbacks: Vec<String> = candidates
            .into_iter()
            .map(|c| c.url)
            .filter(|u| u != &winning_url)
            .collect();

        Ok(PreFlightPlan {
            execution: PreFlightExecution::SingleStream(accelerated_task),
            chunk_strategy: ChunkingStrategy::StandardAdaptive {
                initial_chunk_size: 4 * 1024 * 1024,
                max_connections: task.config.max_connections.clamp(2, 32) as usize,
            },
            expected_size: None,
            expected_hash: None,
            suggested_filename: None,
            fallback_urls: fallbacks,
        })
    }

    async fn post_flight(
        &self,
        ctx: PostFlightContext,
        _telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        if !ctx.final_path.exists() {
            return Err("GitHub release asset does not exist on disk".to_string());
        }

        // Validate hash if expected hash was registered
        if let Some((algo, expected)) = ctx.expected_hash {
            if algo.eq_ignore_ascii_case("sha256") {
                let bytes = tokio::fs::read(&ctx.final_path)
                    .await
                    .map_err(|e| format!("Failed to read file for hash check: {}", e))?;
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let computed = format!("{:x}", hasher.finalize());
                if !computed.eq_ignore_ascii_case(&expected) {
                    return Err(format!(
                        "GitHub asset SHA-256 mismatch: expected {}, got {}",
                        expected, computed
                    ));
                }
            }
        }

        Ok(ctx.final_path)
    }
}
