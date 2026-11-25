//! Bounded & Cancellable Live Stream Recorder (.m3u8 / .mpd)
//! Streams live segments to disk and optionally remuxes with FFmpeg when available.

use super::mpd::{parse_mpd, resolve_url, DashSegmentTemplate};
use crate::packs::{configure_hidden_command, FfmpegPack, M3u8Pack};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveRecordConfig {
    pub manifest_url: String,
    pub output_path: PathBuf,
    pub max_duration_secs: Option<u64>,
    pub max_segments: Option<usize>,
    pub poll_interval_ms: u64,
    pub remux_with_ffmpeg: bool,
}

impl LiveRecordConfig {
    pub fn new(manifest_url: impl Into<String>, output_path: impl Into<PathBuf>) -> Self {
        Self {
            manifest_url: manifest_url.into(),
            output_path: output_path.into(),
            max_duration_secs: None,
            max_segments: None,
            poll_interval_ms: 2000,
            remux_with_ffmpeg: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveRecordProgress {
    pub segments_recorded: usize,
    pub bytes_written: u64,
    pub elapsed_secs: f64,
    pub output_path: PathBuf,
    pub remuxed: bool,
}

#[derive(Debug, Clone)]
pub struct LiveRecordCancelHandle {
    cancelled: Arc<AtomicBool>,
}

impl LiveRecordCancelHandle {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub struct DashLiveRecorder {
    config: LiveRecordConfig,
    cancelled: Arc<AtomicBool>,
    client: reqwest::Client,
}

impl DashLiveRecorder {
    pub fn new(config: LiveRecordConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();

        Self {
            config,
            cancelled: Arc::new(AtomicBool::new(false)),
            client,
        }
    }

    pub fn cancel_handle(&self) -> LiveRecordCancelHandle {
        LiveRecordCancelHandle {
            cancelled: Arc::clone(&self.cancelled),
        }
    }

    fn is_stopped(&self, start: Instant, segments: usize) -> bool {
        if self.cancelled.load(Ordering::SeqCst) {
            return true;
        }
        if let Some(max_secs) = self.config.max_duration_secs {
            if start.elapsed().as_secs() >= max_secs {
                return true;
            }
        }
        if let Some(max_segs) = self.config.max_segments {
            if segments >= max_segs {
                return true;
            }
        }
        false
    }

    /// Record live `.m3u8` or `.mpd` stream until ended, bounded limit reached, or cancelled.
    pub async fn record(&self) -> Result<LiveRecordProgress, String> {
        if let Some(parent) = self.config.output_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }

        let raw_path = self.config.output_path.with_extension("raw.part");
        let start = Instant::now();

        let first_body = self.fetch_text(&self.config.manifest_url).await?;
        let is_mpd = first_body.contains("<MPD")
            || self
                .config
                .manifest_url
                .split('?')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
                .ends_with(".mpd");

        let (segments_recorded, bytes_written) = if is_mpd {
            self.record_dash(&first_body, &raw_path, start).await?
        } else {
            self.record_hls(&first_body, &raw_path, start).await?
        };

        let remuxed = self.finalize_output(&raw_path).await?;

        Ok(LiveRecordProgress {
            segments_recorded,
            bytes_written,
            elapsed_secs: start.elapsed().as_secs_f64(),
            output_path: self.config.output_path.clone(),
            remuxed,
        })
    }

    async fn record_hls(
        &self,
        initial_body: &str,
        raw_path: &Path,
        start: Instant,
    ) -> Result<(usize, u64), String> {
        let mut playlist_url = self.config.manifest_url.clone();
        let mut current_body = initial_body.to_string();

        // If master playlist, select highest-bandwidth variant
        if current_body.contains("#EXT-X-STREAM-INF:") {
            let mut variants = M3u8Pack::parse_master_playlist(&current_body, &playlist_url)?;
            variants.sort_by(|a, b| b.bandwidth.cmp(&a.bandwidth));
            let best = variants
                .into_iter()
                .next()
                .ok_or_else(|| "HLS master playlist contained no variants".to_string())?;
            playlist_url = best.url;
            current_body = self.fetch_text(&playlist_url).await?;
        }

        let mut out_file = File::create(raw_path)
            .await
            .map_err(|e| format!("Failed to create recording buffer: {}", e))?;

        let mut seen_urls = HashSet::new();
        let mut init_written = false;
        let mut segments_recorded = 0usize;
        let mut bytes_written = 0u64;

        loop {
            let playlist = M3u8Pack::parse_media_playlist(&current_body, &playlist_url)?;

            if !init_written {
                if let Some(ref init_url) = playlist.init_segment_url {
                    let bytes = self.fetch_bytes(init_url).await?;
                    out_file
                        .write_all(&bytes)
                        .await
                        .map_err(|e| format!("Failed to write HLS init segment: {}", e))?;
                    bytes_written += bytes.len() as u64;
                }
                init_written = true;
            }

            for seg in &playlist.segments {
                if self.is_stopped(start, segments_recorded) {
                    break;
                }
                if !seen_urls.insert(seg.url.clone()) {
                    continue;
                }
                let bytes = self.fetch_bytes(&seg.url).await?;
                out_file
                    .write_all(&bytes)
                    .await
                    .map_err(|e| format!("Failed to write HLS segment: {}", e))?;
                bytes_written += bytes.len() as u64;
                segments_recorded += 1;
            }

            if playlist.is_endlist || self.is_stopped(start, segments_recorded) {
                break;
            }

            tokio::time::sleep(Duration::from_millis(self.config.poll_interval_ms)).await;
            current_body = self.fetch_text(&playlist_url).await?;
        }

        out_file
            .flush()
            .await
            .map_err(|e| format!("Failed to flush HLS recording: {}", e))?;

        Ok((segments_recorded, bytes_written))
    }

    async fn record_dash(
        &self,
        initial_body: &str,
        raw_path: &Path,
        start: Instant,
    ) -> Result<(usize, u64), String> {
        let manifest = parse_mpd(initial_body, &self.config.manifest_url)?;
        let track = manifest
            .video_tracks
            .first()
            .or_else(|| manifest.audio_tracks.first())
            .ok_or_else(|| "DASH manifest contains no playable tracks".to_string())?;

        let tmpl = track
            .segment_template
            .as_ref()
            .ok_or_else(|| "DASH live recording requires <SegmentTemplate>".to_string())?;

        let base = track
            .base_url
            .as_deref()
            .unwrap_or(&self.config.manifest_url);

        let mut out_file = File::create(raw_path)
            .await
            .map_err(|e| format!("Failed to create DASH recording buffer: {}", e))?;

        let mut segments_recorded = 0usize;
        let mut bytes_written = 0u64;

        if let Some(ref init_pat) = tmpl.initialization {
            let resolved =
                DashSegmentTemplate::resolve_pattern(init_pat, &track.id, None, track.bandwidth, None);
            let init_url = resolve_url(base, &resolved);
            let bytes = self.fetch_bytes(&init_url).await?;
            out_file
                .write_all(&bytes)
                .await
                .map_err(|e| format!("Failed to write DASH init segment: {}", e))?;
            bytes_written += bytes.len() as u64;
        }

        let media_pat = tmpl
            .media
            .as_deref()
            .ok_or_else(|| "DASH <SegmentTemplate> missing media attribute".to_string())?;

        let seg_duration_ms = tmpl
            .duration
            .map(|d| (d * 1000) / tmpl.timescale.max(1))
            .unwrap_or(self.config.poll_interval_ms)
            .max(250);

        let mut seg_num = tmpl.start_number;
        let mut consecutive_404 = 0u32;

        while !self.is_stopped(start, segments_recorded) {
            let resolved = DashSegmentTemplate::resolve_pattern(
                media_pat,
                &track.id,
                Some(seg_num),
                track.bandwidth,
                None,
            );
            let seg_url = resolve_url(base, &resolved);

            match self.fetch_bytes(&seg_url).await {
                Ok(bytes) => {
                    consecutive_404 = 0;
                    out_file
                        .write_all(&bytes)
                        .await
                        .map_err(|e| format!("Failed to write DASH segment: {}", e))?;
                    bytes_written += bytes.len() as u64;
                    segments_recorded += 1;
                    seg_num += 1;
                }
                Err(_) if manifest.is_live && consecutive_404 < 5 => {
                    consecutive_404 += 1;
                    tokio::time::sleep(Duration::from_millis(seg_duration_ms)).await;
                }
                Err(e) => {
                    if !manifest.is_live && segments_recorded > 0 {
                        break;
                    }
                    return Err(e);
                }
            }
        }

        out_file
            .flush()
            .await
            .map_err(|e| format!("Failed to flush DASH recording: {}", e))?;

        Ok((segments_recorded, bytes_written))
    }

    async fn finalize_output(&self, raw_path: &Path) -> Result<bool, String> {
        if self.config.remux_with_ffmpeg {
            if let Some(ffmpeg_bin) = FfmpegPack::locate_binary() {
                let status = configure_hidden_command(
                    &ffmpeg_bin,
                    [
                        "-y",
                        "-i",
                        raw_path.to_string_lossy().as_ref(),
                        "-c",
                        "copy",
                        self.config.output_path.to_string_lossy().as_ref(),
                    ],
                )
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;

                if matches!(status, Ok(s) if s.success()) {
                    let _ = tokio::fs::remove_file(raw_path).await;
                    return Ok(true);
                }
            }
        }

        tokio::fs::rename(raw_path, &self.config.output_path)
            .await
            .map_err(|e| format!("Failed to finalize recorded output: {}", e))?;
        Ok(false)
    }

    async fn fetch_text(&self, url: &str) -> Result<String, String> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch {}: {}", url, e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} fetching {}", resp.status(), url));
        }
        resp.text()
            .await
            .map_err(|e| format!("Failed to read response from {}: {}", url, e))
    }

    async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch segment {}: {}", url, e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} fetching segment {}", resp.status(), url));
        }
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("Failed to read segment bytes from {}: {}", url, e))
    }
}
