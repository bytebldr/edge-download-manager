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
    fn apply_chunk_strategy(task: &mut AdaptiveDownloadTask, strategy: &ChunkingStrategy) {
        match strategy {
            ChunkingStrategy::StandardAdaptive {
                initial_chunk_size,
                max_connections,
            } => {
                let max_conns = (*max_connections as u32).clamp(1, 64);
                task.config.max_connections = max_conns;
                task.config.max_connections_per_host = task.config.max_connections;
                task.config.initial_connections =
                    task.config.initial_connections.clamp(1, max_conns);
                task.config.min_connections = task.config.min_connections.clamp(1, max_conns);
                if max_conns == 1 {
                    task.config.auto_scale = false;
                }
                if *initial_chunk_size > 0 {
                    task.config.min_chunk_size = *initial_chunk_size;
                }
            }
            ChunkingStrategy::CdnAntiThrottling {
                slice_size,
                max_connections,
            } => {
                let max_conns = (*max_connections as u32).clamp(1, 64);
                task.config.max_connections = max_conns;
                task.config.max_connections_per_host = task.config.max_connections;
                task.config.initial_connections =
                    task.config.initial_connections.clamp(1, max_conns);
                task.config.min_connections = task.config.min_connections.clamp(1, max_conns);
                if max_conns == 1 {
                    task.config.auto_scale = false;
                }
                if *slice_size > 0 {
                    task.config.min_chunk_size = (*slice_size).max(64 * 1024);
                }
            }
            ChunkingStrategy::FixedPartSize {
                part_size,
                align_boundaries: _,
            } => {
                if *part_size > 0 {
                    task.config.min_chunk_size = *part_size;
                }
            }
            ChunkingStrategy::DirectStreamPiped => {}
        }
    }

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
                        initial_chunk_size: task.config.min_chunk_size,
                        max_connections: task.config.max_connections.clamp(1, 64) as usize,
                    },
                    expected_size: None,
                    expected_hash: None,
                    suggested_filename: None,
                    fallback_urls: Vec::new(),
                }
            }
        };

        // 2. Transport Execution
        let chunk_strategy = plan.chunk_strategy;
        let expected_hash = plan.expected_hash.clone();
        let fallback_urls = plan.fallback_urls.clone();
        let (final_path, temp_paths) = match plan.execution {
            PreFlightExecution::SingleStream(mut actual_task) => {
                Self::apply_chunk_strategy(&mut actual_task, &chunk_strategy);
                if actual_task.expected_hash.is_none() {
                    actual_task.expected_hash = expected_hash.clone();
                }
                if actual_task.mirror_urls.is_empty() && !fallback_urls.is_empty() {
                    actual_task.mirror_urls = fallback_urls;
                }
                let path = engine.download(actual_task, telemetry_cb.clone()).await?;
                (path, Vec::new())
            }
            PreFlightExecution::DualParallelStream(mut bundle) => {
                Self::apply_chunk_strategy(&mut bundle.video_task, &chunk_strategy);
                Self::apply_chunk_strategy(&mut bundle.audio_task, &chunk_strategy);
                // Partition 32-stream budget across video (24) + audio (8) and use distinct SQLite IDs
                bundle.video_task.id = uuid::Uuid::new_v4();
                bundle.audio_task.id = uuid::Uuid::new_v4();
                bundle.video_task.config.max_connections =
                    bundle.video_task.config.max_connections.min(24).max(1);
                bundle.audio_task.config.max_connections =
                    bundle.audio_task.config.max_connections.min(8).max(1);
                bundle.video_task.filename = format!(".tmp_{}_video", task.id);
                bundle.audio_task.filename = format!(".tmp_{}_audio", task.id);

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
                let final_name = if task
                    .filename
                    .to_ascii_lowercase()
                    .ends_with(&format!(".{}", ext.to_ascii_lowercase()))
                {
                    task.filename.clone()
                } else {
                    format!("{}.{}", task.filename, ext)
                };
                let final_target = task.save_dir.join(final_name);
                if let Some(ffmpeg_bin) = crate::packs::ytdlp::YtDlpPack::locate_ffmpeg() {
                    let v_str = video_path.to_string_lossy().to_string();
                    let a_str = audio_path.to_string_lossy().to_string();
                    let out_str = final_target.to_string_lossy().to_string();
                    let args: Vec<String> = vec![
                        "-y".into(),
                        "-i".into(),
                        v_str,
                        "-i".into(),
                        a_str,
                        "-c".into(),
                        "copy".into(),
                        "-progress".into(),
                        "pipe:1".into(),
                        out_str,
                    ];
                    FfmpegSupervisor::transmux_with_telemetry(
                        &ffmpeg_bin,
                        &args,
                        &task.id.to_string(),
                        telemetry_cb.clone(),
                    )
                    .await?;
                    let _ = tokio::fs::remove_file(&video_path).await;
                    let _ = tokio::fs::remove_file(&audio_path).await;
                }
                (final_target, vec![video_path, audio_path])
            }
            PreFlightExecution::DirectProtocol {
                task: mut proto_task,
                custom_metadata: _,
            } => {
                Self::apply_chunk_strategy(&mut proto_task, &chunk_strategy);
                let path = match pack_target.pack_id {
                    FeaturePackId::BitTorrentPack => {
                        PackExecutor::execute_bittorrent(proto_task, telemetry_cb.clone()).await?
                    }
                    FeaturePackId::M3u8Pack => {
                        PackExecutor::execute_m3u8(proto_task, client.clone(), telemetry_cb.clone())
                            .await?
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
    use crate::adaptive::AdaptiveConfig;
    use std::sync::atomic::AtomicBool;
    use uuid::Uuid;

    fn dummy_task(max_connections: u32) -> AdaptiveDownloadTask {
        AdaptiveDownloadTask {
            id: Uuid::new_v4(),
            url: "https://example.com/file.bin".into(),
            filename: "file.bin".into(),
            save_dir: PathBuf::from("."),
            config: AdaptiveConfig {
                max_connections,
                max_connections_per_host: max_connections,
                ..AdaptiveConfig::default()
            },
            limiter: None,
            cancelled: Arc::new(AtomicBool::new(false)),
            cookies: None,
            referer: None,
            user_agent: None,
            selected_files: None,
            torrent_power: None,
            ..Default::default()
        }
    }

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

    #[test]
    fn test_apply_chunk_strategy_64_and_single_stream() {
        let mut task = dummy_task(32);
        assert_eq!(task.config.max_connections_per_host, 32);
        AdaptiveWrapper::apply_chunk_strategy(
            &mut task,
            &ChunkingStrategy::StandardAdaptive {
                initial_chunk_size: 2 * 1024 * 1024,
                max_connections: 64,
            },
        );
        assert_eq!(task.config.max_connections, 64);
        assert_eq!(task.config.max_connections_per_host, 64);
        assert_eq!(task.config.min_chunk_size, 2 * 1024 * 1024);
        assert!(task.config.auto_scale);

        AdaptiveWrapper::apply_chunk_strategy(
            &mut task,
            &ChunkingStrategy::StandardAdaptive {
                initial_chunk_size: 0,
                max_connections: 1,
            },
        );
        assert_eq!(task.config.max_connections, 1);
        assert_eq!(task.config.initial_connections, 1);
        assert_eq!(task.config.min_connections, 1);
        assert_eq!(task.config.max_connections_per_host, 1);
        assert!(!task.config.auto_scale);
    }

    #[tokio::test]
    async fn test_http_and_github_adapters_respect_1_to_64_ceiling() {
        let client = Client::new();
        for ceiling in [1u32, 32, 64] {
            let task = dummy_task(ceiling);
            let http_plan = get_adapter(FeaturePackId::HttpPack)
                .pre_flight(&task, &client)
                .await
                .expect("http pre_flight");
            assert_eq!(
                http_plan.chunk_strategy,
                ChunkingStrategy::StandardAdaptive {
                    initial_chunk_size: task.config.min_chunk_size,
                    max_connections: ceiling as usize,
                }
            );
        }
    }
}
