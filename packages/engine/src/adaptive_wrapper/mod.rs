//! Adaptive Wrapper Subsystem
//! Intelligent Pre-Flight Scouting, In-Flight Supervision, and Post-Flight Synthesis across all 9 protocol packs.

pub mod adapters;
pub mod process_guard;
pub mod scout;
pub mod traits;
pub mod types;

pub use adapters::get_adapter;
pub use process_guard::{FfmpegSupervisor, ProcessGuard};
pub use scout::Scout;
pub use traits::AdaptiveWrapperAdapter;
pub use types::{
    ChunkingStrategy, DualStreamBundle, EndpointCandidate, PostFlightContext, PreFlightExecution,
    PreFlightPlan,
};

use crate::adaptive::{AdaptiveDownloadTask, AdaptiveEngine, TelemetryCallback};
use crate::packs::{FeaturePackId, FeaturePackManager, PackExecutor};
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;

pub struct AdaptiveWrapper;

impl AdaptiveWrapper {
    /// Master execution pipeline unifying pre-flight scout, engine execution, and post-flight synthesis
    pub async fn execute(
        task: AdaptiveDownloadTask,
        engine: Arc<AdaptiveEngine>,
        client: Client,
        telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        let pack_target = FeaturePackManager::classify_target(&task.url);
        let adapter = get_adapter(pack_target.pack_id);

        // 1. Pre-Flight Scouting (Mirror racing, token isolation, stream separation)
        let plan = match adapter.pre_flight(&task, &client).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "Adaptive wrapper pre-flight warning: {}. Proceeding with default execution.",
                    e
                );
                // Fallback plan
                PreFlightPlan {
                    execution: PreFlightExecution::SingleStream(task.clone()),
                    chunk_strategy: ChunkingStrategy::StandardAdaptive {
                        initial_chunk_size: 4 * 1024 * 1024,
                        max_connections: task.config.max_connections.clamp(1, 32) as usize,
                    },
                    expected_size: None,
                    expected_hash: None,
                    suggested_filename: None,
                    fallback_urls: Vec::new(),
                }
            }
        };

        // 2. Transport Execution
        let (final_path, temp_paths) = match plan.execution {
            PreFlightExecution::SingleStream(actual_task) => {
                let path = engine.download(actual_task, telemetry_cb.clone()).await?;
                (path, Vec::new())
            }
            PreFlightExecution::DualParallelStream(bundle) => {
                let v_engine = Arc::clone(&engine);
                let a_engine = Arc::clone(&engine);
                let v_task = bundle.video_task;
                let a_task = bundle.audio_task;
                let cb_clone = telemetry_cb.clone();

                let (v_res, a_res) = tokio::join!(
                    tokio::spawn(async move { v_engine.download(v_task, cb_clone).await }),
                    tokio::spawn(async move { a_engine.download(a_task, None).await })
                );

                let video_path = v_res
                    .map_err(|e| format!("Video task join error: {}", e))?
                    .map_err(|e| format!("Video download failed: {}", e))?;
                let audio_path = a_res
                    .map_err(|e| format!("Audio task join error: {}", e))?
                    .map_err(|e| format!("Audio download failed: {}", e))?;

                let ext = bundle.output_container;
                let final_target = task.save_dir.join(format!("{}.{}", task.filename, ext));
                (final_target, vec![video_path, audio_path])
            }
            PreFlightExecution::DirectProtocol { task: proto_task, custom_metadata: _ } => {
                let path = match pack_target.pack_id {
                    FeaturePackId::BitTorrentPack => {
                        PackExecutor::execute_bittorrent(proto_task, telemetry_cb.clone()).await?
                    }
                    FeaturePackId::M3u8Pack => {
                        PackExecutor::execute_m3u8(proto_task, client.clone(), telemetry_cb.clone()).await?
                    }
                    FeaturePackId::YtDlpPack => {
                        PackExecutor::execute_ytdlp(
                            proto_task,
                            Arc::clone(&engine),
                            client.clone(),
                            telemetry_cb.clone(),
                        )
                        .await?
                    }
                    FeaturePackId::FtpPack => {
                        PackExecutor::execute_ftp(proto_task, telemetry_cb.clone()).await?
                    }
                    FeaturePackId::Ed2kPack => {
                        PackExecutor::execute_ed2k(proto_task, telemetry_cb.clone()).await?
                    }
                    _ => engine.download(proto_task, telemetry_cb.clone()).await?,
                };
                (path, Vec::new())
            }
        };

        // 3. Post-Flight Synthesis & Integrity Verification
        let post_ctx = PostFlightContext {
            final_path,
            temp_paths,
            expected_hash: plan.expected_hash,
        };

        adapter.post_flight(post_ctx, telemetry_cb).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_9_adapters_resolvable() {
        let packs = [
            FeaturePackId::HttpPack,
            FeaturePackId::BitTorrentPack,
            FeaturePackId::FtpPack,
            FeaturePackId::Ed2kPack,
            FeaturePackId::M3u8Pack,
            FeaturePackId::FfmpegPack,
            FeaturePackId::YtDlpPack,
            FeaturePackId::GithubPack,
            FeaturePackId::HuggingFacePack,
        ];

        for pack in packs {
            let _adapter = get_adapter(pack);
        }
    }
}

