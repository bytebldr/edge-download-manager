use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive_wrapper::scout::Scout;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, EndpointCandidate, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use crate::adaptive::TelemetryCallback;
use crate::packs::huggingface::HuggingFacePack;
use async_trait::async_trait;
use reqwest::Client;
use std::path::PathBuf;

pub struct HuggingFaceAdapter;

#[async_trait]
impl AdaptiveWrapperAdapter for HuggingFaceAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        client: &Client,
    ) -> Result<PreFlightPlan, String> {
        let mut download_url = task.url.clone();
        let mut expected_hash = None;

        // 1. Probe for Git-LFS pointer stub in the first 1024 bytes
        if let Ok(resp) = client
            .get(&task.url)
            .header("Range", "bytes=0-1024")
            .send()
            .await
        {
            if let Ok(text) = resp.text().await {
                if let Some(pointer) = HuggingFacePack::parse_git_lfs_pointer(&text) {
                    expected_hash = Some(("sha256".to_string(), pointer.oid));
                    if !download_url.contains("/resolve/") && download_url.contains("/raw/") {
                        download_url = download_url.replace("/raw/", "/resolve/");
                    }
                }
            }
        }

        // 2. Determine if user token is present (security isolation: never leak tokens to third-party mirrors)
        let has_auth = task.cookies.is_some() || task.url.contains("token=");

        let winning_url = if has_auth {
            // Strictly stay on official origin when authenticated
            download_url.clone()
        } else if let Some(target) = HuggingFacePack::parse_target(&download_url) {
            let candidates = vec![
                EndpointCandidate {
                    name: "origin".to_string(),
                    url: target.direct_download_url.clone(),
                    priority: 1,
                },
                EndpointCandidate {
                    name: "hf-mirror".to_string(),
                    url: target.mirror_url.clone(),
                    priority: 2,
                },
            ];
            Scout::race_endpoints(&candidates, &target.direct_download_url, client).await
        } else {
            download_url.clone()
        };

        let mut accelerated_task = task.clone();
        accelerated_task.url = winning_url;

        Ok(PreFlightPlan {
            execution: PreFlightExecution::SingleStream(accelerated_task),
            chunk_strategy: ChunkingStrategy::StandardAdaptive {
                initial_chunk_size: 8 * 1024 * 1024,
                max_connections: task.config.max_connections.clamp(2, 32) as usize,
            },
            expected_size: None,
            expected_hash,
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
            return Err("Hugging Face model weight file does not exist".to_string());
        }

        // Validate Git-LFS SHA-256 hash if present
        if let Some((algo, expected)) = ctx.expected_hash {
            if algo.eq_ignore_ascii_case("sha256") {
                let bytes = tokio::fs::read(&ctx.final_path)
                    .await
                    .map_err(|e| format!("Failed to read model file for hash check: {}", e))?;
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let computed = format!("{:x}", hasher.finalize());
                if !computed.eq_ignore_ascii_case(&expected) {
                    return Err(format!(
                        "Git-LFS SHA-256 integrity mismatch: expected {}, got {}",
                        expected, computed
                    ));
                }
            }
        }

        Ok(ctx.final_path)
    }
}
