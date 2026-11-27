use bytes::{Bytes, BytesMut};
use chrono::{DateTime, NaiveDateTime, Utc};
use futures_util::StreamExt;
use reqwest::header::{
    ACCEPT_ENCODING, AUTHORIZATION, COOKIE, IF_RANGE, LAST_MODIFIED, LOCATION, RANGE, REFERER,
    RETRY_AFTER, USER_AGENT,
};
use reqwest::{Client, Response, StatusCode, Version};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tokio::time::timeout;
use tracing::{debug, error, warn};
use url::Url;

use super::pipeline_writer::PipelinedWriterTx;
use super::types::{TransportProtocol, WorkerStatus};
use crate::limiter::TokenBucketLimiter;

const WRITE_STAGING_THRESHOLD: usize = 256 * 1024; // 256 KiB coalescing buffer
const MAX_STAGING_AGE: Duration = Duration::from_millis(500);
const SUSTAINED_PROGRESS_RESET_BYTES: u64 = 256 * 1024; // 256 KiB resets retry counter
const MAX_RETRY_AFTER_SECS: u64 = 120;
const MAX_REDIRECTS: usize = 10;

#[derive(Debug)]
pub enum AdaptiveWorkerEvent {
    Progress {
        idx: u32,
        bytes_read: u64,
        range_start: u64,
        range_end: u64,
    },
    ProtocolDetected {
        idx: u32,
        protocol: TransportProtocol,
    },
    ServerIgnoredRange {
        idx: u32,
    },
    ETagChanged {
        idx: u32,
        old_etag: String,
        new_etag: String,
    },
    LastModifiedChanged {
        idx: u32,
        old_last_modified: String,
        new_last_modified: String,
    },
    RateLimited {
        idx: u32,
        retry_after: Option<Duration>,
    },
    TransientError {
        idx: u32,
    },
    Stalled {
        idx: u32,
    },
    Done {
        idx: u32,
    },
    Failed {
        idx: u32,
        reason: String,
    },
}

pub fn parse_content_range(val: &str) -> Option<(u64, u64, Option<u64>)> {
    let val = val.trim();
    let lower = val.to_ascii_lowercase();
    if !lower.starts_with("bytes ") {
        return None;
    }
    let rest = val[6..].trim();
    let mut parts = rest.split('/');
    let range_str = parts.next()?.trim();
    let total_str = parts.next().map(|s| s.trim());

    let mut range_parts = range_str.split('-');
    let start_str = range_parts.next()?.trim();
    let end_str = range_parts.next()?.trim();

    let start = start_str.parse::<u64>().ok()?;
    let end = end_str.parse::<u64>().ok()?;
    let total = total_str.and_then(|s| {
        if s == "*" {
            None
        } else {
            s.parse::<u64>().ok()
        }
    });

    Some((start, end, total))
}

pub(crate) fn normalize_etag(etag: &str) -> &str {
    let s = etag.trim();
    let s = s
        .strip_prefix("W/")
        .or_else(|| s.strip_prefix("w/"))
        .unwrap_or(s);
    s.trim().trim_matches('"')
}

pub fn parse_retry_after(val: &str) -> Option<Duration> {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(secs.min(MAX_RETRY_AFTER_SECS)));
    }
    let parsed_utc: Option<DateTime<Utc>> = DateTime::parse_from_rfc2822(trimmed)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            DateTime::parse_from_rfc3339(trimmed)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        })
        .or_else(|| {
            // IMF-fixdate or asctime fallback without numeric tz offset
            NaiveDateTime::parse_from_str(trimmed, "%a, %d %b %Y %H:%M:%S GMT")
                .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%A, %d-%b-%y %H:%M:%S GMT"))
                .ok()
                .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc))
        });

    parsed_utc.map(|dt| {
        let delta = dt.signed_duration_since(Utc::now());
        if delta.num_seconds() <= 0 {
            Duration::from_secs(1)
        } else {
            Duration::from_secs((delta.num_seconds() as u64).min(MAX_RETRY_AFTER_SECS))
        }
    })
}

pub(crate) fn origin_key(url: &Url) -> (String, String, Option<u16>) {
    (
        url.scheme().to_ascii_lowercase(),
        url.host_str().unwrap_or_default().to_ascii_lowercase(),
        url.port_or_known_default(),
    )
}

pub(crate) fn protocol_from_version(version: Version) -> TransportProtocol {
    match version {
        Version::HTTP_2 => TransportProtocol::Http2,
        Version::HTTP_3 => TransportProtocol::Http3,
        _ => TransportProtocol::Http1,
    }
}

pub(crate) fn epoch_ms_now() -> u64 {
    Utc::now().timestamp_millis().max(0) as u64
}

fn jittered_backoff(idx: u32, retries: u32) -> Duration {
    let base_ms = 400u64.saturating_mul(1u64 << retries.min(5)).min(20_000);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let mut seed = nanos
        ^ ((idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        ^ ((retries as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    seed ^= seed >> 12;
    seed ^= seed << 25;
    seed ^= seed >> 27;
    let jitter_span = (base_ms / 2).max(1);
    let jitter_ms = seed.wrapping_mul(0x2545_F491_4F6C_DD1D) % jitter_span;
    Duration::from_millis((base_ms + jitter_ms).min(30_000))
}

fn is_disk_full(err: &std::io::Error) -> bool {
    match err.raw_os_error() {
        Some(112) | Some(28) => true,
        _ => {
            let msg = err.to_string().to_ascii_lowercase();
            msg.contains("disk is full") || msg.contains("no space left")
        }
    }
}

async fn wait_for_cancel(cancelled: &AtomicBool) {
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn sleep_or_cancel(cancelled: &AtomicBool, dur: Duration) -> bool {
    if dur.is_zero() {
        return cancelled.load(Ordering::Relaxed);
    }
    tokio::select! {
        _ = wait_for_cancel(cancelled) => true,
        _ = tokio::time::sleep(dur) => cancelled.load(Ordering::Relaxed),
    }
}

enum FlushOutcome {
    Continue,
    HitBoundary,
    RetryableError,
    FatalDiskFull,
}

#[derive(Clone)]
pub struct AdaptiveWorker {
    pub idx: u32,
    pub url: String,
    pub start: u64,
    pub end: Arc<AtomicU64>,
    pub pos: Arc<AtomicU64>,
    pub bytes_downloaded: Arc<AtomicU64>,
    pub last_activity_ms: Arc<AtomicU64>,
    pub status: Arc<AtomicU8>,
    pub retry_count: Arc<AtomicU32>,
    pub speed_bps: Arc<AtomicU64>,
    pub last_snapshot_bytes: Arc<AtomicU64>,
    pub last_snapshot_ms: Arc<AtomicU64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub total_size: Option<u64>,
    pub cancelled: Arc<AtomicBool>,
    pub is_reconnecting: Arc<AtomicBool>,
    pub is_writing_disk: Arc<AtomicBool>,
    pub paused_until_ms: Arc<AtomicU64>,
    pub host_semaphore: Option<Arc<Semaphore>>,
    pub is_hedged: bool,
    pub cookies: Option<String>,
    pub referer: Option<String>,
    pub user_agent: Option<String>,
}

impl AdaptiveWorker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        idx: u32,
        url: String,
        start: u64,
        end: u64,
        pos: u64,
        etag: Option<String>,
        cookies: Option<String>,
        referer: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        let now = epoch_ms_now();
        Self {
            idx,
            url,
            start,
            end: Arc::new(AtomicU64::new(end)),
            pos: Arc::new(AtomicU64::new(pos)),
            bytes_downloaded: Arc::new(AtomicU64::new(0)),
            last_activity_ms: Arc::new(AtomicU64::new(now)),
            status: Arc::new(AtomicU8::new(0)), // 0 = Connecting
            retry_count: Arc::new(AtomicU32::new(0)),
            speed_bps: Arc::new(AtomicU64::new(0)),
            last_snapshot_bytes: Arc::new(AtomicU64::new(0)),
            last_snapshot_ms: Arc::new(AtomicU64::new(now)),
            etag,
            last_modified: None,
            total_size: None,
            cancelled: Arc::new(AtomicBool::new(false)),
            is_reconnecting: Arc::new(AtomicBool::new(false)),
            is_writing_disk: Arc::new(AtomicBool::new(false)),
            paused_until_ms: Arc::new(AtomicU64::new(0)),
            host_semaphore: None,
            is_hedged: false,
            cookies,
            referer,
            user_agent,
        }
    }

    pub fn with_last_modified(mut self, last_modified: Option<String>) -> Self {
        self.last_modified = last_modified;
        self
    }

    pub fn with_total_size(mut self, total_size: Option<u64>) -> Self {
        self.total_size = total_size;
        self
    }

    pub fn with_paused_until(mut self, paused_until_ms: Arc<AtomicU64>) -> Self {
        self.paused_until_ms = paused_until_ms;
        self
    }

    pub fn with_host_semaphore(mut self, host_semaphore: Option<Arc<Semaphore>>) -> Self {
        self.host_semaphore = host_semaphore;
        self
    }

    pub fn with_hedged(mut self, is_hedged: bool) -> Self {
        self.is_hedged = is_hedged;
        self
    }

    pub fn get_status(&self) -> WorkerStatus {
        match self.status.load(Ordering::Relaxed) {
            0 => WorkerStatus::Connecting,
            1 => WorkerStatus::Downloading,
            2 => WorkerStatus::Stalled,
            3 => WorkerStatus::Completed,
            _ => WorkerStatus::Failed,
        }
    }

    pub fn set_status(&self, status: WorkerStatus) {
        let val = match status {
            WorkerStatus::Connecting => 0,
            WorkerStatus::Downloading => 1,
            WorkerStatus::Stalled => 2,
            WorkerStatus::Completed => 3,
            WorkerStatus::Failed => 4,
        };
        self.status.store(val, Ordering::Relaxed);
    }

    pub fn remaining_bytes(&self) -> u64 {
        let pos = self.pos.load(Ordering::Relaxed);
        let end = self.end.load(Ordering::Relaxed);
        if end >= pos {
            end.saturating_sub(pos).saturating_add(1)
        } else {
            0
        }
    }

    pub fn mark_activity(&self) {
        self.last_activity_ms
            .store(epoch_ms_now(), Ordering::Relaxed);
    }

    pub fn is_stalled(&self, timeout_secs: u64) -> bool {
        if self.get_status() == WorkerStatus::Completed
            || self.get_status() == WorkerStatus::Failed
            || self.is_reconnecting.load(Ordering::Relaxed)
        {
            return false;
        }
        let now = epoch_ms_now();
        let last = self.last_activity_ms.load(Ordering::Relaxed);
        now.saturating_sub(last) > (timeout_secs * 1000)
    }

    /// Atomically reserves `[claimed_offset .. claimed_offset + len - 1]` against `self.end`
    /// BEFORE writing to the pipeline writer, preventing victim-write vs work-steal overlap.
    fn claim_write_window(&self, desired_len: usize) -> Option<(u64, usize, bool)> {
        if desired_len == 0 {
            return None;
        }
        loop {
            let cur_pos = self.pos.load(Ordering::SeqCst);
            let cur_end = self.end.load(Ordering::SeqCst);
            if cur_pos > cur_end {
                return None;
            }
            let avail = cur_end.saturating_sub(cur_pos).saturating_add(1);
            let want = (desired_len as u64).min(avail) as usize;
            if want == 0 {
                return None;
            }
            let next_pos = cur_pos.saturating_add(want as u64);
            if self
                .pos
                .compare_exchange_weak(cur_pos, next_pos, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                // Re-check `end` after advancing `pos` under SeqCst to stay linearizable
                // with `WorkScheduler::claim_next_range` shrinking `end`.
                let post_end = self.end.load(Ordering::SeqCst);
                if next_pos.saturating_sub(1) > post_end {
                    let clamped_len = if cur_pos <= post_end {
                        (post_end.saturating_sub(cur_pos).saturating_add(1)).min(want as u64)
                            as usize
                    } else {
                        0
                    };
                    self.pos
                        .store(cur_pos.saturating_add(clamped_len as u64), Ordering::SeqCst);
                    if clamped_len == 0 {
                        return None;
                    }
                    return Some((cur_pos, clamped_len, true));
                }
                let hit_boundary = next_pos > post_end;
                return Some((cur_pos, want, hit_boundary));
            }
        }
    }

    async fn flush_staging(
        &self,
        staging: &mut BytesMut,
        writer: &PipelinedWriterTx,
        event_tx: &mpsc::Sender<AdaptiveWorkerEvent>,
    ) -> FlushOutcome {
        if staging.is_empty() {
            return FlushOutcome::Continue;
        }

        let buf: Bytes = staging.split().freeze();
        let Some((write_offset, allowed, hit_boundary)) = self.claim_write_window(buf.len()) else {
            return FlushOutcome::HitBoundary;
        };

        let payload = if allowed < buf.len() {
            buf.slice(0..allowed)
        } else {
            buf
        };

        self.is_writing_disk.store(true, Ordering::Release);
        let write_res = writer.write_bytes_at(write_offset, payload).await;
        self.is_writing_disk.store(false, Ordering::Release);
        self.mark_activity();

        if let Err(e) = write_res {
            error!("Worker {} disk write error: {}", self.idx, e);
            if is_disk_full(&e) {
                self.set_status(WorkerStatus::Failed);
                let _ = event_tx
                    .send(AdaptiveWorkerEvent::Failed {
                        idx: self.idx,
                        reason: "Insufficient disk space: Disk is full".into(),
                    })
                    .await;
                return FlushOutcome::FatalDiskFull;
            }
            // Roll back pos to `write_offset` if safe so a retry can re-fetch these bytes
            let _ = self.pos.compare_exchange(
                write_offset.saturating_add(allowed as u64),
                write_offset,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            return FlushOutcome::RetryableError;
        }

        self.bytes_downloaded
            .fetch_add(allowed as u64, Ordering::Relaxed);
        self.mark_activity();

        let _ = event_tx
            .send(AdaptiveWorkerEvent::Progress {
                idx: self.idx,
                bytes_read: allowed as u64,
                range_start: write_offset,
                range_end: write_offset
                    .saturating_add(allowed as u64)
                    .saturating_sub(1),
            })
            .await;

        if hit_boundary {
            FlushOutcome::HitBoundary
        } else {
            FlushOutcome::Continue
        }
    }

    async fn send_request_with_safe_redirects(
        &self,
        client: &Client,
        range_header: &str,
        if_range: Option<&str>,
        request_timeout: Duration,
    ) -> Result<Option<Response>, String> {
        let initial_url = Url::parse(&self.url).map_err(|e| format!("Invalid URL: {}", e))?;
        let initial_origin = origin_key(&initial_url);
        let mut current_url = initial_url;

        for _hop in 0..=MAX_REDIRECTS {
            if self.cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }

            let mut req = client
                .get(current_url.as_str())
                .header(ACCEPT_ENCODING, "identity")
                .header(RANGE, range_header);

            if let Some(ir) = if_range {
                req = req.header(IF_RANGE, ir);
            }

            // Strip sensitive headers on cross-origin redirects
            if origin_key(&current_url) == initial_origin {
                if let Some(ref cookie_str) = self.cookies {
                    req = req.header(COOKIE, cookie_str.as_str());
                }
                if let Some(ref ref_str) = self.referer {
                    req = req.header(REFERER, ref_str.as_str());
                }
            }

            if let Some(ref ua_str) = self.user_agent {
                req = req.header(USER_AGENT, ua_str.as_str());
            }

            let resp_res = tokio::select! {
                _ = wait_for_cancel(&self.cancelled) => return Ok(None),
                res = timeout(request_timeout, req.send()) => res,
            };

            let resp = match resp_res {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => return Err(format!("connection error: {}", e)),
                Err(_) => return Err(format!("request timed out after {:?}", request_timeout)),
            };

            if resp.status().is_redirection() {
                let Some(loc) = resp.headers().get(LOCATION).and_then(|v| v.to_str().ok()) else {
                    return Ok(Some(resp));
                };
                let next_url = current_url
                    .join(loc)
                    .map_err(|e| format!("Invalid redirect Location '{}': {}", loc, e))?;
                let scheme = next_url.scheme();
                if scheme != "http" && scheme != "https" {
                    return Err(format!("Blocked unsafe redirect scheme '{}'", scheme));
                }
                // Explicit drop of AUTHORIZATION if any client middleware attached it
                let _ = AUTHORIZATION;
                current_url = next_url;
                continue;
            }

            return Ok(Some(resp));
        }

        Err(format!("Exceeded {} redirect hops", MAX_REDIRECTS))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &self,
        client: Client,
        writer: PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: mpsc::Sender<AdaptiveWorkerEvent>,
        stall_timeout: Duration,
        request_timeout: Duration,
        max_retries: u32,
    ) {
        let mut retries = 0u32;
        let mut expected_last_modified = self.last_modified.clone();
        let mut reported_protocol = false;

        while retries < max_retries && !self.cancelled.load(Ordering::Relaxed) {
            // Respect host-wide rate-limit pause window before acquiring semaphore or connecting
            let paused_until = self.paused_until_ms.load(Ordering::Relaxed);
            let now_ms = epoch_ms_now();
            if paused_until > now_ms {
                let wait = Duration::from_millis((paused_until - now_ms).min(120_000));
                if sleep_or_cancel(&self.cancelled, wait).await {
                    return;
                }
            }

            let current_pos = self.pos.load(Ordering::SeqCst);
            let current_end = self.end.load(Ordering::SeqCst);

            if current_pos > current_end {
                self.set_status(WorkerStatus::Completed);
                let _ = event_tx
                    .send(AdaptiveWorkerEvent::Done { idx: self.idx })
                    .await;
                return;
            }

            let _host_permit = if let Some(ref sem) = self.host_semaphore {
                tokio::select! {
                    _ = wait_for_cancel(&self.cancelled) => return,
                    permit_res = sem.clone().acquire_owned() => match permit_res {
                        Ok(p) => Some(p),
                        Err(_) => return,
                    }
                }
            } else {
                None
            };

            self.set_status(WorkerStatus::Connecting);
            self.mark_activity();

            debug!(
                "AdaptiveWorker {} connecting for bytes {}-{} (attempt {})",
                self.idx,
                current_pos,
                current_end,
                retries + 1
            );

            let range_header = format!("bytes={}-{}", current_pos, current_end);
            // RFC 9110 §13.1.5: If-Range requires a strong ETag (weak `W/` ETags MUST NOT be sent);
            // fall back to Last-Modified when ETag is absent or weak.
            let strong_etag = self.etag.as_deref().and_then(|tag| {
                let trimmed = tag.trim();
                if trimmed.is_empty() || trimmed.starts_with("W/") || trimmed.starts_with("w/") {
                    None
                } else {
                    Some(trimmed)
                }
            });
            let if_range = strong_etag.or(expected_last_modified.as_deref());

            let resp = match self
                .send_request_with_safe_redirects(&client, &range_header, if_range, request_timeout)
                .await
            {
                Ok(Some(r)) => r,
                Ok(None) => return,
                Err(err_str) => {
                    warn!("Worker {} {}", self.idx, err_str);
                    let _ = event_tx
                        .send(AdaptiveWorkerEvent::TransientError { idx: self.idx })
                        .await;
                    retries += 1;
                    self.retry_count.store(retries, Ordering::Relaxed);
                    drop(_host_permit);
                    if sleep_or_cancel(&self.cancelled, jittered_backoff(self.idx, retries)).await {
                        return;
                    }
                    continue;
                }
            };

            if !reported_protocol {
                reported_protocol = true;
                let proto = protocol_from_version(resp.version());
                let _ = event_tx
                    .send(AdaptiveWorkerEvent::ProtocolDetected {
                        idx: self.idx,
                        protocol: proto,
                    })
                    .await;
            }

            let status = resp.status();

            // Fail fast on expired links (401/403/410) and permanent 4xx (400/404/416)
            if status == StatusCode::UNAUTHORIZED
                || status == StatusCode::FORBIDDEN
                || status == StatusCode::GONE
            {
                let err_msg = format!(
                    "Download link expired (HTTP {}). Please refresh URL to resume.",
                    status.as_u16()
                );
                warn!(
                    "Worker {} encountered HTTP {}: {}",
                    self.idx, status, err_msg
                );
                self.set_status(WorkerStatus::Failed);
                let _ = event_tx
                    .send(AdaptiveWorkerEvent::Failed {
                        idx: self.idx,
                        reason: err_msg,
                    })
                    .await;
                return;
            }

            if status == StatusCode::BAD_REQUEST
                || status == StatusCode::NOT_FOUND
                || status == StatusCode::RANGE_NOT_SATISFIABLE
            {
                let err_msg = format!("Permanent HTTP {} error", status.as_u16());
                warn!("Worker {} encountered {}: failing fast", self.idx, err_msg);
                self.set_status(WorkerStatus::Failed);
                let _ = event_tx
                    .send(AdaptiveWorkerEvent::Failed {
                        idx: self.idx,
                        reason: err_msg,
                    })
                    .await;
                return;
            }

            // Handle HTTP 429 Too Many Requests & 503 Service Unavailable with Retry-After
            if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE
            {
                let retry_after = resp
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(parse_retry_after);

                let _ = event_tx
                    .send(AdaptiveWorkerEvent::RateLimited {
                        idx: self.idx,
                        retry_after,
                    })
                    .await;

                retries += 1;
                self.retry_count.store(retries, Ordering::Relaxed);
                if retries >= max_retries {
                    let err_msg = format!(
                        "HTTP {} Rate Limited / Unavailable by Host",
                        status.as_u16()
                    );
                    self.set_status(WorkerStatus::Failed);
                    let _ = event_tx
                        .send(AdaptiveWorkerEvent::Failed {
                            idx: self.idx,
                            reason: err_msg,
                        })
                        .await;
                    return;
                }

                let wait_duration =
                    retry_after.unwrap_or_else(|| jittered_backoff(self.idx, retries));
                drop(_host_permit);
                if sleep_or_cancel(&self.cancelled, wait_duration).await {
                    return;
                }
                continue;
            }

            if !status.is_success() && status != StatusCode::PARTIAL_CONTENT {
                warn!("Worker {} received HTTP status {}", self.idx, status);
                let _ = event_tx
                    .send(AdaptiveWorkerEvent::TransientError { idx: self.idx })
                    .await;
                retries += 1;
                self.retry_count.store(retries, Ordering::Relaxed);
                if retries >= max_retries {
                    let err_msg = format!("HTTP Error Status {}", status);
                    self.set_status(WorkerStatus::Failed);
                    let _ = event_tx
                        .send(AdaptiveWorkerEvent::Failed {
                            idx: self.idx,
                            reason: err_msg,
                        })
                        .await;
                    return;
                }
                drop(_host_permit);
                if sleep_or_cancel(&self.cancelled, jittered_backoff(self.idx, retries)).await {
                    return;
                }
                continue;
            }

            if let Some(ct) = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
            {
                if super::engine::is_unexpected_html_response(&self.url, "", Some(ct)) {
                    let err_msg = format!(
                        "Server returned an HTML error/challenge page ({}) instead of the expected file",
                        ct
                    );
                    warn!("Worker {} aborting: {}", self.idx, err_msg);
                    self.set_status(WorkerStatus::Failed);
                    let _ = event_tx
                        .send(AdaptiveWorkerEvent::Failed {
                            idx: self.idx,
                            reason: err_msg,
                        })
                        .await;
                    return;
                }
            }

            // Validate ETag with weak `W/` normalization; fall back to Last-Modified on 206
            let resp_etag_header = resp
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|v| v.to_str().ok());

            if let Some(resp_etag) = resp_etag_header {
                let norm_resp = normalize_etag(resp_etag);
                if let Some(ref expected) = self.etag {
                    let norm_exp = normalize_etag(expected);
                    if !norm_resp.is_empty() && !norm_exp.is_empty() && norm_resp != norm_exp {
                        warn!(
                            "Worker {}: Server ETag changed from '{}' to '{}'. Aborting worker.",
                            self.idx, norm_exp, norm_resp
                        );
                        self.set_status(WorkerStatus::Failed);
                        let _ = event_tx
                            .send(AdaptiveWorkerEvent::ETagChanged {
                                idx: self.idx,
                                old_etag: expected.trim().to_string(),
                                new_etag: resp_etag.trim().to_string(),
                            })
                            .await;
                        return;
                    }
                }
            } else if status == StatusCode::PARTIAL_CONTENT {
                if let Some(resp_lm) = resp
                    .headers()
                    .get(LAST_MODIFIED)
                    .and_then(|v| v.to_str().ok())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    if let Some(ref expected_lm) = expected_last_modified {
                        if resp_lm != expected_lm.trim() {
                            warn!(
                                "Worker {}: Server Last-Modified changed from '{}' to '{}'. Aborting worker.",
                                self.idx, expected_lm, resp_lm
                            );
                            self.set_status(WorkerStatus::Failed);
                            let _ = event_tx
                                .send(AdaptiveWorkerEvent::LastModifiedChanged {
                                    idx: self.idx,
                                    old_last_modified: expected_lm.clone(),
                                    new_last_modified: resp_lm.to_string(),
                                })
                                .await;
                            return;
                        }
                    } else {
                        expected_last_modified = Some(resp_lm.to_string());
                    }
                }
            }

            // Handle server returning 200 OK (ignoring Range)
            if status == StatusCode::OK {
                if self.start > 0 {
                    warn!(
                        "Worker {}: Server returned 200 OK instead of 206 Partial Content. Aborting secondary worker to prevent overwrite.",
                        self.idx
                    );
                    self.set_status(WorkerStatus::Failed);
                    let _ = event_tx
                        .send(AdaptiveWorkerEvent::ServerIgnoredRange { idx: self.idx })
                        .await;
                    return;
                } else {
                    warn!(
                        "Worker 0: Server returned 200 OK. Resetting offset to 0 and expanding to full body stream."
                    );
                    self.pos.store(0, Ordering::SeqCst);
                    if let Some(content_len) = resp.content_length() {
                        if content_len > 0 {
                            self.end.store(content_len - 1, Ordering::SeqCst);
                        }
                    }
                    let _ = event_tx
                        .send(AdaptiveWorkerEvent::ServerIgnoredRange { idx: self.idx })
                        .await;
                }
            }

            // Validate Content-Range header (start, end, and /total vs total_size) for 206
            if status == StatusCode::PARTIAL_CONTENT {
                if let Some(cr_header) = resp.headers().get(reqwest::header::CONTENT_RANGE) {
                    if let Ok(cr_str) = cr_header.to_str() {
                        if let Some((cr_start, cr_end, cr_total)) = parse_content_range(cr_str) {
                            if let (Some(expected_total), Some(actual_total)) =
                                (self.total_size, cr_total)
                            {
                                if expected_total != actual_total {
                                    let err_msg = format!(
                                        "Content-Range total size mismatch (expected {}, got {})",
                                        expected_total, actual_total
                                    );
                                    warn!("Worker {}: {}", self.idx, err_msg);
                                    self.set_status(WorkerStatus::Failed);
                                    let _ = event_tx
                                        .send(AdaptiveWorkerEvent::Failed {
                                            idx: self.idx,
                                            reason: err_msg,
                                        })
                                        .await;
                                    return;
                                }
                            }
                            if cr_start != current_pos {
                                warn!(
                                    "Worker {}: Content-Range start mismatch (expected {}, got {}). Retrying.",
                                    self.idx, current_pos, cr_start
                                );
                                retries += 1;
                                self.retry_count.store(retries, Ordering::Relaxed);
                                drop(_host_permit);
                                if sleep_or_cancel(
                                    &self.cancelled,
                                    jittered_backoff(self.idx, retries),
                                )
                                .await
                                {
                                    return;
                                }
                                continue;
                            }
                            if cr_end < current_end {
                                debug!(
                                    "Worker {}: Server truncated range end to {}.",
                                    self.idx, cr_end
                                );
                                self.end.fetch_min(cr_end, Ordering::SeqCst);
                            }
                        }
                    }
                }
            }

            self.set_status(WorkerStatus::Downloading);
            self.is_reconnecting.store(false, Ordering::Relaxed);
            let mut stream = resp.bytes_stream();
            let mut staging = BytesMut::with_capacity(WRITE_STAGING_THRESHOLD);
            let mut staging_since = Instant::now();
            let mut attempt_bytes = 0u64;
            let mut read_failed = false;

            while !self.cancelled.load(Ordering::Relaxed) {
                let chunk_result = tokio::select! {
                    _ = wait_for_cancel(&self.cancelled) => None,
                    res = timeout(stall_timeout, stream.next()) => match res {
                        Ok(Some(Ok(bytes))) => Some(bytes),
                        Ok(Some(Err(e))) => {
                            warn!("Worker {} read error: {}", self.idx, e);
                            read_failed = true;
                            None
                        }
                        Ok(None) => None, // EOF
                        Err(_) => {
                            warn!("Worker {} stalled (no data for {:?})", self.idx, stall_timeout);
                            self.set_status(WorkerStatus::Stalled);
                            let _ = event_tx
                                .send(AdaptiveWorkerEvent::Stalled { idx: self.idx })
                                .await;
                            read_failed = true;
                            None
                        }
                    }
                };

                let Some(next_chunk) = chunk_result else {
                    break;
                };

                if next_chunk.is_empty() {
                    continue;
                }

                let chunk_len = next_chunk.len() as u64;
                if let Some(ref lim) = limiter {
                    lim.acquire(chunk_len).await;
                }

                if staging.is_empty() {
                    staging_since = Instant::now();
                }
                staging.extend_from_slice(&next_chunk);
                attempt_bytes = attempt_bytes.saturating_add(chunk_len);
                self.mark_activity();

                if attempt_bytes >= SUSTAINED_PROGRESS_RESET_BYTES && retries > 0 {
                    retries = 0;
                    self.retry_count.store(0, Ordering::Relaxed);
                }

                let cur_pos = self.pos.load(Ordering::SeqCst);
                let cur_end = self.end.load(Ordering::SeqCst);
                let reaches_end = cur_pos.saturating_add(staging.len() as u64) > cur_end;

                if staging.len() >= WRITE_STAGING_THRESHOLD
                    || reaches_end
                    || staging_since.elapsed() >= MAX_STAGING_AGE
                {
                    match self.flush_staging(&mut staging, &writer, &event_tx).await {
                        FlushOutcome::Continue => {}
                        FlushOutcome::HitBoundary => break,
                        FlushOutcome::RetryableError => {
                            read_failed = true;
                            break;
                        }
                        FlushOutcome::FatalDiskFull => return,
                    }
                }
            }

            // Flush any residual buffered bytes on stream end, error, or cancel
            if !staging.is_empty() {
                match self.flush_staging(&mut staging, &writer, &event_tx).await {
                    FlushOutcome::Continue | FlushOutcome::HitBoundary => {}
                    FlushOutcome::RetryableError => {
                        read_failed = true;
                    }
                    FlushOutcome::FatalDiskFull => return,
                }
            }

            if self.cancelled.load(Ordering::Relaxed) {
                return;
            }

            let pos_after = self.pos.load(Ordering::SeqCst);
            let end_after = self.end.load(Ordering::SeqCst);

            let is_completed = !read_failed && (pos_after > end_after || end_after >= u64::MAX - 2);

            if is_completed {
                self.set_status(WorkerStatus::Completed);
                let _ = event_tx
                    .send(AdaptiveWorkerEvent::Done { idx: self.idx })
                    .await;
                return;
            }

            let _ = event_tx
                .send(AdaptiveWorkerEvent::TransientError { idx: self.idx })
                .await;
            retries += 1;
            self.retry_count.store(retries, Ordering::Relaxed);
            drop(_host_permit);
            if sleep_or_cancel(&self.cancelled, jittered_backoff(self.idx, retries)).await {
                return;
            }
        }

        if self.cancelled.load(Ordering::Relaxed) {
            return;
        }

        self.set_status(WorkerStatus::Failed);
        let _ = event_tx
            .send(AdaptiveWorkerEvent::Failed {
                idx: self.idx,
                reason: format!("Worker {} exhausted {} retries", self.idx, retries),
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_range() {
        assert_eq!(
            parse_content_range("bytes 0-1048575/67108864"),
            Some((0, 1048575, Some(67108864)))
        );
        assert_eq!(
            parse_content_range("bytes 5242880-10485759/10485760"),
            Some((5242880, 10485759, Some(10485760)))
        );
        assert_eq!(
            parse_content_range("bytes 100-200/*"),
            Some((100, 200, None))
        );
        assert_eq!(parse_content_range("invalid-header"), None);
        assert_eq!(parse_content_range("bytes invalid/100"), None);
    }

    #[test]
    fn test_normalize_weak_and_strong_etags() {
        assert_eq!(normalize_etag("W/\"abc123\""), "abc123");
        assert_eq!(normalize_etag("w/\"abc123\""), "abc123");
        assert_eq!(normalize_etag("\"abc123\""), "abc123");
        assert_eq!(normalize_etag("  W/\"xyz\" "), "xyz");
    }

    #[test]
    fn test_parse_retry_after_int_and_http_date() {
        assert_eq!(parse_retry_after("15"), Some(Duration::from_secs(15)));
        assert_eq!(parse_retry_after("999"), Some(Duration::from_secs(120)));

        let future = (Utc::now() + chrono::Duration::seconds(42)).to_rfc2822();
        let parsed = parse_retry_after(&future).expect("rfc2822 future");
        assert!(parsed >= Duration::from_secs(38) && parsed <= Duration::from_secs(45));

        let far_future = (Utc::now() + chrono::Duration::seconds(500)).to_rfc2822();
        assert_eq!(
            parse_retry_after(&far_future),
            Some(Duration::from_secs(120))
        );

        let past = "Wed, 21 Oct 2015 07:28:00 GMT";
        assert_eq!(parse_retry_after(past), Some(Duration::from_secs(1)));
    }

    #[test]
    fn test_claim_write_window_prevents_steal_race_and_underflow() {
        let w = AdaptiveWorker::new(
            0,
            "http://localhost/file".into(),
            0,
            999,
            0,
            None,
            None,
            None,
            None,
        );
        assert!(w.claim_write_window(0).is_none());

        let (off, len, hit) = w.claim_write_window(400).unwrap();
        assert_eq!((off, len, hit), (0, 400, false));
        assert_eq!(w.pos.load(Ordering::SeqCst), 400);

        // Simulate steal shrinking end to 499 while worker asks for 300 bytes
        w.end.store(499, Ordering::SeqCst);
        let (off2, len2, hit2) = w.claim_write_window(300).unwrap();
        assert_eq!((off2, len2, hit2), (400, 100, true));
        assert_eq!(w.pos.load(Ordering::SeqCst), 500);
        assert!(w.claim_write_window(10).is_none());
    }
}
