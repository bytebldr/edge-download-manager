use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive_wrapper::process_guard::FfmpegSupervisor;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, DualStreamBundle, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use crate::adaptive::TelemetryCallback;
use crate::packs::ytdlp::YtDlpPack;
use async_trait::async_trait;
use reqwest::Client;
use std::path::PathBuf;

pub struct YtDlpAdapter;

#[async_trait]
impl AdaptiveWrapperAdapter for YtDlpAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        _client: &Client,
    ) -> Result<PreFlightPlan, String> {
        // Attempt quick metadata extraction to locate direct stream URLs
        if let Ok(meta) = YtDlpPack::extract_info(&task.url).await {
            if let (Some(video_url), Some(audio_url)) = (meta.best_video_url, meta.best_audio_url)
            {
                let mut v_task = task.clone();
                v_task.url = video_url;
                v_task.filename = format!("{}.video.tmp", task.id);

                let mut a_task = task.clone();
                a_task.url = audio_url;
                a_task.filename = format!("{}.audio.tmp", task.id);

                return Ok(PreFlightPlan {
                    execution: PreFlightExecution::DualParallelStream(DualStreamBundle {
                        video_task: v_task,
                        audio_task: a_task,
                        output_container: "mp4".to_string(),
                    }),
                    chunk_strategy: ChunkingStrategy::StandardAdaptive {
                        initial_chunk_size: 4 * 1024 * 1024,
                        max_connections: 16,
                    },
                    expected_size: None,
                    expected_hash: None,
                    suggested_filename: Some(meta.title),
                    fallback_urls: Vec::new(),
                });
            }
        }

        // Direct protocol fallback if direct stream extraction is unavailable
        Ok(PreFlightPlan {
            execution: PreFlightExecution::SingleStream(task.clone()),
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
        telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        // If dual streams exist in temp_paths, transmux them into the final container
        if ctx.temp_paths.len() >= 2 {
            let video_path = &ctx.temp_paths[0];
            let audio_path = &ctx.temp_paths[1];

            if video_path.exists() && audio_path.exists() {
                if let Some(ffmpeg_bin) = YtDlpPack::locate_ffmpeg() {
                    let out_path_str = ctx.final_path.to_string_lossy().to_string();
                    let args = vec![
                        "-y".to_string(),
                        "-i".to_string(),
                        video_path.to_string_lossy().to_string(),
                        "-i".to_string(),
                        audio_path.to_string_lossy().to_string(),
                        "-c:v".to_string(),
                        "copy".to_string(),
                        "-c:a".to_string(),
                        "copy".to_string(),
                        "-progress".to_string(),
                        "pipe:1".to_string(),
                        out_path_str,
                    ];

                    let id_str = ctx.final_path.to_string_lossy();
                    FfmpegSupervisor::transmux_with_telemetry(
                        &ffmpeg_bin,
                        &args,
                        &id_str,
                        telemetry_cb,
                    )
                    .await?;

                    // Clean temporary stream parts
                    let _ = tokio::fs::remove_file(video_path).await;
                    let _ = tokio::fs::remove_file(audio_path).await;

                    return Ok(ctx.final_path);
                }
            }
        }

        if !ctx.final_path.exists() {
            return Err("Media destination file does not exist".to_string());
        }
        Ok(ctx.final_path)
    }
}
