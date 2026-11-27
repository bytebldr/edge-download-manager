use reqwest::Client;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::{AbortHandle, JoinHandle};
use tracing::info;

use super::pipeline_writer::PipelinedWriterTx;
use super::types::{WorkerStatus, WorkerTelemetry};
use super::worker::{AdaptiveWorker, AdaptiveWorkerEvent};
use crate::limiter::TokenBucketLimiter;

const MIN_TAIL_CHUNK_BYTES: u64 = 64 * 1024; // 64 KiB micro-chunk
const STEAL_MIN_VICTIM_ETA_SECS: f64 = 0.75; // Avoid splitting warm sockets finishing in <750ms
const HEDGE_MIN_VICTIM_ETA_SECS: f64 = 1.0; // Hedge stragglers with >=1s ETA or stalled
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug)]
struct SnapshotState {
    last_bytes: u64,
    last_ms: u64,
}

#[derive(Debug)]
struct WorkerRuntime {
    join_handle: Option<JoinHandle<()>>,
    abort_handle: Option<AbortHandle>,
    last_progress_at: Instant,
    last_observed_pos: u64,
    last_observed_bytes: u64,
    hedged_for: Option<u32>,
}

pub struct WorkScheduler {
    pub workers: Vec<Arc<AdaptiveWorker>>,
    next_idx: u32,
    min_chunk_size: u64,
    stall_timeout: Duration,
    request_timeout: Duration,
    max_retries: u32,
    target_connections: u32,
    enable_hedged_requests: bool,
    max_hedged_workers: u32,
    hedged_requests_won: u32,
    started_at: Instant,
    last_modified: Option<String>,
    total_size: Option<u64>,
    paused_until_ms: Arc<AtomicU64>,
    host_semaphore: Option<Arc<Semaphore>>,
    runtime: HashMap<u32, WorkerRuntime>,
    snapshots: Mutex<HashMap<u32, SnapshotState>>,
    archived_workers: BTreeMap<u32, WorkerTelemetry>,
}

impl WorkScheduler {
    pub fn new(min_chunk_size: u64, stall_timeout: Duration, max_retries: u32) -> Self {
        Self {
            workers: Vec::new(),
            next_idx: 0,
            min_chunk_size,
            stall_timeout,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            max_retries,
            target_connections: 64,
            enable_hedged_requests: true,
            max_hedged_workers: 2,
            hedged_requests_won: 0,
            started_at: Instant::now(),
            last_modified: None,
            total_size: None,
            paused_until_ms: Arc::new(AtomicU64::new(0)),
            host_semaphore: None,
            runtime: HashMap::new(),
            snapshots: Mutex::new(HashMap::new()),
            archived_workers: BTreeMap::new(),
        }
    }

    /// Configures download-wide validators, host concurrency semaphore, pause clock, and request timeout
    /// applied to every spawned worker.
    pub fn set_download_context(
        &mut self,
        last_modified: Option<String>,
        total_size: Option<u64>,
        paused_until_ms: Arc<AtomicU64>,
        host_semaphore: Option<Arc<Semaphore>>,
        request_timeout: Duration,
    ) {
        self.last_modified = last_modified;
        self.total_size = total_size;
        self.paused_until_ms = paused_until_ms;
        self.host_semaphore = host_semaphore;
        self.request_timeout = request_timeout;
    }

    pub fn set_target_connections(&mut self, target: u32) {
        self.target_connections = target.max(1);
    }

    pub fn target_connections(&self) -> u32 {
        self.target_connections
    }

    pub fn set_tail_hedging(&mut self, enabled: bool, max_hedged_workers: u32) {
        self.enable_hedged_requests = enabled;
        self.max_hedged_workers = max_hedged_workers;
    }

    pub fn set_request_timeout(&mut self, timeout: Duration) {
        self.request_timeout = timeout;
    }

    pub fn set_paused_until(&mut self, paused_until_ms: Arc<AtomicU64>) {
        self.paused_until_ms = paused_until_ms;
    }

    pub fn set_host_semaphore(&mut self, semaphore: Option<Arc<Semaphore>>) {
        self.host_semaphore = semaphore;
    }

    pub fn hedged_requests_won(&self) -> u32 {
        self.hedged_requests_won
    }

    /// Monotonic elapsed milliseconds since this scheduler was created.
    pub fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }

    /// Spawns an initial worker for a byte range.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_worker(
        &mut self,
        url: String,
        start: u64,
        end: u64,
        etag: Option<String>,
        cookies: Option<String>,
        referer: Option<String>,
        user_agent: Option<String>,
        client: Client,
        writer: PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: mpsc::Sender<AdaptiveWorkerEvent>,
    ) -> Arc<AdaptiveWorker> {
        self.spawn_worker_at(
            url, start, end, start, etag, cookies, referer, user_agent, client, writer, limiter,
            event_tx,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_worker_at(
        &mut self,
        url: String,
        start: u64,
        end: u64,
        pos: u64,
        etag: Option<String>,
        cookies: Option<String>,
        referer: Option<String>,
        user_agent: Option<String>,
        client: Client,
        writer: PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: mpsc::Sender<AdaptiveWorkerEvent>,
    ) -> Arc<AdaptiveWorker> {
        self.spawn_worker_internal(
            url, start, end, pos, etag, cookies, referer, user_agent, client, writer, limiter,
            event_tx, 0, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_worker_internal(
        &mut self,
        url: String,
        start: u64,
        end: u64,
        pos: u64,
        etag: Option<String>,
        cookies: Option<String>,
        referer: Option<String>,
        user_agent: Option<String>,
        client: Client,
        writer: PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: mpsc::Sender<AdaptiveWorkerEvent>,
        initial_retries: u32,
        hedged_for: Option<u32>,
    ) -> Arc<AdaptiveWorker> {
        let idx = self.next_idx;
        self.next_idx += 1;

        let initial_pos = pos.clamp(start, end.saturating_add(1));
        let worker = Arc::new(
            AdaptiveWorker::new(
                idx,
                url,
                start,
                end,
                initial_pos,
                etag,
                cookies,
                referer,
                user_agent,
            )
            .with_last_modified(self.last_modified.clone())
            .with_total_size(self.total_size)
            .with_paused_until(self.paused_until_ms.clone())
            .with_host_semaphore(self.host_semaphore.clone())
            .with_hedged(hedged_for.is_some()),
        );
        if initial_retries > 0 {
            worker.retry_count.store(initial_retries, Ordering::Relaxed);
        }

        self.workers.push(worker.clone());

        let now = Instant::now();
        let now_ms = self.elapsed_ms();
        self.snapshots
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                idx,
                SnapshotState {
                    last_bytes: 0,
                    last_ms: now_ms,
                },
            );

        let w_clone = worker.clone();
        let stall = self.stall_timeout;
        let req_timeout = self.request_timeout;
        let retries = self.max_retries;

        let join_handle = tokio::spawn(async move {
            w_clone
                .run(
                    client,
                    writer,
                    limiter,
                    event_tx,
                    stall,
                    req_timeout,
                    retries,
                )
                .await;
        });
        let abort_handle = join_handle.abort_handle();

        self.runtime.insert(
            idx,
            WorkerRuntime {
                join_handle: Some(join_handle),
                abort_handle: Some(abort_handle),
                last_progress_at: now,
                last_observed_pos: initial_pos,
                last_observed_bytes: 0,
                hedged_for,
            },
        );

        worker
    }

    /// Atomically steals the tail half of a victim worker's remaining byte range `[pos, end]`
    /// if the split satisfies `min_chunk` on both sides.
    pub fn claim_next_range(&self, victim_idx: u32, min_chunk: u64) -> Option<(u64, u64)> {
        let target = self.workers.iter().find(|w| w.idx == victim_idx)?;
        let status = target.get_status();
        if status == WorkerStatus::Completed
            || status == WorkerStatus::Failed
            || target.cancelled.load(Ordering::Relaxed)
        {
            return None;
        }

        let cur_end = target.end.load(Ordering::SeqCst);
        let cur_pos = target.pos.load(Ordering::SeqCst);

        if cur_end <= cur_pos {
            return None;
        }
        let remaining = cur_end.saturating_sub(cur_pos).saturating_add(1);
        let min_split_total = min_chunk.saturating_mul(2);
        if remaining < min_split_total {
            return None;
        }

        let stolen_len = remaining / 2;
        if stolen_len < min_chunk || remaining.saturating_sub(stolen_len) < min_chunk {
            return None;
        }

        // Steal from tail: victim keeps [cur_pos ..= cur_end - stolen_len], thief gets [new_end + 1 ..= cur_end]
        let new_target_end = cur_end - stolen_len;

        if target
            .end
            .compare_exchange(cur_end, new_target_end, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }

        // Verify victim hasn't already crossed the new end boundary while CAS was executing
        let active_pos = target.pos.load(Ordering::SeqCst);
        if active_pos > new_target_end {
            let _ = target.end.compare_exchange(
                new_target_end,
                cur_end,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            return None;
        }

        Some((new_target_end + 1, cur_end))
    }

    fn victim_worth_splitting(&self, victim: &AdaptiveWorker, rem: u64, min_eta_secs: f64) -> bool {
        let status = victim.get_status();
        if status == WorkerStatus::Stalled {
            return true;
        }
        let speed_bytes_sec = victim.speed_bps.load(Ordering::Relaxed) / 8;
        if speed_bytes_sec > 0 {
            let eta_secs = rem as f64 / speed_bytes_sec as f64;
            return eta_secs >= min_eta_secs;
        }
        // Speed not yet measured (e.g. Connecting / early warmup): allow steal if worker has
        // been idle >= 500ms or remaining work exceeds min_chunk_size
        let idle_ms = self
            .runtime
            .get(&victim.idx)
            .map(|rt| rt.last_progress_at.elapsed().as_millis() as u64)
            .unwrap_or(500);
        idle_ms >= 500 || rem >= self.min_chunk_size
    }

    fn find_best_steal_candidate(
        &self,
        min_remaining: u64,
        max_remaining: u64,
        min_eta_secs: Option<f64>,
    ) -> Option<(u32, u64)> {
        let mut best: Option<(u32, u64)> = None;
        for w in &self.workers {
            let s = w.get_status();
            if s == WorkerStatus::Completed
                || s == WorkerStatus::Failed
                || w.is_hedged
                || w.cancelled.load(Ordering::Relaxed)
            {
                continue;
            }
            let rem = w.remaining_bytes();
            if rem < min_remaining || rem > max_remaining {
                continue;
            }
            if let Some(eta_gate) = min_eta_secs {
                if !self.victim_worth_splitting(w, rem, eta_gate) {
                    continue;
                }
            }
            match best {
                None => best = Some((w.idx, rem)),
                Some((_, best_rem)) if rem > best_rem => best = Some((w.idx, rem)),
                _ => {}
            }
        }
        best
    }

    fn spawn_stolen_worker(
        &mut self,
        victim_idx: u32,
        new_start: u64,
        new_end: u64,
        client: Client,
        writer: PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: mpsc::Sender<AdaptiveWorkerEvent>,
    ) -> Option<Arc<AdaptiveWorker>> {
        let target = self.workers.iter().find(|w| w.idx == victim_idx)?.clone();
        let new_worker = self.spawn_worker(
            target.url.clone(),
            new_start,
            new_end,
            target.etag.clone(),
            target.cookies.clone(),
            target.referer.clone(),
            target.user_agent.clone(),
            client,
            writer,
            limiter,
            event_tx,
        );
        Some(new_worker)
    }

    /// Attempts dynamic work stealing within the `target_connections` budget:
    /// prefers splitting ranges >= `2 * min_chunk_size`, and falls back seamlessly into
    /// micro-chunk splitting (`>= 2 * 64 KiB`) so there is no deadzone below `2 * min_chunk_size`.
    pub fn try_steal_work(
        &mut self,
        client: Client,
        writer: PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: mpsc::Sender<AdaptiveWorkerEvent>,
    ) -> Option<Arc<AdaptiveWorker>> {
        if self.active_count() >= self.target_connections {
            return None;
        }

        let standard_min = self.min_chunk_size.saturating_mul(2);
        let candidate = self
            .find_best_steal_candidate(standard_min, u64::MAX, None)
            .map(|(idx, rem)| (idx, rem, self.min_chunk_size))
            .or_else(|| {
                // Eliminate split deadzone between 2 * MIN_TAIL_CHUNK and 2 * min_chunk_size
                self.find_best_steal_candidate(
                    MIN_TAIL_CHUNK_BYTES * 2,
                    u64::MAX,
                    Some(STEAL_MIN_VICTIM_ETA_SECS),
                )
                .map(|(idx, rem)| (idx, rem, MIN_TAIL_CHUNK_BYTES))
            });

        let (victim_idx, _, split_min) = candidate?;
        let (new_start, new_end) = self.claim_next_range(victim_idx, split_min)?;

        info!(
            "Dynamic Work Steal: Worker {} split -> spawning Worker {} for {}-{} ({} KiB stolen)",
            victim_idx,
            self.next_idx,
            new_start,
            new_end,
            new_end.saturating_sub(new_start).saturating_add(1) / 1024
        );

        self.spawn_stolen_worker(
            victim_idx, new_start, new_end, client, writer, limiter, event_tx,
        )
    }

    /// Smart Tail Latency Sniper:
    /// Splits a straggling tail range (`<= max(tail_threshold_bytes, 2 * min_chunk_size)`)
    /// down to 64 KiB micro-chunks when victim ETA exceeds TLS handshake overhead (~750ms)
    /// and within `target_connections` budget.
    pub fn try_tail_snipe(
        &mut self,
        client: Client,
        writer: PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: mpsc::Sender<AdaptiveWorkerEvent>,
        tail_threshold_bytes: u64,
    ) -> Option<Arc<AdaptiveWorker>> {
        if self.active_count() >= self.target_connections {
            return None;
        }

        let effective_threshold = tail_threshold_bytes.max(self.min_chunk_size.saturating_mul(2));
        let (victim_idx, rem) = self.find_best_steal_candidate(
            MIN_TAIL_CHUNK_BYTES * 2,
            effective_threshold,
            Some(STEAL_MIN_VICTIM_ETA_SECS),
        )?;

        let (new_start, new_end) = self.claim_next_range(victim_idx, MIN_TAIL_CHUNK_BYTES)?;

        info!(
            "Speculative Tail Latency Sniper: Worker {} straggling with {} KiB left -> Sniped tail range {}-{} ({} KiB) to fresh socket",
            victim_idx,
            rem / 1024,
            new_start,
            new_end,
            new_end.saturating_sub(new_start).saturating_add(1) / 1024
        );

        self.spawn_stolen_worker(
            victim_idx, new_start, new_end, client, writer, limiter, event_tx,
        )
    }

    fn active_hedged_count(&self) -> u32 {
        self.workers
            .iter()
            .filter(|w| {
                w.is_hedged && !w.cancelled.load(Ordering::Relaxed) && {
                    let s = w.get_status();
                    s == WorkerStatus::Connecting
                        || s == WorkerStatus::Downloading
                        || s == WorkerStatus::Stalled
                }
            })
            .count() as u32
    }

    fn is_worker_hedged(&self, victim_idx: u32) -> bool {
        self.runtime
            .values()
            .any(|rt| rt.hedged_for == Some(victim_idx))
            && self.workers.iter().any(|w| {
                let s = w.get_status();
                !w.cancelled.load(Ordering::Relaxed)
                    && (s == WorkerStatus::Connecting
                        || s == WorkerStatus::Downloading
                        || s == WorkerStatus::Stalled)
                    && self.runtime.get(&w.idx).and_then(|rt| rt.hedged_for) == Some(victim_idx)
            })
    }

    /// Speculative Tail Hedging:
    /// Races a duplicate request on a fresh socket for a straggling tail worker without killing
    /// the warm primary socket. Gated by `enable_hedged_requests`, `max_hedged_workers`,
    /// `target_connections + max_hedged_workers`, and victim ETA >= 1.0s.
    pub fn try_hedge_tail(
        &mut self,
        client: Client,
        writer: PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: mpsc::Sender<AdaptiveWorkerEvent>,
        tail_threshold_bytes: u64,
    ) -> Option<Arc<AdaptiveWorker>> {
        if !self.enable_hedged_requests || self.max_hedged_workers == 0 {
            return None;
        }
        if self.active_hedged_count() >= self.max_hedged_workers {
            return None;
        }
        if self.active_count()
            >= self
                .target_connections
                .saturating_add(self.max_hedged_workers)
        {
            return None;
        }

        let effective_threshold = tail_threshold_bytes.max(self.min_chunk_size.saturating_mul(2));
        let mut best: Option<(u32, u64)> = None;

        for w in &self.workers {
            if w.is_hedged || w.cancelled.load(Ordering::Relaxed) {
                continue;
            }
            let s = w.get_status();
            if s == WorkerStatus::Completed || s == WorkerStatus::Failed {
                continue;
            }
            let rem = w.remaining_bytes();
            if rem == 0 || rem > effective_threshold {
                continue;
            }
            if self.is_worker_hedged(w.idx) {
                continue;
            }
            if !self.victim_worth_splitting(w, rem, HEDGE_MIN_VICTIM_ETA_SECS) {
                continue;
            }
            match best {
                None => best = Some((w.idx, rem)),
                Some((_, best_rem)) if rem > best_rem => best = Some((w.idx, rem)),
                _ => {}
            }
        }

        let (victim_idx, rem) = best?;
        let target = self.workers.iter().find(|w| w.idx == victim_idx)?.clone();
        let cur_pos = target.pos.load(Ordering::SeqCst);
        let cur_end = target.end.load(Ordering::SeqCst);
        if cur_pos > cur_end {
            return None;
        }

        info!(
            "Speculative Tail Hedge: Worker {} has {} KiB remaining -> hedging range {}-{} on Worker {}",
            victim_idx,
            rem / 1024,
            cur_pos,
            cur_end,
            self.next_idx
        );

        let hedge_worker = self.spawn_worker_internal(
            target.url.clone(),
            cur_pos,
            cur_end,
            cur_pos,
            target.etag.clone(),
            target.cookies.clone(),
            target.referer.clone(),
            target.user_agent.clone(),
            client,
            writer,
            limiter,
            event_tx,
            0,
            Some(victim_idx),
        );

        Some(hedge_worker)
    }

    /// Notifies the scheduler when a worker finishes; cancels the twin if part of a hedged race.
    /// Returns `true` if a speculative hedged worker won the race.
    pub fn on_worker_finished(&mut self, idx: u32) -> bool {
        // Case 1: `idx` itself is a hedged worker that finished first
        if let Some(victim_idx) = self.runtime.get(&idx).and_then(|rt| rt.hedged_for) {
            if let Some(victim) = self.workers.iter().find(|w| w.idx == victim_idx) {
                let s = victim.get_status();
                if s != WorkerStatus::Completed && s != WorkerStatus::Failed {
                    victim.cancelled.store(true, Ordering::SeqCst);
                    victim.set_status(WorkerStatus::Completed);
                    self.hedged_requests_won = self.hedged_requests_won.saturating_add(1);
                    return true;
                }
            }
            return false;
        }

        // Case 2: `idx` is a primary victim whose hedged twin is still running -> cancel twin
        let twin_indices: Vec<u32> = self
            .runtime
            .iter()
            .filter_map(|(&h_idx, rt)| (rt.hedged_for == Some(idx)).then_some(h_idx))
            .collect();

        for h_idx in twin_indices {
            if let Some(twin) = self.workers.iter().find(|w| w.idx == h_idx) {
                twin.cancelled.store(true, Ordering::SeqCst);
                twin.set_status(WorkerStatus::Completed);
            }
            if let Some(rt) = self.runtime.get_mut(&h_idx) {
                if let Some(abort) = rt.abort_handle.take() {
                    abort.abort();
                }
            }
        }
        false
    }

    /// Clears hedge pairing when a worker fails without cancelling the surviving twin/primary
    /// or incrementing `hedged_requests_won`.
    pub fn on_worker_failed(&mut self, idx: u32) {
        if let Some(rt) = self.runtime.get_mut(&idx) {
            rt.hedged_for = None;
        }
        for rt in self.runtime.values_mut() {
            if rt.hedged_for == Some(idx) {
                rt.hedged_for = None;
            }
        }
    }

    /// Returns `true` when active workers exceed `target` (e.g. after `ScaleDown`).
    pub fn should_retire_worker(&self, target: u32) -> bool {
        self.active_count() > target.max(1)
    }

    /// Signals excess speculative hedged workers to cancel gracefully (`ScaleDown` / `LimitRate`)
    /// and lowers `target_connections` so non-hedged workers wind down naturally upon completing
    /// their current ranges without ever stranding unowned byte ranges.
    pub fn retire_excess_workers(&mut self, target: u32) -> u32 {
        let clamped_target = target.max(1);
        self.target_connections = clamped_target;
        let active = self.active_count();
        if active <= clamped_target {
            return 0;
        }
        let excess = (active - clamped_target) as usize;

        // Only cancel duplicate hedged workers immediately; non-hedged workers keep their
        // exclusive byte ranges and retire naturally when they finish without replacement.
        let mut candidates: Vec<(u64, u32)> = self
            .workers
            .iter()
            .filter_map(|w| {
                if !w.is_hedged || w.cancelled.load(Ordering::Relaxed) {
                    return None;
                }
                let s = w.get_status();
                if s == WorkerStatus::Completed || s == WorkerStatus::Failed {
                    return None;
                }
                Some((w.bytes_downloaded.load(Ordering::Relaxed), w.idx))
            })
            .collect();

        candidates.sort_unstable();

        let mut retired = 0u32;
        for (_, idx) in candidates.into_iter().take(excess) {
            if let Some(w) = self.workers.iter().find(|w| w.idx == idx) {
                w.cancelled.store(true, Ordering::SeqCst);
                retired += 1;
            }
        }
        retired
    }

    /// Checks for stalled workers using monotonic `Instant` timers, aborts hung Tokio tasks,
    /// and respawns a replacement worker for the uncompleted `[pos..=end]` range up to `max_retries`.
    /// Skips cancelled workers and workers currently blocked on disk backpressure (`is_writing_disk`).
    /// In tail phase, stall sensitivity is accelerated to 2s to prevent tail hangs.
    pub fn check_stalled_workers_adaptive(
        &mut self,
        client: &Client,
        writer: &PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: &mpsc::Sender<AdaptiveWorkerEvent>,
        is_tail_phase: bool,
    ) {
        let timeout_secs = if is_tail_phase {
            self.stall_timeout.as_secs().clamp(4, 10)
        } else {
            self.stall_timeout.as_secs().max(10)
        };
        let timeout_dur = Duration::from_secs(timeout_secs);
        let now = Instant::now();

        let mut stalled_indices: Vec<u32> = Vec::new();

        for w in &self.workers {
            let status = w.get_status();
            if status == WorkerStatus::Completed
                || status == WorkerStatus::Failed
                || w.cancelled.load(Ordering::SeqCst)
            {
                continue;
            }

            let cur_pos = w.pos.load(Ordering::Relaxed);
            let cur_bytes = w.bytes_downloaded.load(Ordering::Relaxed);
            let rt = self.runtime.entry(w.idx).or_insert_with(|| WorkerRuntime {
                join_handle: None,
                abort_handle: None,
                last_progress_at: now,
                last_observed_pos: cur_pos,
                last_observed_bytes: cur_bytes,
                hedged_for: None,
            });

            if w.is_writing_disk.load(Ordering::Acquire) {
                rt.last_progress_at = now;
                continue;
            }

            if cur_pos != rt.last_observed_pos || cur_bytes != rt.last_observed_bytes {
                rt.last_observed_pos = cur_pos;
                rt.last_observed_bytes = cur_bytes;
                rt.last_progress_at = now;
                continue;
            }

            if now.saturating_duration_since(rt.last_progress_at) > timeout_dur {
                stalled_indices.push(w.idx);
            }
        }

        for idx in stalled_indices {
            let Some(pos_in_vec) = self.workers.iter().position(|w| w.idx == idx) else {
                continue;
            };
            let w = self.workers[pos_in_vec].clone();

            w.cancelled.store(true, Ordering::SeqCst);
            if let Some(rt) = self.runtime.get_mut(&idx) {
                if let Some(abort) = rt.abort_handle.take() {
                    abort.abort();
                }
            }

            let cur_pos = w.pos.load(Ordering::SeqCst);
            let cur_end = w.end.load(Ordering::SeqCst);
            if cur_pos > cur_end {
                w.set_status(WorkerStatus::Completed);
                continue;
            }

            // If a hedged worker stalled, simply retire it without spawning a retry loop
            if w.is_hedged {
                w.set_status(WorkerStatus::Failed);
                continue;
            }

            let prev_retries = w.retry_count.fetch_add(1, Ordering::Relaxed);
            if prev_retries >= self.max_retries {
                w.set_status(WorkerStatus::Failed);
                let _ = event_tx.try_send(AdaptiveWorkerEvent::Failed {
                    idx: w.idx,
                    reason: format!(
                        "Worker {} stalled after {} retry attempts",
                        w.idx, prev_retries
                    ),
                });
                continue;
            }

            info!(
                "Stall detected on Worker {} at pos {} (timeout: {}s, tail: {}). Aborting hung task and respawning (attempt {}/{}).",
                w.idx,
                cur_pos,
                timeout_secs,
                is_tail_phase,
                prev_retries + 1,
                self.max_retries
            );

            w.set_status(WorkerStatus::Stalled);
            let _ = event_tx.try_send(AdaptiveWorkerEvent::Stalled { idx: w.idx });

            // Retire the aborted worker slot before spawning the clean replacement
            self.workers.remove(pos_in_vec);
            self.runtime.remove(&idx);
            self.snapshots
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&idx);

            self.spawn_worker_internal(
                w.url.clone(),
                w.start,
                cur_end,
                cur_pos,
                w.etag.clone(),
                w.cookies.clone(),
                w.referer.clone(),
                w.user_agent.clone(),
                client.clone(),
                writer.clone(),
                limiter.clone(),
                event_tx.clone(),
                prev_retries + 1,
                None,
            );
        }
    }

    /// Checks for stalled workers and aborts/respawns hung tasks.
    pub fn check_stalled_workers(
        &mut self,
        client: &Client,
        writer: &PipelinedWriterTx,
        limiter: Option<TokenBucketLimiter>,
        event_tx: &mpsc::Sender<AdaptiveWorkerEvent>,
    ) {
        self.check_stalled_workers_adaptive(client, writer, limiter, event_tx, false);
    }

    fn archive_worker_if_transferred(&mut self, w: &AdaptiveWorker) {
        let bytes = w.bytes_downloaded.load(Ordering::Relaxed);
        if bytes == 0 {
            return;
        }
        let status = match w.get_status() {
            WorkerStatus::Connecting | WorkerStatus::Downloading | WorkerStatus::Stalled => {
                WorkerStatus::Completed
            }
            other => other,
        };
        self.archived_workers.insert(
            w.idx,
            WorkerTelemetry {
                idx: w.idx,
                start: w.start,
                end: w.end.load(Ordering::Relaxed),
                pos: w.pos.load(Ordering::Relaxed),
                bytes_downloaded: bytes,
                speed_bps: 0,
                status,
                retry_count: w.retry_count.load(Ordering::Relaxed),
            },
        );
    }

    /// Prunes a specific worker by `idx` (e.g. a failed worker after its transient error
    /// has been recorded) from `workers` and runtime maps.
    pub fn prune_worker(&mut self, idx: u32) -> bool {
        if let Some(w) = self.workers.iter().find(|w| w.idx == idx).cloned() {
            self.archive_worker_if_transferred(&w);
        }
        let before = self.workers.len();
        self.workers.retain(|w| w.idx != idx);
        let removed = self.workers.len() < before;
        if removed {
            self.runtime.remove(&idx);
            self.snapshots
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&idx);
        }
        removed
    }

    /// Prunes finished (`Completed` / `Failed` / `cancelled`) workers from `workers` and runtime maps
    /// once their downloaded byte ranges have been folded into `RangeSet`.
    pub fn prune_finished_workers(&mut self) -> usize {
        let before = self.workers.len();
        let mut removed_ids = Vec::new();
        let mut to_archive = Vec::new();
        self.workers.retain(|w| {
            let s = w.get_status();
            let keep = s != WorkerStatus::Completed
                && s != WorkerStatus::Failed
                && !w.cancelled.load(Ordering::Relaxed);
            if !keep {
                removed_ids.push(w.idx);
                to_archive.push(w.clone());
            }
            keep
        });

        for w in &to_archive {
            self.archive_worker_if_transferred(w);
        }

        if !removed_ids.is_empty() {
            let mut snaps = self.snapshots.lock().unwrap_or_else(|p| p.into_inner());
            for idx in &removed_ids {
                self.runtime.remove(idx);
                snaps.remove(idx);
            }
        }

        before.saturating_sub(self.workers.len())
    }

    pub fn active_count(&self) -> u32 {
        self.workers
            .iter()
            .filter(|w| {
                if w.cancelled.load(Ordering::Relaxed) {
                    return false;
                }
                let s = w.get_status();
                s == WorkerStatus::Connecting
                    || s == WorkerStatus::Downloading
                    || s == WorkerStatus::Stalled
            })
            .count() as u32
    }

    pub fn active_error_count(&self) -> u32 {
        self.workers
            .iter()
            .filter(|w| w.get_status() == WorkerStatus::Failed)
            .count() as u32
    }

    pub fn cancel_all(&self) {
        for w in &self.workers {
            w.cancelled.store(true, Ordering::SeqCst);
        }
    }

    pub fn abort_all(&mut self) {
        self.cancel_all();
        for rt in self.runtime.values_mut() {
            if let Some(abort) = rt.abort_handle.take() {
                abort.abort();
            }
        }
    }

    pub fn cancel_all_except(&self, keep_idx: u32) {
        for w in &self.workers {
            if w.idx != keep_idx {
                w.cancelled.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Awaits all spawned worker tasks to finish after cancellation or completion,
    /// providing a clean join barrier before flushing/closing the writer or persisting state.
    pub async fn join_all(&mut self) {
        let mut handles = Vec::new();
        for rt in self.runtime.values_mut() {
            if let Some(jh) = rt.join_handle.take() {
                handles.push(jh);
            }
        }
        for jh in handles {
            let _ = jh.await;
        }
    }

    /// Returns a monotonic, read-safe telemetry snapshot without dropping bytes on `delta_t == 0`.
    /// Includes previously completed/pruned workers that transferred data alongside active workers.
    pub fn get_telemetry_snapshot(&self) -> Vec<WorkerTelemetry> {
        let now_ms = self.elapsed_ms();
        let mut snaps = self.snapshots.lock().unwrap_or_else(|p| p.into_inner());
        let mut archived: Vec<WorkerTelemetry> = self.archived_workers.values().cloned().collect();
        if archived.len() > 16 {
            archived.sort_by(|a, b| b.bytes_downloaded.cmp(&a.bytes_downloaded));
            archived.truncate(16);
        }
        let mut merged: BTreeMap<u32, WorkerTelemetry> =
            archived.into_iter().map(|w| (w.idx, w)).collect();

        for w in &self.workers {
            let start = w.start;
            let end = w.end.load(Ordering::Relaxed);
            let pos = w.pos.load(Ordering::Relaxed);
            let bytes = w.bytes_downloaded.load(Ordering::Relaxed);

            let state = snaps.entry(w.idx).or_insert(SnapshotState {
                last_bytes: bytes,
                last_ms: now_ms,
            });

            let delta_t = now_ms.saturating_sub(state.last_ms);
            let speed_bps = if delta_t > 0 {
                let delta_b = bytes.saturating_sub(state.last_bytes);
                state.last_bytes = bytes;
                state.last_ms = now_ms;
                let bps = delta_b.saturating_mul(8000) / delta_t;
                w.speed_bps.store(bps, Ordering::Relaxed);
                bps
            } else {
                // Read-safe on rapid calls within the same millisecond: keep last_bytes intact
                w.speed_bps.load(Ordering::Relaxed)
            };

            merged.insert(
                w.idx,
                WorkerTelemetry {
                    idx: w.idx,
                    start,
                    end,
                    pos,
                    bytes_downloaded: bytes,
                    speed_bps,
                    status: w.get_status(),
                    retry_count: w.retry_count.load(Ordering::Relaxed),
                },
            );
        }

        merged.into_values().collect()
    }

    /// Returns the final telemetry snapshot on download completion, preserving all workers
    /// that actually transferred data (including mid-download scaled and completed workers).
    pub fn get_final_telemetry_snapshot(&self) -> Vec<WorkerTelemetry> {
        let mut archived: Vec<WorkerTelemetry> = self.archived_workers.values().cloned().collect();
        if archived.len() > 16 {
            archived.sort_by(|a, b| b.bytes_downloaded.cmp(&a.bytes_downloaded));
            archived.truncate(16);
        }
        let mut merged: BTreeMap<u32, WorkerTelemetry> =
            archived.into_iter().map(|w| (w.idx, w)).collect();

        for w in &self.workers {
            let bytes = w.bytes_downloaded.load(Ordering::Relaxed);
            if bytes == 0 && !merged.is_empty() {
                continue;
            }
            let status = match w.get_status() {
                WorkerStatus::Failed => WorkerStatus::Failed,
                _ => WorkerStatus::Completed,
            };
            merged.insert(
                w.idx,
                WorkerTelemetry {
                    idx: w.idx,
                    start: w.start,
                    end: w.end.load(Ordering::Relaxed),
                    pos: w.pos.load(Ordering::Relaxed),
                    bytes_downloaded: bytes,
                    speed_bps: 0,
                    status,
                    retry_count: w.retry_count.load(Ordering::Relaxed),
                },
            );
        }

        let transferred: Vec<WorkerTelemetry> = merged
            .values()
            .filter(|w| w.bytes_downloaded > 0)
            .cloned()
            .collect();
        if transferred.is_empty() {
            merged.into_values().collect()
        } else {
            transferred
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dummy_worker(idx: u32, start: u64, end: u64, pos: u64) -> Arc<AdaptiveWorker> {
        let w = Arc::new(AdaptiveWorker::new(
            idx,
            "http://localhost/test".into(),
            start,
            end,
            pos,
            None,
            None,
            None,
            None,
        ));
        w.set_status(WorkerStatus::Downloading);
        w
    }

    #[test]
    fn test_work_stealing_stress_1000_runs() {
        use crate::adaptive::range_set::RangeSet;

        let total_size = 100 * 1024 * 1024; // 100 MiB
        let min_chunk = 1024 * 1024; // 1 MiB

        let mut rng_state: u64 = 0xDEADBEEFCAFE;
        let mut next_rand = || {
            rng_state ^= rng_state >> 12;
            rng_state ^= rng_state << 25;
            rng_state ^= rng_state >> 27;
            rng_state.wrapping_mul(0x2545F4914F6CDD1D)
        };

        for run in 0..1000 {
            let mut scheduler = WorkScheduler::new(min_chunk, Duration::from_secs(3), 3);
            let initial_conns = (next_rand() % 8 + 1) as u32;
            let plans = crate::planner::Planner::plan(total_size, initial_conns, min_chunk);

            for plan in plans {
                let w = make_dummy_worker(scheduler.next_idx, plan.start, plan.end, plan.start);
                scheduler.next_idx += 1;
                scheduler.workers.push(w);
            }

            // Simulate up to 20 random steal and progress events per run
            let num_events = (next_rand() % 20 + 5) as usize;
            for _ in 0..num_events {
                if !scheduler.workers.is_empty() {
                    let w_idx = (next_rand() as usize) % scheduler.workers.len();
                    let w = &scheduler.workers[w_idx];
                    let cur_pos = w.pos.load(Ordering::SeqCst);
                    let cur_end = w.end.load(Ordering::SeqCst);
                    if cur_pos < cur_end {
                        let advance = (next_rand() % (cur_end - cur_pos + 1)).min(512 * 1024);
                        w.pos.fetch_add(advance, Ordering::SeqCst);
                    }
                }

                if let Some((victim_idx, _)) = scheduler.find_best_steal_candidate(
                    2 * scheduler.min_chunk_size,
                    u64::MAX,
                    None,
                ) {
                    if let Some((new_start, new_end)) =
                        scheduler.claim_next_range(victim_idx, scheduler.min_chunk_size)
                    {
                        let new_w =
                            make_dummy_worker(scheduler.next_idx, new_start, new_end, new_start);
                        scheduler.next_idx += 1;
                        scheduler.workers.push(new_w);
                    }
                }

                let mut range_set = RangeSet::new();
                for w in &scheduler.workers {
                    let s = w.start;
                    let e = w.end.load(Ordering::SeqCst);
                    assert!(s <= e, "Run {}: worker start {} > end {}", run, s, e);
                    range_set.add(s, e);
                }

                assert!(
                    range_set.is_complete(total_size),
                    "Run {}: range_set is not complete! Gaps: {:?}",
                    run,
                    range_set.missing_ranges(total_size)
                );
                assert_eq!(
                    range_set.total_covered_bytes(),
                    total_size,
                    "Run {}: total covered bytes {} != expected {}",
                    run,
                    range_set.total_covered_bytes(),
                    total_size
                );
            }
        }
    }

    #[test]
    fn test_speculative_tail_sniper_disjoint_coverage() {
        use crate::adaptive::range_set::RangeSet;

        let total_size = 2 * 1024 * 1024; // 2 MiB tail
        let min_chunk = 1024 * 1024; // 1 MiB standard min chunk

        let mut scheduler = WorkScheduler::new(min_chunk, Duration::from_secs(3), 3);
        let w = make_dummy_worker(0, 0, total_size - 1, 0);
        scheduler.workers.push(w.clone());
        scheduler.next_idx = 1;

        // Advance worker to 1.5 MiB (512 KiB remaining tail)
        w.pos.store(1536 * 1024, Ordering::SeqCst);
        let rem = w.remaining_bytes();
        assert_eq!(rem, 512 * 1024);
        assert!(rem < 2 * scheduler.min_chunk_size);

        // Slow victim (100 KiB/s -> 5.12s ETA > 0.75s gate) should be eligible for tail snipe
        w.speed_bps.store(100 * 1024 * 8, Ordering::Relaxed);
        let candidate = scheduler.find_best_steal_candidate(
            MIN_TAIL_CHUNK_BYTES * 2,
            5 * 1024 * 1024,
            Some(STEAL_MIN_VICTIM_ETA_SECS),
        );
        assert_eq!(candidate, Some((0, 512 * 1024)));

        let (new_start, new_end) = scheduler
            .claim_next_range(0, MIN_TAIL_CHUNK_BYTES)
            .expect("claim tail range");

        let new_w = make_dummy_worker(scheduler.next_idx, new_start, new_end, new_start);
        scheduler.next_idx += 1;
        scheduler.workers.push(new_w);

        let mut range_set = RangeSet::new();
        for worker in &scheduler.workers {
            range_set.add(worker.start, worker.end.load(Ordering::SeqCst));
        }

        assert!(range_set.is_complete(total_size));
        assert_eq!(range_set.total_covered_bytes(), total_size);
    }

    #[test]
    fn test_fast_warm_socket_gated_and_deadzone_eliminated() {
        let min_chunk = 1024 * 1024; // 1 MiB -> 2*min_chunk = 2 MiB
        let mut scheduler = WorkScheduler::new(min_chunk, Duration::from_secs(3), 3);

        // Fast warm worker with 1.5 MiB remaining at 20 MB/s (ETA = 75ms < 750ms)
        let fast_w = make_dummy_worker(0, 0, 10 * 1024 * 1024 - 1, 8_704 * 1024);
        fast_w
            .speed_bps
            .store(20 * 1024 * 1024 * 8, Ordering::Relaxed);
        scheduler.workers.push(fast_w);

        // Fast socket must NOT be split
        assert_eq!(
            scheduler.find_best_steal_candidate(
                MIN_TAIL_CHUNK_BYTES * 2,
                u64::MAX,
                Some(STEAL_MIN_VICTIM_ETA_SECS),
            ),
            None
        );

        // Slow worker with 6 MiB remaining (would fall in 5 MiB..2*4 MiB deadzone before)
        let slow_w = make_dummy_worker(1, 0, 6 * 1024 * 1024 - 1, 0);
        slow_w.speed_bps.store(256 * 1024 * 8, Ordering::Relaxed); // 256 KiB/s -> 24s ETA
        scheduler.workers.push(slow_w);

        // Tail threshold 5 MiB with 4 MiB min_chunk -> effective threshold 8 MiB eliminates deadzone
        let mut deadzone_sched = WorkScheduler::new(4 * 1024 * 1024, Duration::from_secs(3), 3);
        let dz_w = make_dummy_worker(0, 0, 6 * 1024 * 1024 - 1, 0);
        dz_w.speed_bps.store(256 * 1024 * 8, Ordering::Relaxed);
        deadzone_sched.workers.push(dz_w);
        assert!(deadzone_sched
            .find_best_steal_candidate(
                MIN_TAIL_CHUNK_BYTES * 2,
                (5 * 1024 * 1024).max(2 * 4 * 1024 * 1024),
                Some(STEAL_MIN_VICTIM_ETA_SECS),
            )
            .is_some());
    }

    #[test]
    fn test_telemetry_snapshot_read_safe_on_zero_delta_t_and_prune() {
        let mut scheduler = WorkScheduler::new(512 * 1024, Duration::from_secs(3), 3);
        let w0 = make_dummy_worker(0, 0, 1023, 512);
        w0.bytes_downloaded.store(512, Ordering::Relaxed);
        scheduler.workers.push(w0.clone());

        // First call at t~0: delta_t may be 0; must not lose `bytes_downloaded` baseline
        let s1 = scheduler.get_telemetry_snapshot();
        assert_eq!(s1.len(), 1);

        // Simulate 100ms passing and more bytes arriving
        {
            let mut snaps = scheduler.snapshots.lock().unwrap();
            let entry = snaps.get_mut(&0).unwrap();
            entry.last_ms = 0;
            entry.last_bytes = 0;
        }
        std::thread::sleep(Duration::from_millis(20));
        w0.bytes_downloaded.store(1024, Ordering::Relaxed);
        let s2 = scheduler.get_telemetry_snapshot();
        assert!(s2[0].speed_bps > 0);

        // Completed worker is pruned cleanly, active worker retained
        let w1 = make_dummy_worker(1, 1024, 2047, 1024);
        scheduler.workers.push(w1);
        w0.set_status(WorkerStatus::Completed);
        let pruned = scheduler.prune_finished_workers();
        assert_eq!(pruned, 1);
        assert_eq!(scheduler.workers.len(), 1);
        assert_eq!(scheduler.workers[0].idx, 1);
    }

    #[test]
    fn test_retire_excess_workers_cancels_hedged_without_stranding_ranges() {
        let mut scheduler = WorkScheduler::new(512 * 1024, Duration::from_secs(3), 3);
        let w0 = make_dummy_worker(0, 0, 10_000, 5_000);
        w0.bytes_downloaded.store(5_000, Ordering::Relaxed);
        let w1 = make_dummy_worker(1, 10_001, 20_000, 10_500);
        w1.bytes_downloaded.store(500, Ordering::Relaxed);
        let w2_raw = AdaptiveWorker::new(
            2,
            "http://localhost/test".into(),
            0,
            10_000,
            5_000,
            None,
            None,
            None,
            None,
        )
        .with_hedged(true);
        w2_raw.set_status(WorkerStatus::Downloading);
        let w2 = Arc::new(w2_raw);

        scheduler
            .workers
            .extend([w0.clone(), w1.clone(), w2.clone()]);
        assert!(scheduler.should_retire_worker(1));
        assert_eq!(scheduler.active_count(), 3);

        // Retires the speculative hedged worker immediately, never strands non-hedged ranges w0/w1
        let retired = scheduler.retire_excess_workers(1);
        assert_eq!(retired, 1);
        assert!(w2.cancelled.load(Ordering::Relaxed));
        assert!(!w1.cancelled.load(Ordering::Relaxed));
        assert!(!w0.cancelled.load(Ordering::Relaxed));
        assert_eq!(scheduler.target_connections(), 1);
        assert_eq!(scheduler.active_count(), 2);

        // Second call does not recount already-cancelled w2
        let retired_again = scheduler.retire_excess_workers(1);
        assert_eq!(retired_again, 0);
    }

    #[test]
    fn test_on_worker_failed_preserves_survivor_and_skips_win_counter() {
        let mut scheduler = WorkScheduler::new(512 * 1024, Duration::from_secs(3), 3);
        let primary = make_dummy_worker(0, 0, 10_000, 1_000);
        let hedged_raw = AdaptiveWorker::new(
            1,
            "http://localhost/test".into(),
            1_000,
            10_000,
            1_000,
            None,
            None,
            None,
            None,
        )
        .with_hedged(true);
        hedged_raw.set_status(WorkerStatus::Downloading);
        let hedged = Arc::new(hedged_raw);
        scheduler.workers.extend([primary.clone(), hedged.clone()]);

        let now = Instant::now();
        scheduler.runtime.insert(
            0,
            WorkerRuntime {
                join_handle: None,
                abort_handle: None,
                last_progress_at: now,
                last_observed_pos: 1_000,
                last_observed_bytes: 0,
                hedged_for: None,
            },
        );
        scheduler.runtime.insert(
            1,
            WorkerRuntime {
                join_handle: None,
                abort_handle: None,
                last_progress_at: now,
                last_observed_pos: 1_000,
                last_observed_bytes: 0,
                hedged_for: Some(0),
            },
        );

        // If hedged twin fails, primary must NOT be cancelled and win counter stays 0
        hedged.set_status(WorkerStatus::Failed);
        scheduler.on_worker_failed(1);
        assert!(!primary.cancelled.load(Ordering::Relaxed));
        assert_eq!(scheduler.hedged_requests_won(), 0);
        assert!(scheduler.prune_worker(1));
        assert_eq!(scheduler.workers.len(), 1);
    }

    #[tokio::test]
    async fn test_stall_detector_skips_cancelled_and_disk_writing_workers() {
        let mut scheduler = WorkScheduler::new(512 * 1024, Duration::from_secs(1), 3);
        let w_disk = make_dummy_worker(0, 0, 10_000, 1_000);
        w_disk.is_writing_disk.store(true, Ordering::Release);
        let w_cancelled = make_dummy_worker(1, 10_001, 20_000, 10_500);
        w_cancelled.cancelled.store(true, Ordering::SeqCst);

        scheduler
            .workers
            .extend([w_disk.clone(), w_cancelled.clone()]);
        let old = Instant::now() - Duration::from_secs(10);
        for idx in [0u32, 1u32] {
            scheduler.runtime.insert(
                idx,
                WorkerRuntime {
                    join_handle: None,
                    abort_handle: None,
                    last_progress_at: old,
                    last_observed_pos: scheduler.workers[idx as usize].pos.load(Ordering::Relaxed),
                    last_observed_bytes: 0,
                    hedged_for: None,
                },
            );
        }

        let dir = tempfile::tempdir().unwrap();
        let writer_handle = crate::adaptive::pipeline_writer::PipelinedWriterHandle::new(
            dir.path(),
            "stall_skip.bin",
            Some(20_001),
            16,
        )
        .unwrap();
        let writer_tx = writer_handle.writer();
        let (tx, mut rx) = mpsc::channel(8);

        scheduler.check_stalled_workers_adaptive(&Client::new(), &writer_tx, None, &tx, false);
        assert!(rx.try_recv().is_err());
        assert_eq!(w_disk.retry_count.load(Ordering::Relaxed), 0);
        assert_eq!(w_cancelled.retry_count.load(Ordering::Relaxed), 0);
        drop(writer_tx);
        let mut wh = writer_handle;
        let _ = wh.abort();
    }

    #[test]
    fn test_final_telemetry_snapshot_preserves_mid_download_workers() {
        let mut scheduler = WorkScheduler::new(512 * 1024, Duration::from_secs(3), 3);

        // Worker #0 transferred 100 MB and finished early (pruned)
        let w0 = make_dummy_worker(0, 0, 100_000_000, 100_000_000);
        w0.bytes_downloaded.store(100_000_000, Ordering::Relaxed);
        w0.set_status(WorkerStatus::Completed);

        // Worker #1 scaled up mid-download, transferred 50 MB, and finished (pruned)
        let w1 = make_dummy_worker(1, 100_000_000, 150_000_000, 150_000_000);
        w1.bytes_downloaded.store(50_000_000, Ordering::Relaxed);
        w1.set_status(WorkerStatus::Completed);

        // Worker #2 failed with 0 bytes transferred (should not pollute final snapshot)
        let w2 = make_dummy_worker(2, 150_000_000, 160_000_000, 150_000_000);
        w2.bytes_downloaded.store(0, Ordering::Relaxed);
        w2.set_status(WorkerStatus::Failed);

        scheduler.workers.extend([w0, w1, w2]);
        assert_eq!(scheduler.prune_finished_workers(), 3);

        // Tail hedged worker #34 transferred 118 KB
        let w34 = make_dummy_worker(34, 150_000_000, 150_118_800, 150_118_800);
        w34.bytes_downloaded.store(118_800, Ordering::Relaxed);
        w34.set_status(WorkerStatus::Completed);
        scheduler.workers.push(w34);

        let final_snap = scheduler.get_final_telemetry_snapshot();
        let ids: Vec<u32> = final_snap.iter().map(|w| w.idx).collect();
        assert_eq!(ids, vec![0, 1, 34]);
        assert_eq!(final_snap[0].bytes_downloaded, 100_000_000);
        assert_eq!(final_snap[1].bytes_downloaded, 50_000_000);
        assert_eq!(final_snap[2].bytes_downloaded, 118_800);
    }
}
