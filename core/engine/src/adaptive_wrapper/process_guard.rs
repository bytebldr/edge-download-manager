use crate::adaptive::{
    AccelerationMode, AdaptiveTelemetry, TelemetryCallback, WorkerStatus, WorkerTelemetry,
};
use chrono::Utc;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

pub struct ProcessGuard {
    child: Option<Child>,
}

impl ProcessGuard {
    pub fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    pub fn take(&mut self) -> Option<Child> {
        self.child.take()
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}

pub struct FfmpegSupervisor;

impl FfmpegSupervisor {
    /// Transmuxes streams while parsing progress and concurrently draining stderr to avoid pipe deadlocks
    pub async fn transmux_with_telemetry(
        ffmpeg_bin: &Path,
        args: &[String],
        download_id: &str,
        telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<(), String> {
        let mut cmd = Command::new(ffmpeg_bin);
        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW = 0x08000000
            cmd.creation_flags(0x08000000);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn FFmpeg: {}", e))?;

        let stdout = child.stdout.take().ok_or("Failed to open FFmpeg stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to open FFmpeg stderr")?;

        let mut guard = ProcessGuard::new(child);

        // Drain stderr concurrently to prevent pipe buffer deadlock
        let stderr_drain = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            let mut last_err = String::new();
            while let Ok(Some(line)) = reader.next_line().await {
                if !line.trim().is_empty() {
                    last_err = line;
                }
            }
            last_err
        });

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut total_microseconds = 0u64;

        while let Ok(Some(line)) = stdout_reader.next_line().await {
            let line = line.trim();
            if line.starts_with("out_time_us=") {
                let us_str = line.trim_start_matches("out_time_us=");
                if let Ok(us) = us_str.parse::<u64>() {
                    total_microseconds = us;
                    let seconds = us / 1_000_000;
                    if let Some(ref cb) = telemetry_cb {
                        cb(AdaptiveTelemetry {
                            download_id: download_id.to_string(),
                            total_bytes: None,
                            downloaded_bytes: seconds * 1024 * 1024, // Synthetic visualization bytes
                            progress_pct: 99.0,                      // Post-flight synthesis phase
                            instant_throughput_bps: 10_000_000,
                            rolling_throughput_bps: 10_000_000,
                            active_connections: 1,
                            target_connections: 1,
                            live_workers: 1,
                            max_allowed_connections: 1,
                            mode: AccelerationMode::Stable,
                            eta_seconds: Some(1),
                            decision_reason: format!("Transmuxing: {}s processed", seconds),
                            phase: Some(format!("Transmuxing ({:.1}s)", seconds as f64)),
                            workers: vec![WorkerTelemetry {
                                idx: 0,
                                start: 0,
                                end: 0,
                                pos: 0,
                                bytes_downloaded: 0,
                                speed_bps: 0,
                                status: WorkerStatus::Completed,
                                retry_count: 0,
                            }],
                            updated_at: Utc::now(),
                        });
                    }
                }
            }
        }

        let mut active_child = guard.take().ok_or("FFmpeg process guard empty")?;
        let status = active_child
            .wait()
            .await
            .map_err(|e| format!("FFmpeg wait error: {}", e))?;

        let last_stderr = stderr_drain.await.unwrap_or_default();

        if !status.success() && total_microseconds == 0 {
            return Err(format!("FFmpeg failed: {}", last_stderr));
        }

        Ok(())
    }
}
