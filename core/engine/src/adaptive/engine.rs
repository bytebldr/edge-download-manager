use chrono::Utc;
use edm_ipc::DownloadState;
use edm_store::{DownloadRecord, SegmentRecord, Store};
use reqwest::header::{ACCEPT_ENCODING, COOKIE, LOCATION, RANGE, REFERER, USER_AGENT};
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

use super::client_registry::{ClientRegistry, ProxyKey};
use super::controller::AdaptiveController;
use super::host_gate::HostRegistry;
use super::pipeline_writer::PipelinedWriterHandle;
use super::range_set::RangeSet;
use super::scheduler::WorkScheduler;
use super::types::{
    AccelerationMode, AdaptiveConfig, AdaptiveDecision, AdaptiveTelemetry, TransportProtocol,
};
use super::worker::{
    epoch_ms_now, normalize_etag, origin_key, protocol_from_version, AdaptiveWorkerEvent,
};
use crate::limiter::TokenBucketLimiter;
use crate::planner::Planner;
use crate::probe::Probe;

pub type TelemetryCallback = Arc<dyn Fn(AdaptiveTelemetry) + Send + Sync>;

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36 Edg/135.0.0.0";
const MAX_REDIRECT_HOPS: usize = 15;
const TAIL_THRESHOLD_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct AdaptiveDownloadTask {
    pub id: Uuid,
    pub url: String,
    pub filename: String,
    pub save_dir: PathBuf,
    pub config: AdaptiveConfig,
    pub limiter: Option<TokenBucketLimiter>,
    pub cancelled: Arc<AtomicBool>,
    pub cookies: Option<String>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
    pub selected_files: Option<Vec<usize>>,
    pub torrent_power: Option<crate::adaptive::types::TorrentPowerConfig>,
    pub proxy_url: Option<String>,
    pub expected_hash: Option<(String, String)>,
    pub mirror_urls: Vec<String>,
}

pub struct AdaptiveEngine {
    #[allow(dead_code)]
    client: Client,
    store: Store,
    host_semaphores: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    client_registry: ClientRegistry,
    host_registry: HostRegistry,
}

impl AdaptiveEngine {
    pub fn try_new(store: Store) -> Result<Self, String> {
        let client = Client::builder()
            .http2_adaptive_window(true)
            .tcp_nodelay(true)
            .pool_max_idle_per_host(64)
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

        Ok(Self {
            client,
            store,
            host_semaphores: Arc::new(Mutex::new(HashMap::new())),
            client_registry: ClientRegistry::default(),
            host_registry: HostRegistry::default(),
        })
    }

    pub fn new(store: Store) -> Self {
        Self::try_new(store).expect("AdaptiveEngine HTTP client initialization failed")
    }

    fn host_semaphore_for_url(&self, url_str: &str, max_per_host: u32) -> Arc<Semaphore> {
        let key = Url::parse(url_str)
            .ok()
            .map(|u| {
                format!(
                    "{}:{}",
                    u.host_str().unwrap_or("unknown").to_ascii_lowercase(),
                    u.port_or_known_default().unwrap_or(80)
                )
            })
            .unwrap_or_else(|| "unknown".to_string());

        let mut map = self
            .host_semaphores
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        map.entry(key)
            .or_insert_with(|| Arc::new(Semaphore::new(max_per_host.max(1) as usize)))
            .clone()
    }

    /// Resolves HTTP redirects manually with `Policy::none()`, stripping `Cookie`/`Referer`
    /// on cross-origin hops and detecting the negotiated HTTP transport protocol.
    async fn resolve_safe_redirects_with_client(
        &self,
        client: &Client,
        url_str: &str,
        cookies: Option<&str>,
        referer: Option<&str>,
        user_agent: Option<&str>,
        request_timeout: Duration,
    ) -> (String, Option<String>, Option<String>, TransportProtocol) {
        let Ok(initial_url) = Url::parse(url_str) else {
            return (
                url_str.to_string(),
                cookies.map(str::to_string),
                referer.map(str::to_string),
                TransportProtocol::Http1,
            );
        };
        let initial_origin = origin_key(&initial_url);
        let mut current_url = initial_url;
        let mut detected_proto = TransportProtocol::Http1;
        let mut use_get_range = false;

        for _ in 0..MAX_REDIRECT_HOPS {
            let same_origin = origin_key(&current_url) == initial_origin;
            let mut req = if use_get_range {
                client.get(current_url.as_str()).header(RANGE, "bytes=0-0")
            } else {
                client.head(current_url.as_str())
            };
            req = req.header(ACCEPT_ENCODING, "identity");

            if same_origin {
                if let Some(c) = cookies {
                    req = req.header(COOKIE, c);
                }
                if let Some(r) = referer {
                    req = req.header(REFERER, r);
                }
            }
            if let Some(ua) = user_agent {
                req = req.header(USER_AGENT, ua);
            }

            let Ok(Ok(resp)) = tokio::time::timeout(request_timeout, req.send()).await else {
                break;
            };

            detected_proto = protocol_from_version(resp.version());

            if resp.status().is_redirection() {
                if let Some(loc) = resp.headers().get(LOCATION).and_then(|v| v.to_str().ok()) {
                    if let Ok(next_url) = current_url.join(loc) {
                        if next_url.scheme() == "http" || next_url.scheme() == "https" {
                            current_url = next_url;
                            continue;
                        }
                    }
                }
                break;
            }

            if !use_get_range && matches!(resp.status().as_u16(), 400 | 403 | 405 | 501) {
                use_get_range = true;
                continue;
            }

            break;
        }

        let same_final_origin = origin_key(&current_url) == initial_origin;
        (
            current_url.to_string(),
            if same_final_origin {
                cookies.map(str::to_string)
            } else {
                None
            },
            if same_final_origin {
                referer.map(str::to_string)
            } else {
                None
            },
            detected_proto,
        )
    }

    pub async fn download(
        &self,
        task: AdaptiveDownloadTask,
        telemetry_callback: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        info!("AdaptiveEngine: Probing {}", task.url);

        let proxy_key = task
            .proxy_url
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(ProxyKey::new);
        let client_pair = self
            .client_registry
            .get_or_build(proxy_key.as_ref(), task.config.request_timeout.as_secs())?;
        let probe_client = client_pair.probe_client();

        let (resolved_url, effective_cookies, effective_referer, initial_proto) = self
            .resolve_safe_redirects_with_client(
                &probe_client,
                &task.url,
                task.cookies.as_deref(),
                task.referer.as_deref(),
                task.user_agent.as_deref(),
                task.config.request_timeout,
            )
            .await;

        let probe = Probe::probe_url_with_headers(
            &probe_client,
            &resolved_url,
            effective_cookies.as_deref(),
            effective_referer.as_deref(),
            task.user_agent.as_deref(),
        )
        .await
        .map_err(|e| format!("Probe failed: {}", e))?;

        if probe.status_code >= 400 {
            return Err(format!("Server returned HTTP {} error", probe.status_code));
        }

        if is_unexpected_html_response(&task.url, &task.filename, probe.content_type.as_deref()) {
            return Err(format!(
                "Server returned an HTML error/challenge page ({}) instead of the expected file",
                probe.content_type.as_deref().unwrap_or("text/html")
            ));
        }

        let total_size = probe.size_total;
        let is_resumable = probe.resumable && total_size.is_some() && total_size.unwrap() > 0;

        let redacted = redact_headers(&probe.response_headers);
        info!(
            "Task {}: Probed URL '{}' -> Final URL: '{}' | Proto: {} | Status: {} | Range Support: {} | Total Size: {:?} | Content-Type: {:?} | Headers: {:?}",
            task.id,
            task.url,
            probe.final_url,
            initial_proto.as_str(),
            probe.status_code,
            if probe.resumable { "bytes" } else { "none" },
            total_size,
            probe.content_type,
            redacted
        );

        // 1. Load existing record & segments BEFORE resolving duplicate filenames so resume
        // never renames an existing `.edmpart` target to `file (1).bin`.
        let existing_record = {
            let store = self.store.clone();
            let id = task.id;
            tokio::task::spawn_blocking(move || {
                store
                    .list_all_downloads()
                    .ok()
                    .and_then(|records| records.into_iter().find(|d| d.id == id))
            })
            .await
            .map_err(|e| format!("Store lookup failed: {}", e))?
        };

        let is_task_name_valid = !task.filename.trim().is_empty()
            && task.filename != "download"
            && task.filename != "download.bin";

        let candidate_filename = if let Some(ref existing) = existing_record {
            existing.filename.clone()
        } else if is_task_name_valid {
            task.filename.clone()
        } else if let Some(ref pf) = probe.filename {
            pf.clone()
        } else {
            task.filename.clone()
        };

        let sanitized = crate::sanitizer::Sanitizer::sanitize_filename(&candidate_filename);
        let filename = if existing_record.is_some() {
            sanitized
        } else {
            let save_dir_clone = task.save_dir.clone();
            let sanitized_clone = sanitized.clone();
            let resolved_path = tokio::task::spawn_blocking(move || {
                crate::sanitizer::Sanitizer::resolve_duplicate(&save_dir_clone, &sanitized_clone)
            })
            .await
            .map_err(|e| format!("Filename resolution task failed: {}", e))?;
            resolved_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(sanitized)
        };

        let part_path = task.save_dir.join(format!("{}.edmpart", filename));
        let target_path = task.save_dir.join(&filename);
        let partial_or_target_exists = tokio::task::spawn_blocking({
            let part_path = part_path.clone();
            let target_path = target_path.clone();
            move || part_path.exists() || target_path.exists()
        })
        .await
        .map_err(|e| format!("Path existence check failed: {}", e))?;

        let mut existing_segments = if is_resumable {
            let store = self.store.clone();
            let id = task.id;
            tokio::task::spawn_blocking(move || store.get_segments(id).unwrap_or_default())
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        if let Some(ref existing) = existing_record {
            if !existing_segments.is_empty() {
                if should_invalidate_resume(
                    existing,
                    probe.etag.as_deref(),
                    probe.last_modified.as_deref(),
                    total_size,
                    partial_or_target_exists,
                ) {
                    warn!("Server file changed or partial file missing; restarting from byte 0");
                    let store = self.store.clone();
                    let id = task.id;
                    let etag = probe.etag.clone();
                    let lm = probe.last_modified.clone();
                    let part_to_remove = part_path.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        let _ = store.delete_segments(id);
                        if part_to_remove.exists() {
                            let _ = std::fs::remove_file(&part_to_remove);
                        }
                        let _ = store.update_download_validators(
                            id,
                            etag.as_deref(),
                            lm.as_deref(),
                            total_size,
                        );
                        let _ = store.update_download_progress(id, 0);
                    })
                    .await;
                    existing_segments.clear();
                } else if (existing.etag.is_none() && probe.etag.is_some())
                    || (existing.last_modified.is_none() && probe.last_modified.is_some())
                    || (existing.size_total.is_none() && total_size.is_some())
                {
                    let store = self.store.clone();
                    let id = task.id;
                    let etag = existing.etag.clone().or_else(|| probe.etag.clone());
                    let lm = existing
                        .last_modified
                        .clone()
                        .or_else(|| probe.last_modified.clone());
                    let tot = existing.size_total.or(total_size);
                    let _ = tokio::task::spawn_blocking(move || {
                        let _ = store.update_download_validators(
                            id,
                            etag.as_deref(),
                            lm.as_deref(),
                            tot,
                        );
                    })
                    .await;
                }
            }
        }

        // 2. Persist download in SQLite store (non-blocking)
        if existing_record.is_none() {
            let record = DownloadRecord {
                id: task.id,
                url: task.url.clone(),
                final_url: Some(probe.final_url.clone()),
                filename: filename.clone(),
                save_dir: task.save_dir.to_string_lossy().to_string(),
                category: "General".into(),
                state: DownloadState::Downloading,
                size_total: total_size,
                size_done: 0,
                resumable: is_resumable,
                etag: probe.etag.clone(),
                last_modified: probe.last_modified.clone(),
                content_type: probe.content_type.clone(),
                referer: effective_referer.clone(),
                user_agent: task.user_agent.clone(),
                connections: if is_resumable {
                    task.config.initial_connections
                } else {
                    1
                },
                created_at: Utc::now(),
                error_msg: None,
            };
            let store = self.store.clone();
            let _ = tokio::task::spawn_blocking(move || store.insert_download(&record)).await;
        } else {
            let store = self.store.clone();
            let id = task.id;
            let _ = tokio::task::spawn_blocking(move || {
                store.update_download_state(id, DownloadState::Downloading, None)
            })
            .await;
        }

        // 3. Check early cancellation before creating/preallocating .edmpart
        if task.cancelled.load(Ordering::Relaxed) {
            let store = self.store.clone();
            let id = task.id;
            let _ = tokio::task::spawn_blocking(move || {
                let _ = store.update_download_state(id, DownloadState::Paused, None);
                let _ = store.checkpoint();
            })
            .await;
            return Err("Download cancelled by user".into());
        }

        // 4. Initialize AdaptiveController (with negotiated protocol)
        let mut controller = AdaptiveController::new(task.config.clone(), is_resumable);
        controller.set_protocol(initial_proto);

        // 5. Handle zero-byte files cleanly without spawning workers or hanging
        if total_size == Some(0) {
            let save_dir = task.save_dir.clone();
            let fname = filename.clone();
            let url_copy = task.url.clone();
            let final_path = tokio::task::spawn_blocking(move || {
                let writer = PipelinedWriterHandle::new(&save_dir, &fname, Some(0), 16)
                    .map_err(|e| format!("Writer init failed for empty file: {}", e))?;
                writer
                    .finish(Some(&url_copy))
                    .map_err(|e| format!("Failed to finalize empty file: {}", e))
            })
            .await
            .map_err(|e| format!("Empty file task failed: {}", e))??;

            let store = self.store.clone();
            let id = task.id;
            let _ = tokio::task::spawn_blocking(move || {
                let _ = store.update_download_progress(id, 0);
                let _ = store.update_download_state(id, DownloadState::Completed, None);
            })
            .await;

            if let Some(ref cb) = telemetry_callback {
                cb(AdaptiveTelemetry {
                    download_id: task.id.to_string(),
                    total_bytes: Some(0),
                    downloaded_bytes: 0,
                    progress_pct: 100.0,
                    active_connections: 1,
                    target_connections: 1,
                    live_workers: 0,
                    max_allowed_connections: task.config.max_connections,
                    mode: controller.mode(),
                    eta_seconds: Some(0),
                    decision_reason: "Empty file (0 bytes) completed".into(),
                    negotiated_protocol: controller.protocol().as_str().to_string(),
                    updated_at: Utc::now(),
                    ..Default::default()
                });
            }

            info!("Completed 0-byte download: {:?}", final_path);
            return Ok(final_path);
        }

        // 6. Prepare pipelined disk writer (bounded zero-copy Bytes coalescing writer)
        let mut writer_handle = {
            let save_dir = task.save_dir.clone();
            let fname = filename.clone();
            tokio::task::spawn_blocking(move || {
                PipelinedWriterHandle::new(&save_dir, &fname, total_size, 256)
            })
            .await
            .map_err(|e| format!("Writer init task panicked: {}", e))?
            .map_err(|e| format!("Writer init failed: {}", e))?
        };
        let mut writer_tx = writer_handle.writer();

        let host_key = Url::parse(&probe.final_url)
            .ok()
            .map(|u| {
                format!(
                    "{}:{}",
                    u.host_str().unwrap_or("unknown").to_ascii_lowercase(),
                    u.port_or_known_default().unwrap_or(80)
                )
            })
            .unwrap_or_else(|| "unknown".to_string());
        let (host_gate, host_circuit) = self
            .host_registry
            .get_or_create(&host_key, task.config.max_connections_per_host);
        let host_semaphore =
            self.host_semaphore_for_url(&probe.final_url, task.config.max_connections_per_host);
        let paused_until_ms = Arc::new(AtomicU64::new(
            host_circuit.paused_until_ms.load(Ordering::Acquire),
        ));
        let active_proto = host_circuit.preferred_protocol().unwrap_or(initial_proto);
        let worker_client = client_pair.client_for_worker(0, active_proto);

        let mut scheduler = WorkScheduler::new(
            task.config.min_chunk_size,
            task.config.stall_timeout,
            task.config.max_worker_retries,
        );
        scheduler.set_download_context(
            probe.last_modified.clone(),
            total_size,
            paused_until_ms.clone(),
            Some(host_semaphore.clone()),
            task.config.request_timeout,
        );
        scheduler.set_host_gate(Some(host_gate.clone()));
        scheduler.set_tail_hedging(
            task.config.enable_hedged_requests,
            task.config.max_hedged_workers,
        );
        scheduler.set_target_connections(controller.current_connections());

        let (event_tx, mut event_rx) = mpsc::channel::<AdaptiveWorkerEvent>(512);

        let mut range_set = RangeSet::new();
        let mut total_downloaded: u64 = 0;

        if let Some(size) = total_size {
            let initial_conns = controller.current_connections().max(1);

            if !existing_segments.is_empty() {
                info!(
                    "Resuming download for task {} with {} saved segments",
                    task.id,
                    existing_segments.len()
                );
                for s in existing_segments {
                    if s.pos > s.start {
                        range_set.add(s.start, s.pos.saturating_sub(1));
                    }
                    if s.pos <= s.end {
                        scheduler.spawn_worker_at(
                            probe.final_url.clone(),
                            s.start,
                            s.end,
                            s.pos,
                            probe.etag.clone(),
                            effective_cookies.clone(),
                            effective_referer.clone(),
                            task.user_agent.clone(),
                            worker_client.clone(),
                            writer_tx.clone(),
                            task.limiter.clone(),
                            event_tx.clone(),
                        );
                    }
                }
                total_downloaded = range_set.total_covered_bytes();
            } else {
                let plans = Planner::plan(size, initial_conns, task.config.min_chunk_size);
                for plan in plans {
                    scheduler.spawn_worker(
                        probe.final_url.clone(),
                        plan.start,
                        plan.end,
                        probe.etag.clone(),
                        effective_cookies.clone(),
                        effective_referer.clone(),
                        task.user_agent.clone(),
                        worker_client.clone(),
                        writer_tx.clone(),
                        task.limiter.clone(),
                        event_tx.clone(),
                    );
                }
            }
        } else {
            scheduler.spawn_worker(
                probe.final_url.clone(),
                0,
                u64::MAX - 1,
                probe.etag.clone(),
                effective_cookies.clone(),
                effective_referer.clone(),
                task.user_agent.clone(),
                worker_client.clone(),
                writer_tx.clone(),
                task.limiter.clone(),
                event_tx.clone(),
            );
        }

        let mut last_sample_time = Instant::now();
        let mut sample_bytes: u64 = 0;
        let mut last_checkpoint = Instant::now();
        let mut checkpoint_handle: Option<tokio::task::JoinHandle<()>> = None;
        let mut sample_ticker = tokio::time::interval(task.config.sample_interval);
        sample_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut cancel_ticker = tokio::time::interval(Duration::from_millis(50));
        cancel_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut etag_restarts = 0u32;
        let mut retry_gap_attempts = 0u32;
        let start_time = Instant::now();
        let mut smoothed_speed_bytes: f64 = 0.0;
        let mut smoothed_eta_sec: Option<u64> = None;
        let mut last_progress_time = Instant::now();
        let mut active_etag = probe.etag.clone();
        let mut active_last_modified = probe.last_modified.clone();

        let mut latest_ttfb: Option<Duration> = None;

        loop {
            if task.cancelled.load(Ordering::Relaxed) {
                info!("Download task {} received cancellation signal", task.id);
                scheduler.cancel_all();
                writer_tx.cancel_and_close_budget();
                scheduler.join_all().await;

                while let Ok(ev) = event_rx.try_recv() {
                    if let AdaptiveWorkerEvent::Progress {
                        range_start,
                        range_end,
                        ..
                    } = ev
                    {
                        range_set.add(range_start, range_end);
                        total_downloaded = range_set.total_covered_bytes();
                    }
                }

                // Flush already-queued chunks (that w.pos already reflects) before aborting rename
                let _ = writer_tx.flush().await;
                let _ = tokio::task::spawn_blocking(move || writer_handle.abort()).await;
                await_checkpoint(&mut checkpoint_handle).await;

                let seg_records =
                    build_checkpoint_segments(task.id, &range_set, &scheduler, total_size, true);

                let store = self.store.clone();
                let id = task.id;
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = store.save_segment_positions(&seg_records);
                    let _ = store.update_download_progress(id, total_downloaded);
                    let _ = store.update_download_state(id, DownloadState::Paused, None);
                    let _ = store.checkpoint();
                })
                .await;
                return Err("Download cancelled by user".into());
            }

            tokio::select! {
                _ = cancel_ticker.tick() => {}

                maybe_event = event_rx.recv() => {
                    match maybe_event {
                        Some(AdaptiveWorkerEvent::Progress { bytes_read, range_start, range_end, .. }) => {
                            range_set.add(range_start, range_end);
                            total_downloaded = range_set.total_covered_bytes();
                            sample_bytes = sample_bytes.saturating_add(bytes_read);

                            // Non-blocking 2s SQLite checkpoint via tracked JoinHandle
                            if last_checkpoint.elapsed() >= Duration::from_secs(2)
                                && checkpoint_handle.as_ref().is_none_or(|h| h.is_finished())
                            {
                                await_checkpoint(&mut checkpoint_handle).await;
                                let seg_records = build_checkpoint_segments(
                                    task.id,
                                    &range_set,
                                    &scheduler,
                                    total_size,
                                    false,
                                );
                                let store = self.store.clone();
                                let id = task.id;
                                let done = total_downloaded;
                                checkpoint_handle = Some(tokio::task::spawn_blocking(move || {
                                    let _ = store.save_segment_positions(&seg_records);
                                    let _ = store.update_download_progress(id, done);
                                }));
                                last_checkpoint = Instant::now();
                            }
                        }
                        Some(AdaptiveWorkerEvent::ProtocolDetected { protocol, .. }) => {
                            if controller.protocol() != protocol {
                                controller.set_protocol(protocol);
                                scheduler.set_target_connections(controller.current_connections());
                            }
                        }
                        Some(AdaptiveWorkerEvent::TtfbMeasured { ttfb, .. }) => {
                            latest_ttfb = Some(match latest_ttfb {
                                Some(prev) => prev.max(ttfb),
                                None => ttfb,
                            });
                        }
                        Some(AdaptiveWorkerEvent::TransientError { .. }) => {
                            controller.record_transient_error();
                        }
                        Some(AdaptiveWorkerEvent::ServerIgnoredRange { idx }) => {
                            warn!(
                                "Task {}: Worker {} reported server ignored Range header. Switching to single-stream fallback.",
                                task.id, idx
                            );
                            controller.force_single_stream();
                            scheduler.set_target_connections(1);
                            scheduler.cancel_all_except(0);

                            if idx == 0 {
                                // Worker 0 reset to byte 0 on 200 OK: reset RangeSet cleanly to avoid double-counting
                                range_set = RangeSet::new();
                                total_downloaded = 0;
                                sample_bytes = 0;
                            }

                            let mut w0_active = false;
                            for w in &scheduler.workers {
                                if w.idx == 0 {
                                    if let Some(tot) = total_size {
                                        w.end.store(tot.saturating_sub(1), Ordering::SeqCst);
                                    } else {
                                        w.end.store(u64::MAX - 1, Ordering::SeqCst);
                                    }
                                    let s = w.get_status();
                                    if s == super::types::WorkerStatus::Connecting
                                        || s == super::types::WorkerStatus::Downloading
                                    {
                                        w0_active = true;
                                    }
                                }
                            }

                            if !w0_active {
                                let full_end = total_size.map(|s| s.saturating_sub(1)).unwrap_or(u64::MAX - 1);
                                scheduler.spawn_worker(
                                    probe.final_url.clone(),
                                    0,
                                    full_end,
                                    active_etag.clone(),
                                    effective_cookies.clone(),
                                    effective_referer.clone(),
                                    task.user_agent.clone(),
                                    worker_client.clone(),
                                    writer_tx.clone(),
                                    task.limiter.clone(),
                                    event_tx.clone(),
                                );
                            }
                        }
                        Some(AdaptiveWorkerEvent::ETagChanged { idx, old_etag, new_etag }) => {
                            warn!(
                                "Task {}: Worker {} detected validator change ('{}' -> '{}'). Restarting clean download.",
                                task.id, idx, old_etag, new_etag
                            );
                            scheduler.cancel_all();
                            scheduler.join_all().await;

                            if etag_restarts < 2 {
                                etag_restarts += 1;
                                while event_rx.try_recv().is_ok() {}

                                let _ = tokio::task::spawn_blocking(move || writer_handle.abort()).await;
                                let part_to_remove = part_path.clone();
                                let save_dir = task.save_dir.clone();
                                let fname = filename.clone();
                                writer_handle = tokio::task::spawn_blocking(move || {
                                    if part_to_remove.exists() {
                                        let _ = std::fs::remove_file(&part_to_remove);
                                    }
                                    PipelinedWriterHandle::new(&save_dir, &fname, total_size, 256)
                                })
                                .await
                                .map_err(|e| format!("Writer reinit failed: {}", e))?
                                .map_err(|e| format!("Writer reinit failed: {}", e))?;
                                writer_tx = writer_handle.writer();

                                active_etag = Some(new_etag.clone());
                                await_checkpoint(&mut checkpoint_handle).await;
                                let store = self.store.clone();
                                let id = task.id;
                                let etag_save = active_etag.clone();
                                let lm_save = active_last_modified.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    let _ = store.delete_segments(id);
                                    let _ = store.update_download_validators(
                                        id,
                                        etag_save.as_deref(),
                                        lm_save.as_deref(),
                                        total_size,
                                    );
                                    let _ = store.update_download_progress(id, 0);
                                })
                                .await;

                                range_set = RangeSet::new();
                                total_downloaded = 0;
                                sample_bytes = 0;
                                last_sample_time = Instant::now();

                                if let Some(size) = total_size {
                                    let initial_conns = controller.current_connections().max(1);
                                    let plans = Planner::plan(size, initial_conns, task.config.min_chunk_size);
                                    for plan in plans {
                                        scheduler.spawn_worker(
                                            probe.final_url.clone(),
                                            plan.start,
                                            plan.end,
                                            active_etag.clone(),
                                            effective_cookies.clone(),
                                            effective_referer.clone(),
                                            task.user_agent.clone(),
                                            worker_client.clone(),
                                            writer_tx.clone(),
                                            task.limiter.clone(),
                                            event_tx.clone(),
                                        );
                                    }
                                } else {
                                    scheduler.spawn_worker(
                                        probe.final_url.clone(),
                                        0,
                                        u64::MAX - 1,
                                        active_etag.clone(),
                                        effective_cookies.clone(),
                                        effective_referer.clone(),
                                        task.user_agent.clone(),
                                        worker_client.clone(),
                                        writer_tx.clone(),
                                        task.limiter.clone(),
                                        event_tx.clone(),
                                    );
                                }
                            } else {
                                error!("Task {}: Server ETag changed repeatedly. Aborting.", task.id);
                                let _ = tokio::task::spawn_blocking(move || writer_handle.abort()).await;
                                await_checkpoint(&mut checkpoint_handle).await;
                                let store = self.store.clone();
                                let id = task.id;
                                let _ = tokio::task::spawn_blocking(move || {
                                    let _ = store.update_download_state(id, DownloadState::Failed, Some("ETag changed repeatedly"));
                                })
                                .await;
                                return Err("Server ETag changed repeatedly mid-download".into());
                            }
                        }
                        Some(AdaptiveWorkerEvent::LastModifiedChanged { idx, old_last_modified, new_last_modified }) => {
                            warn!(
                                "Task {}: Worker {} detected Last-Modified change ('{}' -> '{}'). Restarting clean download.",
                                task.id, idx, old_last_modified, new_last_modified
                            );
                            scheduler.cancel_all();
                            scheduler.join_all().await;

                            if etag_restarts < 2 {
                                etag_restarts += 1;
                                while event_rx.try_recv().is_ok() {}

                                let _ = tokio::task::spawn_blocking(move || writer_handle.abort()).await;
                                let part_to_remove = part_path.clone();
                                let save_dir = task.save_dir.clone();
                                let fname = filename.clone();
                                writer_handle = tokio::task::spawn_blocking(move || {
                                    if part_to_remove.exists() {
                                        let _ = std::fs::remove_file(&part_to_remove);
                                    }
                                    PipelinedWriterHandle::new(&save_dir, &fname, total_size, 256)
                                })
                                .await
                                .map_err(|e| format!("Writer reinit failed: {}", e))?
                                .map_err(|e| format!("Writer reinit failed: {}", e))?;
                                writer_tx = writer_handle.writer();

                                active_last_modified = Some(new_last_modified.clone());
                                await_checkpoint(&mut checkpoint_handle).await;
                                scheduler.set_download_context(
                                    active_last_modified.clone(),
                                    total_size,
                                    paused_until_ms.clone(),
                                    Some(host_semaphore.clone()),
                                    task.config.request_timeout,
                                );

                                let store = self.store.clone();
                                let id = task.id;
                                let etag_save = active_etag.clone();
                                let lm_save = active_last_modified.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    let _ = store.delete_segments(id);
                                    let _ = store.update_download_validators(
                                        id,
                                        etag_save.as_deref(),
                                        lm_save.as_deref(),
                                        total_size,
                                    );
                                    let _ = store.update_download_progress(id, 0);
                                })
                                .await;

                                range_set = RangeSet::new();
                                total_downloaded = 0;
                                sample_bytes = 0;
                                last_sample_time = Instant::now();

                                if let Some(size) = total_size {
                                    let initial_conns = controller.current_connections().max(1);
                                    let plans = Planner::plan(size, initial_conns, task.config.min_chunk_size);
                                    for plan in plans {
                                        scheduler.spawn_worker(
                                            probe.final_url.clone(),
                                            plan.start,
                                            plan.end,
                                            active_etag.clone(),
                                            effective_cookies.clone(),
                                            effective_referer.clone(),
                                            task.user_agent.clone(),
                                            worker_client.clone(),
                                            writer_tx.clone(),
                                            task.limiter.clone(),
                                            event_tx.clone(),
                                        );
                                    }
                                } else {
                                    scheduler.spawn_worker(
                                        probe.final_url.clone(),
                                        0,
                                        u64::MAX - 1,
                                        active_etag.clone(),
                                        effective_cookies.clone(),
                                        effective_referer.clone(),
                                        task.user_agent.clone(),
                                        worker_client.clone(),
                                        writer_tx.clone(),
                                        task.limiter.clone(),
                                        event_tx.clone(),
                                    );
                                }
                            } else {
                                error!("Task {}: Server Last-Modified changed repeatedly. Aborting.", task.id);
                                let _ = tokio::task::spawn_blocking(move || writer_handle.abort()).await;
                                await_checkpoint(&mut checkpoint_handle).await;
                                let store = self.store.clone();
                                let id = task.id;
                                let _ = tokio::task::spawn_blocking(move || {
                                    let _ = store.update_download_state(id, DownloadState::Failed, Some("Last-Modified changed repeatedly"));
                                })
                                .await;
                                return Err("Server Last-Modified changed repeatedly mid-download".into());
                            }
                        }
                        Some(AdaptiveWorkerEvent::RateLimited { idx, retry_after }) => {
                            warn!("Worker {} reported rate limiting from host", idx);
                            let decision = controller.evaluate_sample(
                                sample_bytes,
                                last_sample_time.elapsed(),
                                0,
                                true,
                                retry_after,
                            );
                            if let AdaptiveDecision::LimitRate { retry_after, .. } = decision {
                                apply_limit_rate(
                                    &paused_until_ms,
                                    &mut scheduler,
                                    controller.current_connections(),
                                    retry_after,
                                );
                                sample_bytes = 0;
                                last_sample_time = Instant::now();
                            }
                        }
                        Some(AdaptiveWorkerEvent::Stalled { idx }) => {
                            warn!("Worker {} reported stall", idx);
                        }
                        Some(AdaptiveWorkerEvent::Done { idx }) => {
                            info!("Worker {} completed assigned byte range", idx);
                            let _hedged_won = scheduler.on_worker_finished(idx);

                            // Check completion BEFORE stealing/pruning
                            if let Some(tot) = total_size {
                                if range_set.is_complete(tot) {
                                    info!("All bytes verified and received for download {}", task.id);
                                    break;
                                }
                            }

                            if is_resumable && controller.mode() != AccelerationMode::FallbackSingleStream {
                                let rem_total = total_size
                                    .map(|tot| tot.saturating_sub(range_set.total_covered_bytes()))
                                    .unwrap_or(u64::MAX);

                                if rem_total <= TAIL_THRESHOLD_BYTES {
                                    if scheduler.try_tail_snipe(
                                        worker_client.clone(),
                                        writer_tx.clone(),
                                        task.limiter.clone(),
                                        event_tx.clone(),
                                        TAIL_THRESHOLD_BYTES,
                                    ).is_none() {
                                        scheduler.try_hedge_tail(
                                            worker_client.clone(),
                                            writer_tx.clone(),
                                            task.limiter.clone(),
                                            event_tx.clone(),
                                            TAIL_THRESHOLD_BYTES,
                                        );
                                    }
                                } else {
                                    scheduler.try_steal_work(
                                        worker_client.clone(),
                                        writer_tx.clone(),
                                        task.limiter.clone(),
                                        event_tx.clone(),
                                    );
                                }
                            }

                            scheduler.prune_finished_workers();
                            let active = scheduler.active_count();
                            if let Some(tot) = total_size {
                                if range_set.is_complete(tot) {
                                    info!("All bytes verified and received for download {}", task.id);
                                    break;
                                } else if active == 0 {
                                    let missing = range_set.missing_ranges(tot);
                                    warn!("Task {}: All workers stopped but {} byte ranges are missing: {:?}", task.id, missing.len(), missing);
                                    if retry_gap_attempts < 3 {
                                        retry_gap_attempts += 1;
                                        info!("Task {}: Recovering {} missing byte ranges (attempt {}/3)...", task.id, missing.len(), retry_gap_attempts);
                                        for (gap_start, gap_end) in missing {
                                            scheduler.spawn_worker(
                                                probe.final_url.clone(),
                                                gap_start,
                                                gap_end,
                                                active_etag.clone(),
                                                effective_cookies.clone(),
                                                effective_referer.clone(),
                                                task.user_agent.clone(),
                                                worker_client.clone(),
                                                writer_tx.clone(),
                                                task.limiter.clone(),
                                                event_tx.clone(),
                                            );
                                        }
                                    } else {
                                        error!("Task {}: Failed to fill missing byte ranges after retries", task.id);
                                        scheduler.cancel_all();
                                        scheduler.join_all().await;
                                        let _ = writer_tx.flush().await;
                                        let _ = tokio::task::spawn_blocking(move || writer_handle.abort()).await;
                                        await_checkpoint(&mut checkpoint_handle).await;
                                        let missing_count = missing.len();
                                        let store = self.store.clone();
                                        let id = task.id;
                                        let _ = tokio::task::spawn_blocking(move || {
                                            let _ = store.update_download_state(
                                                id,
                                                DownloadState::Failed,
                                                Some(&format!("Integrity check failed: {} unwritten ranges", missing_count)),
                                            );
                                        })
                                        .await;
                                        return Err(format!("Download failed integrity check: {} missing byte ranges", missing_count));
                                    }
                                }
                            } else if active == 0 {
                                info!("All workers stopped for unbounded download {}", task.id);
                                break;
                            }
                        }
                        Some(AdaptiveWorkerEvent::Failed { idx, reason }) => {
                            error!("Worker {} failed: {}", idx, reason);
                            scheduler.on_worker_failed(idx);
                            controller.record_transient_error();
                            scheduler.prune_worker(idx);

                            let is_fatal_disk_or_perm = reason.contains("Insufficient disk space")
                                || reason.contains("expired")
                                || reason.contains("Permanent HTTP");

                            if is_fatal_disk_or_perm || scheduler.active_count() == 0 {
                                if !is_fatal_disk_or_perm {
                                    if let Some(tot) = total_size {
                                        if range_set.is_complete(tot) {
                                            info!("Download {} complete despite failed worker event", task.id);
                                            break;
                                        }
                                        let missing = range_set.missing_ranges(tot);
                                        if retry_gap_attempts < 3 && !missing.is_empty() {
                                            retry_gap_attempts += 1;
                                            info!("Task {}: Retrying {} missing ranges after worker failure...", task.id, missing.len());
                                            for (gap_start, gap_end) in missing {
                                                scheduler.spawn_worker(
                                                    probe.final_url.clone(),
                                                    gap_start,
                                                    gap_end,
                                                    active_etag.clone(),
                                                    effective_cookies.clone(),
                                                    effective_referer.clone(),
                                                    task.user_agent.clone(),
                                                    worker_client.clone(),
                                                    writer_tx.clone(),
                                                    task.limiter.clone(),
                                                    event_tx.clone(),
                                                );
                                            }
                                            continue;
                                        }
                                    }
                                }

                                scheduler.cancel_all();
                                scheduler.join_all().await;
                                while let Ok(ev) = event_rx.try_recv() {
                                    if let AdaptiveWorkerEvent::Progress {
                                        range_start,
                                        range_end,
                                        ..
                                    } = ev
                                    {
                                        range_set.add(range_start, range_end);
                                        total_downloaded = range_set.total_covered_bytes();
                                    }
                                }
                                let _ = writer_tx.flush().await;
                                let seg_records = build_checkpoint_segments(
                                    task.id,
                                    &range_set,
                                    &scheduler,
                                    total_size,
                                    true,
                                );
                                let _ = tokio::task::spawn_blocking(move || writer_handle.abort()).await;
                                await_checkpoint(&mut checkpoint_handle).await;

                                let existing_done = existing_record
                                    .as_ref()
                                    .map(|r| r.size_done)
                                    .unwrap_or(0);
                                let is_expired = reason.contains("401")
                                    || reason.contains("410")
                                    || (reason.contains("403")
                                        && (total_downloaded > 0
                                            || existing_done > 0
                                            || has_expiry_query_params(&task.url)));
                                let state = if is_expired {
                                    DownloadState::LinkExpired
                                } else {
                                    DownloadState::Failed
                                };
                                let store = self.store.clone();
                                let id = task.id;
                                let done = total_downloaded;
                                let err_reason = reason.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    let _ = store.save_segment_positions(&seg_records);
                                    let _ = store.update_download_progress(id, done);
                                    let _ = store.update_download_state(id, state, Some(&err_reason));
                                    let _ = store.checkpoint();
                                })
                                .await;
                                return Err(format!("Workers stopped: {}", reason));
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }

                _ = sample_ticker.tick() => {
                    let sample_duration = last_sample_time.elapsed();
                    let error_count = scheduler.active_error_count();

                    let decision = controller.evaluate_sample_with_context(
                        sample_bytes,
                        sample_duration,
                        error_count,
                        false,
                        None,
                        latest_ttfb.take(),
                        host_gate.is_contended(),
                    );

                    match decision {
                        AdaptiveDecision::ScaleUp { to, .. } => {
                            scheduler.set_target_connections(to);
                            if is_resumable && controller.mode() != AccelerationMode::FallbackSingleStream {
                                let mut spawned_any = true;
                                while scheduler.active_count() < to && spawned_any {
                                    let rem_total = total_size
                                        .map(|tot| tot.saturating_sub(range_set.total_covered_bytes()))
                                        .unwrap_or(u64::MAX);

                                    let spawned = if rem_total <= TAIL_THRESHOLD_BYTES {
                                        scheduler.try_tail_snipe(
                                            worker_client.clone(),
                                            writer_tx.clone(),
                                            task.limiter.clone(),
                                            event_tx.clone(),
                                            TAIL_THRESHOLD_BYTES,
                                        ).or_else(|| {
                                            scheduler.try_hedge_tail(
                                                worker_client.clone(),
                                                writer_tx.clone(),
                                                task.limiter.clone(),
                                                event_tx.clone(),
                                                TAIL_THRESHOLD_BYTES,
                                            )
                                        })
                                    } else {
                                        scheduler.try_steal_work(
                                            worker_client.clone(),
                                            writer_tx.clone(),
                                            task.limiter.clone(),
                                            event_tx.clone(),
                                        )
                                    };
                                    spawned_any = spawned.is_some();
                                }
                            }
                        }
                        AdaptiveDecision::ScaleDown { to, .. } => {
                            scheduler.set_target_connections(to);
                            let retired = scheduler.retire_excess_workers(to);
                            if retired > 0 {
                                debug!("Task {}: ScaleDown retired {} excess workers toward target {}", task.id, retired, to);
                            }
                        }
                        AdaptiveDecision::LimitRate { retry_after, .. } => {
                            apply_limit_rate(
                                &paused_until_ms,
                                &mut scheduler,
                                controller.current_connections(),
                                retry_after,
                            );
                        }
                        AdaptiveDecision::HoldStable { connections, .. } => {
                            scheduler.set_target_connections(connections);
                        }
                        AdaptiveDecision::FallbackSingleStream { .. } => {
                            scheduler.set_target_connections(1);
                        }
                    }

                    let rem_total = total_size
                        .map(|tot| tot.saturating_sub(range_set.total_covered_bytes()))
                        .unwrap_or(u64::MAX);
                    let is_tail = total_size.is_some() && rem_total <= TAIL_THRESHOLD_BYTES;

                    scheduler.check_stalled_workers_adaptive(
                        &worker_client,
                        &writer_tx,
                        task.limiter.clone(),
                        &event_tx,
                        is_tail,
                    );

                    // Speculative tail hedging check during tail phase if a straggler remains
                    if is_tail
                        && is_resumable
                        && controller.mode() != AccelerationMode::FallbackSingleStream
                    {
                        scheduler.try_hedge_tail(
                            worker_client.clone(),
                            writer_tx.clone(),
                            task.limiter.clone(),
                            event_tx.clone(),
                            TAIL_THRESHOLD_BYTES,
                        );
                    }

                    if let Some(ref cb) = telemetry_callback {
                        let secs = sample_duration.as_secs_f64().max(0.001);
                        let instant_rate = if sample_duration.as_secs_f64() > 0.0 {
                            (sample_bytes as f64 / secs) as u64
                        } else {
                            0
                        };

                        if sample_bytes > 0 {
                            last_progress_time = Instant::now();
                        }

                        if smoothed_speed_bytes <= 0.0 {
                            smoothed_speed_bytes = instant_rate as f64;
                        } else {
                            smoothed_speed_bytes = 0.12 * (instant_rate as f64) + 0.88 * smoothed_speed_bytes;
                        }

                        let progress = if let Some(tot) = total_size {
                            if tot > 0 {
                                ((total_downloaded as f64 / tot as f64) * 100.0).min(100.0)
                            } else {
                                100.0
                            }
                        } else {
                            0.0
                        };

                        let effective_rate_bytes = (controller.rolling_throughput_bps() / 8).max(smoothed_speed_bytes as u64);

                        let is_warmed_up = start_time.elapsed() >= Duration::from_secs(3) || total_downloaded >= 1024 * 1024;
                        let eta = if !is_warmed_up {
                            None
                        } else if let Some(tot) = total_size {
                            let rem = tot.saturating_sub(total_downloaded);
                            if let Some(raw_eta) = rem.checked_div(effective_rate_bytes) {
                                let smoothed = if let Some(prev) = smoothed_eta_sec {
                                    let max_delta = (prev as f64 * 0.15).max(1.0) as u64;
                                    if raw_eta > prev + max_delta {
                                        prev + max_delta
                                    } else if prev > max_delta && raw_eta < prev - max_delta {
                                        prev - max_delta
                                    } else {
                                        raw_eta
                                    }
                                } else {
                                    raw_eta
                                };
                                smoothed_eta_sec = Some(smoothed);
                                Some(smoothed)
                            } else if last_progress_time.elapsed() < Duration::from_secs(10) {
                                smoothed_eta_sec
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let stable_conns = controller.stable_target().max(1);
                        let telemetry = AdaptiveTelemetry {
                            download_id: task.id.to_string(),
                            total_bytes: total_size,
                            downloaded_bytes: total_downloaded,
                            progress_pct: progress,
                            instant_throughput_bps: instant_rate * 8,
                            rolling_throughput_bps: if controller.rolling_throughput_bps() > 0 {
                                controller.rolling_throughput_bps()
                            } else {
                                (smoothed_speed_bytes as u64) * 8
                            },
                            upload_throughput_bps: 0,
                            uploaded_bytes: 0,
                            active_connections: stable_conns,
                            target_connections: controller.current_connections().max(1),
                            live_workers: scheduler.active_count(),
                            max_allowed_connections: task.config.max_connections,
                            mode: controller.mode(),
                            eta_seconds: eta,
                            decision_reason: controller.status_reason().to_string(),
                            phase: None,
                            workers: scheduler.get_telemetry_snapshot(),
                            updated_at: Utc::now(),
                            negotiated_protocol: controller.protocol().as_str().to_string(),
                            hedged_requests_won: scheduler.hedged_requests_won(),
                            coalesced_write_ops: writer_handle.coalesced_write_ops(),
                        };

                        debug!(
                            "[{:?}] Telemetry emission: id={}, proto={}, stable_target={}, live_workers={}, speed_bps={}, eta_sec={:?}",
                            Utc::now(), task.id, controller.protocol().as_str(), stable_conns, scheduler.active_count(), effective_rate_bytes * 8, eta
                        );

                        cb(telemetry);
                    }

                    sample_bytes = 0;
                    last_sample_time = Instant::now();
                }
            }
        }

        // 7. Graceful Worker Join Barrier before flushing/closing/finalizing writer
        scheduler.cancel_all();
        scheduler.join_all().await;
        await_checkpoint(&mut checkpoint_handle).await;

        while let Ok(ev) = event_rx.try_recv() {
            if let AdaptiveWorkerEvent::Progress {
                range_start,
                range_end,
                ..
            } = ev
            {
                range_set.add(range_start, range_end);
                total_downloaded = range_set.total_covered_bytes();
            }
        }

        if let Some(tot) = total_size {
            if !range_set.is_complete(tot) {
                let missing = range_set.missing_ranges(tot);
                let err = format!(
                    "Integrity verification failed: {} byte ranges missing from file",
                    missing.len()
                );
                error!("{}", err);
                drop(writer_tx);
                let _ = tokio::task::spawn_blocking(move || writer_handle.abort()).await;
                let store = self.store.clone();
                let id = task.id;
                let err_copy = err.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = store.update_download_state(id, DownloadState::Failed, Some(&err_copy));
                })
                .await;
                return Err(err);
            }
        }

        let coalesced_ops = writer_handle.coalesced_write_ops();
        let hedged_won = scheduler.hedged_requests_won();
        let final_workers = scheduler.get_final_telemetry_snapshot();
        let url_for_zone = task.url.clone();
        let effective_hash = task.expected_hash.clone().or(probe.repr_digest.clone());
        drop(writer_tx);
        let final_path = tokio::task::spawn_blocking(move || {
            writer_handle.finish_verified(Some(&url_for_zone), total_size, effective_hash.as_ref())
        })
        .await
        .map_err(|e| format!("File finalization task failed: {}", e))?
        .map_err(|e| format!("Failed to finalize file: {}", e))?;

        let final_completed_bytes = total_size.unwrap_or(total_downloaded);
        if total_size.is_none() && final_completed_bytes == 0 {
            let err = "Download failed: 0 bytes received from server".to_string();
            error!("{}", err);
            let store = self.store.clone();
            let id = task.id;
            let err_copy = err.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let _ = store.update_download_state(id, DownloadState::Failed, Some(&err_copy));
            })
            .await;
            return Err(err);
        }

        let store = self.store.clone();
        let id = task.id;
        let _ = tokio::task::spawn_blocking(move || {
            let _ = store.update_download_progress(id, final_completed_bytes);
            let _ = store.update_download_state(id, DownloadState::Completed, None);
        })
        .await;

        if let Some(ref cb) = telemetry_callback {
            cb(AdaptiveTelemetry {
                download_id: task.id.to_string(),
                total_bytes: total_size.or(Some(final_completed_bytes)),
                downloaded_bytes: final_completed_bytes,
                progress_pct: 100.0,
                active_connections: 0,
                target_connections: 0,
                live_workers: 0,
                max_allowed_connections: task.config.max_connections,
                mode: controller.mode(),
                eta_seconds: Some(0),
                decision_reason: "Completed and verified".into(),
                phase: Some("Completed".to_string()),
                workers: final_workers,
                negotiated_protocol: controller.protocol().as_str().to_string(),
                hedged_requests_won: hedged_won,
                coalesced_write_ops: coalesced_ops,
                updated_at: Utc::now(),
                ..Default::default()
            });
        }

        info!(
            "Download completed and verified successfully: {:?} ({} bytes, proto={}, coalesced_ops={})",
            final_path,
            final_completed_bytes,
            controller.protocol().as_str(),
            coalesced_ops
        );
        Ok(final_path)
    }
}

async fn await_checkpoint(handle: &mut Option<tokio::task::JoinHandle<()>>) {
    if let Some(jh) = handle.take() {
        let _ = jh.await;
    }
}

fn build_checkpoint_segments(
    download_id: Uuid,
    range_set: &RangeSet,
    scheduler: &WorkScheduler,
    total_size: Option<u64>,
    is_pause: bool,
) -> Vec<SegmentRecord> {
    let mut records = Vec::new();
    let mut next_idx = 0u32;

    for &(r_start, r_end) in range_set.ranges() {
        records.push(SegmentRecord {
            download_id,
            idx: next_idx,
            start: r_start,
            end: r_end,
            pos: r_end.saturating_add(1),
            state: "completed".to_string(),
            retries: 0,
        });
        next_idx += 1;
    }

    let mut pending_ranges = RangeSet::new();
    for w in &scheduler.workers {
        if w.is_hedged {
            continue;
        }
        let w_pos = w.pos.load(Ordering::Relaxed);
        let w_end = w.end.load(Ordering::Relaxed);
        if w_pos <= w_end {
            pending_ranges.add(w_pos, w_end);
            records.push(SegmentRecord {
                download_id,
                idx: next_idx,
                start: w_pos,
                end: w_end,
                pos: w_pos,
                state: if is_pause {
                    "paused".to_string()
                } else {
                    format!("{:?}", w.get_status()).to_lowercase()
                },
                retries: w.retry_count.load(Ordering::Relaxed),
            });
            next_idx += 1;
        }
    }

    if is_pause {
        if let Some(tot) = total_size {
            let mut covered_or_queued = range_set.clone();
            for &(ps, pe) in pending_ranges.ranges() {
                covered_or_queued.add(ps, pe);
            }
            for (gap_start, gap_end) in covered_or_queued.missing_ranges(tot) {
                records.push(SegmentRecord {
                    download_id,
                    idx: next_idx,
                    start: gap_start,
                    end: gap_end,
                    pos: gap_start,
                    state: "paused".to_string(),
                    retries: 0,
                });
                next_idx += 1;
            }
        }
    }

    records
}

fn apply_limit_rate(
    paused_until_ms: &AtomicU64,
    scheduler: &mut WorkScheduler,
    target: u32,
    retry_after: Option<Duration>,
) {
    let pause_dur = retry_after
        .unwrap_or_else(|| Duration::from_secs(5))
        .clamp(Duration::from_secs(1), Duration::from_secs(120));
    let until = epoch_ms_now().saturating_add(pause_dur.as_millis() as u64);
    paused_until_ms.fetch_max(until, Ordering::SeqCst);
    scheduler.set_target_connections(target);
    scheduler.retire_excess_workers(target);
}

pub(crate) fn has_expiry_query_params(url_str: &str) -> bool {
    let lower = url_str.to_ascii_lowercase();
    lower.contains("expires=")
        || lower.contains("x-amz-expires=")
        || lower.contains("signature=")
        || lower.contains("x-goog-signature=")
        || lower.contains("key-pair-id=")
        || lower.contains("token=")
        || lower.contains("sig=")
}

pub(crate) fn is_unexpected_html_response(
    url: &str,
    filename: &str,
    content_type: Option<&str>,
) -> bool {
    let Some(ct) = content_type else {
        return false;
    };
    let ct_lower = ct.trim().to_ascii_lowercase();
    let is_html_ct =
        ct_lower.starts_with("text/html") || ct_lower.starts_with("application/xhtml+xml");
    if !is_html_ct {
        return false;
    }

    let extract_ext = |s: &str| -> Option<String> {
        let clean = s
            .split('?')
            .next()
            .unwrap_or(s)
            .split('#')
            .next()
            .unwrap_or(s);
        let name = clean.rsplit('/').next().unwrap_or(clean);
        let (_, ext) = name.rsplit_once('.')?;
        let ext_lower = ext.trim().to_ascii_lowercase();
        if ext_lower.is_empty() || ext_lower.len() > 16 {
            None
        } else {
            Some(ext_lower)
        }
    };

    let html_exts = ["html", "htm", "xhtml", "mhtml", "php", "asp", "aspx", "jsp"];
    let non_html_exts = [
        "zip",
        "rar",
        "7z",
        "tar",
        "gz",
        "bz2",
        "xz",
        "zst",
        "iso",
        "img",
        "dmg",
        "exe",
        "msi",
        "msix",
        "appx",
        "deb",
        "rpm",
        "apk",
        "pkg",
        "appimage",
        "bin",
        "mp4",
        "mkv",
        "avi",
        "mov",
        "webm",
        "flv",
        "wmv",
        "m4v",
        "ts",
        "mp3",
        "flac",
        "wav",
        "aac",
        "ogg",
        "m4a",
        "opus",
        "pdf",
        "doc",
        "docx",
        "xls",
        "xlsx",
        "ppt",
        "pptx",
        "png",
        "jpg",
        "jpeg",
        "gif",
        "webp",
        "avif",
        "svg",
        "wasm",
        "dll",
        "so",
        "dylib",
        "jar",
        "torrent",
        "safetensors",
        "gguf",
        "onnx",
        "pt",
        "pth",
    ];

    if let Some(ext) = extract_ext(filename) {
        if html_exts.contains(&ext.as_str()) {
            return false;
        }
        if filename != "download.bin" && non_html_exts.contains(&ext.as_str()) {
            return true;
        }
    }

    if let Some(url_ext) = extract_ext(url) {
        if html_exts.contains(&url_ext.as_str()) {
            return false;
        }
        if non_html_exts.contains(&url_ext.as_str()) {
            return true;
        }
    }

    false
}

fn redact_headers(headers: &HashMap<String, String>) -> HashMap<&str, &str> {
    headers
        .iter()
        .map(|(k, v)| {
            let kl = k.to_ascii_lowercase();
            let is_sensitive = matches!(
                kl.as_str(),
                "set-cookie"
                    | "cookie"
                    | "authorization"
                    | "proxy-authorization"
                    | "x-amz-security-token"
                    | "x-api-key"
                    | "x-auth-token"
            );
            if is_sensitive {
                (k.as_str(), "[REDACTED]")
            } else {
                (k.as_str(), v.as_str())
            }
        })
        .collect()
}

/// Returns `true` when saved segments must be discarded and restarted from byte 0.
pub(crate) fn should_invalidate_resume(
    existing: &DownloadRecord,
    probe_etag: Option<&str>,
    probe_last_modified: Option<&str>,
    total_size: Option<u64>,
    partial_or_target_exists: bool,
) -> bool {
    if !partial_or_target_exists {
        return true;
    }

    if let (Some(old_etag), Some(new_etag)) = (existing.etag.as_deref(), probe_etag) {
        let old_norm = normalize_etag(old_etag);
        let new_norm = normalize_etag(new_etag);
        if !old_norm.is_empty() && !new_norm.is_empty() && old_norm != new_norm {
            return true;
        }
    }

    if let (Some(old_lm), Some(new_lm)) = (existing.last_modified.as_deref(), probe_last_modified) {
        let old_trim = old_lm.trim();
        let new_trim = new_lm.trim();
        if !old_trim.is_empty() && !new_trim.is_empty() && old_trim != new_trim {
            return true;
        }
    }

    if let (Some(old_size), Some(new_size)) = (existing.size_total, total_size) {
        if old_size != new_size {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(
        etag: Option<&str>,
        last_modified: Option<&str>,
        size_total: Option<u64>,
    ) -> DownloadRecord {
        DownloadRecord {
            id: Uuid::new_v4(),
            url: "https://example.com/archive.bin".into(),
            final_url: None,
            filename: "archive.bin".into(),
            save_dir: "C:\\Downloads".into(),
            category: "General".into(),
            state: DownloadState::Paused,
            size_total,
            size_done: 512,
            resumable: true,
            etag: etag.map(str::to_string),
            last_modified: last_modified.map(str::to_string),
            content_type: None,
            referer: None,
            user_agent: None,
            connections: 4,
            created_at: Utc::now(),
            error_msg: None,
        }
    }

    #[test]
    fn test_should_invalidate_resume_on_validator_mismatch_or_missing_file() {
        let rec = sample_record(
            Some("\"abc123\""),
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(1024),
        );

        assert!(!should_invalidate_resume(
            &rec,
            Some("W/\"abc123\""),
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(1024),
            true,
        ));

        assert!(should_invalidate_resume(
            &rec,
            Some("\"abc123\""),
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(1024),
            false,
        ));

        assert!(should_invalidate_resume(
            &rec,
            Some("\"def456\""),
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(1024),
            true,
        ));

        assert!(should_invalidate_resume(
            &rec,
            Some("\"abc123\""),
            Some("Thu, 22 Oct 2015 07:28:00 GMT"),
            Some(1024),
            true,
        ));

        assert!(should_invalidate_resume(
            &rec,
            Some("\"abc123\""),
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(2048),
            true,
        ));

        let rec_no_etag = sample_record(None, None, Some(1024));
        assert!(!should_invalidate_resume(
            &rec_no_etag,
            Some("\"new-etag\""),
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(1024),
            true,
        ));
    }

    #[test]
    fn test_is_unexpected_html_response() {
        assert!(is_unexpected_html_response(
            "https://example.com/archive.zip",
            "archive.zip",
            Some("text/html; charset=utf-8")
        ));
        assert!(is_unexpected_html_response(
            "https://example.com/installer.exe?token=abc",
            "download.bin",
            Some("text/html")
        ));
        assert!(!is_unexpected_html_response(
            "https://example.com/index.html",
            "index.html",
            Some("text/html; charset=utf-8")
        ));
        assert!(!is_unexpected_html_response(
            "https://example.com/archive.zip",
            "archive.zip",
            Some("application/zip")
        ));
    }

    #[test]
    fn test_redact_headers_hides_secrets() {
        let mut h = HashMap::new();
        h.insert("Set-Cookie".to_string(), "session=secret123".to_string());
        h.insert("Authorization".to_string(), "Bearer tok".to_string());
        h.insert(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        );
        let r = redact_headers(&h);
        assert_eq!(r.get("Set-Cookie"), Some(&"[REDACTED]"));
        assert_eq!(r.get("Authorization"), Some(&"[REDACTED]"));
        assert_eq!(r.get("Content-Type"), Some(&"application/octet-stream"));
    }

    #[test]
    fn test_build_checkpoint_segments_preserves_completed_and_active_ranges() {
        use super::super::worker::AdaptiveWorker;

        let id = Uuid::new_v4();
        let mut rs = RangeSet::new();
        rs.add(0, 999);
        rs.add(2000, 2499);

        let mut sched = WorkScheduler::new(512, Duration::from_secs(5), 3);
        let w1 = Arc::new(AdaptiveWorker::new(
            1,
            "http://localhost/file".into(),
            2000,
            2999,
            2500,
            None,
            None,
            None,
            None,
        ));
        let w_hedged = Arc::new(
            AdaptiveWorker::new(
                2,
                "http://localhost/file".into(),
                2500,
                2999,
                2500,
                None,
                None,
                None,
                None,
            )
            .with_hedged(true),
        );
        sched.workers.push(w1);
        sched.workers.push(w_hedged);

        let segs = build_checkpoint_segments(id, &rs, &sched, Some(4000), true);
        // Sequential idx 0..=3: [0..999] completed, [2000..2499] completed, [2500..2999] paused, [1000..1999] & [3000..3999] gaps
        assert_eq!(segs.len(), 5);
        for (i, seg) in segs.iter().enumerate() {
            assert_eq!(seg.idx, i as u32);
        }
        assert_eq!((segs[0].start, segs[0].end, segs[0].pos), (0, 999, 1000));
        assert_eq!(
            (segs[1].start, segs[1].end, segs[1].pos),
            (2000, 2499, 2500)
        );
        assert_eq!(
            (segs[2].start, segs[2].end, segs[2].pos),
            (2500, 2999, 2500)
        );
        assert_eq!(
            (segs[3].start, segs[3].end, segs[3].pos),
            (1000, 1999, 1000)
        );
        assert_eq!(
            (segs[4].start, segs[4].end, segs[4].pos),
            (3000, 3999, 3000)
        );
    }
}
