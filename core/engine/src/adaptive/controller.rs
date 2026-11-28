use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tracing::debug;

use super::types::{AccelerationMode, AdaptiveConfig, AdaptiveDecision, TransportProtocol};

pub const CONNECTION_STEPS: &[u32] = &[1, 4, 8, 16, 24, 32, 48, 64];

const HTTP2_DEFAULT_MAX_CONNECTIONS: u32 = 16;
const HTTP2_THROTTLED_MAX_CONNECTIONS: u32 = 32;
const HTTP2_PER_STREAM_THROTTLE_GAIN: f64 = 0.08;
const TRANSIENT_ERROR_WINDOW: Duration = Duration::from_secs(10);
const TRANSIENT_ERROR_THRESHOLD: usize = 3;
const RTT_BUFFERBLOAT_FLOOR: Duration = Duration::from_millis(30);
const RTT_BUFFERBLOAT_MULTIPLIER: f64 = 2.5;

pub fn next_step_up(current: u32, max: u32) -> u32 {
    let target = CONNECTION_STEPS
        .iter()
        .copied()
        .find(|&s| s > current)
        .unwrap_or(current);
    target.min(max)
}

pub fn next_step_down(current: u32, min: u32) -> u32 {
    let target = CONNECTION_STEPS
        .iter()
        .copied()
        .rfind(|&s| s < current)
        .unwrap_or(min);
    target.max(min)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeState {
    /// Collecting baseline at `stable_target` before probing upward
    Baseline,
    /// Actively probing `to` connections (from `from`)
    Probing { from: u32, to: u32 },
    /// Cooling down at `stable_target` after failed probe #1 before launching a real 2nd probe
    CooldownBeforeRetry,
}

#[derive(Debug)]
pub struct AdaptiveController {
    config: AdaptiveConfig,
    protocol: TransportProtocol,
    http2_cap: u32,
    stable_target: u32,
    current_connections: u32,
    mode: AccelerationMode,
    probe_state: ProbeState,
    has_plateaued: bool,
    last_adjustment: Instant,
    rolling_throughput_bps: f64,
    previous_baseline_throughput: f64,
    window_bytes: u64,
    window_secs: f64,
    baseline_rtt: Option<Duration>,
    consecutive_failed_probes: u32,
    stable_locked_until: Option<Instant>,
    limited_until: Option<Instant>,
    transient_errors: VecDeque<Instant>,
    is_resumable_range: bool,
    status_reason: String,
}

impl AdaptiveController {
    pub fn new(config: AdaptiveConfig, is_resumable_range: bool) -> Self {
        let max_allowed = config
            .max_connections
            .min(config.max_connections_per_host)
            .max(1);
        let min_allowed = config.min_connections.clamp(1, max_allowed);
        let initial = if is_resumable_range {
            config.initial_connections.clamp(min_allowed, max_allowed)
        } else {
            1
        };

        let mode = if !is_resumable_range {
            AccelerationMode::FallbackSingleStream
        } else if !config.auto_scale {
            AccelerationMode::Stable
        } else {
            AccelerationMode::Auto
        };

        let status_reason = if !is_resumable_range {
            "Server does not support HTTP Range requests; single stream mode active".into()
        } else if !config.auto_scale {
            format!(
                "Fixed concurrency mode (auto-scale disabled) with {} streams",
                initial
            )
        } else {
            format!(
                "Starting adaptive acceleration with {} initial streams",
                initial
            )
        };

        Self {
            config,
            protocol: TransportProtocol::Http1,
            http2_cap: HTTP2_DEFAULT_MAX_CONNECTIONS,
            stable_target: initial,
            current_connections: initial,
            mode,
            probe_state: ProbeState::Baseline,
            has_plateaued: false,
            last_adjustment: Instant::now(),
            rolling_throughput_bps: 0.0,
            previous_baseline_throughput: 0.0,
            window_bytes: 0,
            window_secs: 0.0,
            baseline_rtt: None,
            consecutive_failed_probes: 0,
            stable_locked_until: None,
            limited_until: None,
            transient_errors: VecDeque::new(),
            is_resumable_range,
            status_reason,
        }
    }

    pub fn mode(&self) -> AccelerationMode {
        self.mode
    }

    pub fn protocol(&self) -> TransportProtocol {
        self.protocol
    }

    pub fn set_protocol(&mut self, protocol: TransportProtocol) {
        self.protocol = protocol;
        if !self.is_resumable_range || !self.config.auto_scale {
            return;
        }
        let max_allowed = self.effective_max_connections();
        if self.current_connections > max_allowed {
            self.current_connections = max_allowed;
            self.stable_target = self.stable_target.min(max_allowed);
            self.probe_state = ProbeState::Baseline;
            self.status_reason = format!(
                "{} multiplexing active; capping TCP connections at {}",
                protocol.as_str(),
                max_allowed
            );
        }
    }

    pub fn stable_target(&self) -> u32 {
        self.stable_target
    }

    pub fn current_connections(&self) -> u32 {
        self.current_connections
    }

    pub fn effective_max_connections(&self) -> u32 {
        let base_max = self
            .config
            .max_connections
            .min(self.config.max_connections_per_host)
            .max(1);
        if !self.is_resumable_range {
            return 1;
        }
        if !self.config.auto_scale {
            return self.stable_target.clamp(1, base_max);
        }
        match self.protocol {
            TransportProtocol::Http1 => base_max,
            TransportProtocol::Http2 | TransportProtocol::Http3 => base_max.min(self.http2_cap),
        }
    }

    pub fn rolling_throughput_bps(&self) -> u64 {
        self.rolling_throughput_bps.max(0.0) as u64
    }

    pub fn status_reason(&self) -> &str {
        &self.status_reason
    }

    pub fn record_transient_error(&mut self) {
        self.record_transient_error_at(Instant::now());
    }

    pub fn record_transient_error_at(&mut self, now: Instant) {
        self.prune_transient_errors(now);
        self.transient_errors.push_back(now);
    }

    pub fn recent_transient_error_count(&self, now: Instant) -> u32 {
        self.transient_errors
            .iter()
            .filter(|&&ts| now.saturating_duration_since(ts) <= TRANSIENT_ERROR_WINDOW)
            .count() as u32
    }

    fn prune_transient_errors(&mut self, now: Instant) {
        while let Some(&front) = self.transient_errors.front() {
            if now.saturating_duration_since(front) > TRANSIENT_ERROR_WINDOW {
                self.transient_errors.pop_front();
            } else {
                break;
            }
        }
    }

    fn effective_cooldown(&self) -> Duration {
        let base = self.config.cooldown_duration;
        if self.mode == AccelerationMode::Stable {
            base.saturating_mul(3)
        } else if self.has_plateaued || self.probe_state == ProbeState::CooldownBeforeRetry {
            base.saturating_mul(2)
        } else {
            base
        }
    }

    pub fn force_single_stream(&mut self) {
        self.is_resumable_range = false;
        self.stable_target = 1;
        self.current_connections = 1;
        self.mode = AccelerationMode::FallbackSingleStream;
        self.probe_state = ProbeState::Baseline;
        self.limited_until = None;
        self.stable_locked_until = None;
        self.status_reason = "Server ignored HTTP Range; single stream fallback active".into();
    }

    /// Evaluates current throughput sample and makes an adaptive scaling decision
    pub fn evaluate_sample(
        &mut self,
        bytes_transferred: u64,
        sample_duration: Duration,
        active_error_count: u32,
        has_http_429: bool,
        retry_after: Option<Duration>,
    ) -> AdaptiveDecision {
        self.evaluate_sample_with_context(
            bytes_transferred,
            sample_duration,
            active_error_count,
            has_http_429,
            retry_after,
            None,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_sample_with_context(
        &mut self,
        bytes_transferred: u64,
        sample_duration: Duration,
        active_error_count: u32,
        has_http_429: bool,
        retry_after: Option<Duration>,
        sample_rtt: Option<Duration>,
        gate_contended: bool,
    ) -> AdaptiveDecision {
        self.evaluate_sample_with_context_at(
            Instant::now(),
            bytes_transferred,
            sample_duration,
            active_error_count,
            has_http_429,
            retry_after,
            sample_rtt,
            gate_contended,
        )
    }

    /// Evaluates throughput sample at an explicit timestamp (supports testing simulation)
    pub fn evaluate_sample_at(
        &mut self,
        now: Instant,
        bytes_transferred: u64,
        sample_duration: Duration,
        active_error_count: u32,
        has_http_429: bool,
        retry_after: Option<Duration>,
    ) -> AdaptiveDecision {
        self.evaluate_sample_with_context_at(
            now,
            bytes_transferred,
            sample_duration,
            active_error_count,
            has_http_429,
            retry_after,
            None,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_sample_with_context_at(
        &mut self,
        now: Instant,
        bytes_transferred: u64,
        sample_duration: Duration,
        active_error_count: u32,
        has_http_429: bool,
        retry_after: Option<Duration>,
        sample_rtt: Option<Duration>,
        gate_contended: bool,
    ) -> AdaptiveDecision {
        let secs = sample_duration.as_secs_f64().max(0.001);
        let instant_bps = (bytes_transferred as f64 / secs) * 8.0;
        self.window_bytes = self.window_bytes.saturating_add(bytes_transferred);
        self.window_secs += secs;

        // Time-aware exponential moving average for telemetry display
        let alpha = (secs * 0.35).clamp(0.12, 0.40);
        if self.rolling_throughput_bps == 0.0 {
            self.rolling_throughput_bps = instant_bps;
        } else {
            self.rolling_throughput_bps =
                alpha * instant_bps + (1.0 - alpha) * self.rolling_throughput_bps;
        }

        if let Some(rtt) = sample_rtt {
            let floored = rtt.max(RTT_BUFFERBLOAT_FLOOR);
            self.baseline_rtt = Some(match self.baseline_rtt {
                Some(b) => b.min(floored),
                None => floored,
            });
        }

        // Single stream fallback invariant: never scale if server cannot handle ranges
        if !self.is_resumable_range {
            let reason = "Single stream fallback enforced (server non-resumable)".to_string();
            self.status_reason = reason.clone();
            return AdaptiveDecision::FallbackSingleStream { reason };
        }

        let base_max = self
            .config
            .max_connections
            .min(self.config.max_connections_per_host)
            .max(1);
        let min_allowed = self.config.min_connections.clamp(1, base_max);

        self.prune_transient_errors(now);
        let transient_count = self.transient_errors.len();

        // 1. Critical backoff: HTTP 429 Too Many Requests (step down by at most 1 step)
        if has_http_429 {
            let from = self.current_connections;
            let to = next_step_down(from, min_allowed);
            self.stable_target = to;
            self.current_connections = to;
            self.mode = AccelerationMode::Limited;
            self.probe_state = ProbeState::Baseline;
            self.last_adjustment = now;
            self.consecutive_failed_probes = 0;
            self.stable_locked_until = None;
            self.transient_errors.clear();
            let backoff = retry_after
                .unwrap_or_else(|| Duration::from_secs(30))
                .clamp(Duration::from_secs(5), Duration::from_secs(120));
            self.limited_until = Some(now + backoff);

            let reason = format!(
                "HTTP 429 Rate Limit from server. Backed off concurrency from {} to {} streams{}",
                from,
                to,
                retry_after
                    .map(|d| format!(" (waiting {:?})", d))
                    .unwrap_or_default()
            );
            self.status_reason = reason.clone();
            debug!(
                "[{:?}] AdaptiveController decision: old_target={}, new_target={}, reason='{}', rolling_bps={}",
                now, from, to, reason, self.rolling_throughput_bps
            );
            return AdaptiveDecision::LimitRate {
                retry_after,
                reason,
            };
        }

        // 2. Error rate backoff (permanent errors or sliding-window transient error burst)
        if active_error_count > 0 || transient_count >= TRANSIENT_ERROR_THRESHOLD {
            let from = self.current_connections;
            let to = next_step_down(from, min_allowed);
            self.stable_target = to;
            self.current_connections = to;
            self.mode = AccelerationMode::Limited;
            self.probe_state = ProbeState::Baseline;
            self.last_adjustment = now;
            self.consecutive_failed_probes = 0;
            self.stable_locked_until = None;
            self.transient_errors.clear();
            let backoff = self
                .config
                .cooldown_duration
                .max(Duration::from_secs(15))
                .min(Duration::from_secs(60));
            self.limited_until = Some(now + backoff);

            let reason = if active_error_count > 0 {
                format!(
                    "Detected {} worker errors. Stepping down concurrency from {} to {}",
                    active_error_count, from, to
                )
            } else {
                format!(
                    "Detected {} transient worker errors in {}s. Stepping down concurrency from {} to {}",
                    transient_count,
                    TRANSIENT_ERROR_WINDOW.as_secs(),
                    from,
                    to
                )
            };
            self.status_reason = reason.clone();
            debug!(
                "[{:?}] AdaptiveController decision: old_target={}, new_target={}, reason='{}', rolling_bps={}",
                now, from, to, reason, self.rolling_throughput_bps
            );
            return AdaptiveDecision::ScaleDown { from, to, reason };
        }

        // 3. Recovery from AccelerationMode::Limited once backoff window expires
        if self.mode == AccelerationMode::Limited {
            if let Some(until) = self.limited_until {
                if now < until {
                    let rem_secs = until.saturating_duration_since(now).as_secs_f64();
                    let reason = format!(
                        "Rate-limit/error backoff active for {:.0}s more at {} streams",
                        rem_secs, self.current_connections
                    );
                    self.status_reason = reason.clone();
                    return AdaptiveDecision::HoldStable {
                        connections: self.current_connections,
                        reason,
                    };
                }
            }

            self.limited_until = None;
            self.last_adjustment = now;
            self.probe_state = ProbeState::Baseline;
            self.consecutive_failed_probes = 0;
            self.previous_baseline_throughput = 0.0;

            if self.config.auto_scale {
                self.mode = AccelerationMode::Auto;
                let reason = format!(
                    "Backoff window expired; resuming adaptive optimization from {} streams",
                    self.current_connections
                );
                self.status_reason = reason.clone();
                return AdaptiveDecision::HoldStable {
                    connections: self.current_connections,
                    reason,
                };
            } else {
                self.mode = AccelerationMode::Stable;
                let reason = format!(
                    "Backoff window expired; holding fixed {} streams",
                    self.current_connections
                );
                self.status_reason = reason.clone();
                return AdaptiveDecision::HoldStable {
                    connections: self.current_connections,
                    reason,
                };
            }
        }

        // Fixed concurrency mode when auto_scale is disabled
        if !self.config.auto_scale {
            let reason = format!(
                "Fixed concurrency ({}) maintained at {} streams ({:.2} Mbps)",
                self.mode.as_str(),
                self.current_connections,
                self.rolling_throughput_bps / 1_000_000.0
            );
            self.status_reason = reason.clone();
            return AdaptiveDecision::HoldStable {
                connections: self.current_connections,
                reason,
            };
        }

        // 4. Dynamic cooldown check to enforce hysteresis (fast early warmup, wider once plateaued/Stable)
        let min_cooldown = self.effective_cooldown();
        let elapsed_since_adjust = now.saturating_duration_since(self.last_adjustment);
        if elapsed_since_adjust < min_cooldown {
            let reason = format!(
                "Observing throughput in {:?} state ({:.1}s / {:.1}s cooldown)",
                self.mode,
                elapsed_since_adjust.as_secs_f64(),
                min_cooldown.as_secs_f64()
            );
            return AdaptiveDecision::HoldStable {
                connections: self.current_connections,
                reason,
            };
        }

        // 4a. Gate-contention freeze: if workers are queued locally on HostConcurrencyGate,
        // do not misinterpret permit starvation as remote bandwidth plateau or congestion.
        if gate_contended {
            let reason = format!(
                "Host concurrency gate contended across active downloads; holding {} streams",
                self.current_connections
            );
            self.status_reason = reason.clone();
            return AdaptiveDecision::HoldStable {
                connections: self.current_connections,
                reason,
            };
        }

        // 4b. RTT bufferbloat guard (only on fresh TTFB samples exceeding 2.5x floored baseline)
        if let (Some(rtt), Some(base_rtt)) = (sample_rtt, self.baseline_rtt) {
            if self.current_connections > min_allowed
                && rtt.as_secs_f64() > base_rtt.as_secs_f64() * RTT_BUFFERBLOAT_MULTIPLIER
            {
                let step_from = self.current_connections;
                let step_to = next_step_down(step_from, min_allowed);
                self.stable_target = step_to;
                self.current_connections = step_to;
                self.mode = AccelerationMode::Stable;
                self.probe_state = ProbeState::Baseline;
                self.last_adjustment = now;
                self.window_bytes = 0;
                self.window_secs = 0.0;
                self.stable_locked_until = Some(now + Duration::from_secs(20));
                let reason = format!(
                    "Bufferbloat RTT spike ({}ms > {:.0}ms baseline) -> stepping down {} -> {}",
                    rtt.as_millis(),
                    base_rtt.as_millis() as f64 * RTT_BUFFERBLOAT_MULTIPLIER,
                    step_from,
                    step_to
                );
                self.status_reason = reason.clone();
                return AdaptiveDecision::ScaleDown {
                    from: step_from,
                    to: step_to,
                    reason,
                };
            }
        }

        // 4b. Severe throughput collapse guard (e.g. 10 MB/s -> 25 KB/s CDN throttling):
        // Break Stable lock immediately and halve concurrency to relieve server throttling.
        if self.current_connections > min_allowed.max(4)
            && self.previous_baseline_throughput > 500_000.0
            && self.rolling_throughput_bps < self.previous_baseline_throughput * 0.35
        {
            let step_from = self.current_connections;
            let step_to = (step_from / 2).max(min_allowed.max(2));
            if step_to < step_from {
                self.stable_locked_until = Some(now + Duration::from_secs(30));
                self.mode = AccelerationMode::Stable;
                self.probe_state = ProbeState::Baseline;
                self.stable_target = step_to;
                self.current_connections = step_to;
                self.last_adjustment = now;
                let reason = format!(
                    "Severe throughput collapse ({:.2} Mbps vs {:.2} Mbps baseline) -> halving concurrency {} -> {}",
                    self.rolling_throughput_bps / 1_000_000.0,
                    self.previous_baseline_throughput / 1_000_000.0,
                    step_from,
                    step_to
                );
                self.status_reason = reason.clone();
                return AdaptiveDecision::ScaleDown {
                    from: step_from,
                    to: step_to,
                    reason,
                };
            }
        }

        // 5. Check if controller is locked in Stable mode after plateau/degradation (lock for 60s)
        if let Some(lock_until) = self.stable_locked_until {
            if now < lock_until {
                let rem_secs = lock_until.saturating_duration_since(now).as_secs_f64();
                let reason = format!(
                    "Stable lock active for {:.0}s more (locked at {} streams)",
                    rem_secs, self.current_connections
                );
                self.status_reason = reason.clone();
                return AdaptiveDecision::HoldStable {
                    connections: self.current_connections,
                    reason,
                };
            } else {
                // Lock expired, allow fresh exploration
                self.stable_locked_until = None;
                self.mode = AccelerationMode::Auto;
                self.probe_state = ProbeState::Baseline;
                self.consecutive_failed_probes = 0;
                self.previous_baseline_throughput = 0.0;
            }
        }

        // 6. Concurrency Optimization State Machine
        let max_allowed = self.effective_max_connections();

        if self.mode == AccelerationMode::Auto || self.mode == AccelerationMode::Optimizing {
            match self.probe_state {
                ProbeState::Baseline | ProbeState::CooldownBeforeRetry => {
                    // Record fresh baseline using window-mean throughput (no EMA lag) before probing
                    let baseline_bps = if self.window_secs >= 0.05 && self.window_bytes > 0 {
                        (self.window_bytes as f64 / self.window_secs) * 8.0
                    } else {
                        self.rolling_throughput_bps
                    };
                    self.previous_baseline_throughput = baseline_bps;
                    self.window_bytes = 0;
                    self.window_secs = 0.0;
                    if self.current_connections < max_allowed {
                        let from = self.current_connections;
                        let to = next_step_up(from, max_allowed);
                        if to > from {
                            self.current_connections = to;
                            self.mode = AccelerationMode::Optimizing;
                            self.probe_state = ProbeState::Probing { from, to };
                            self.last_adjustment = now;
                            let reason = if self.consecutive_failed_probes > 0 {
                                format!(
                                    "Retrying bandwidth probe: scaling from {} to {} streams",
                                    from, to
                                )
                            } else {
                                format!(
                                    "Scaling from {} to {} streams to probe bandwidth headroom ({})",
                                    from,
                                    to,
                                    self.protocol.as_str()
                                )
                            };
                            self.status_reason = reason.clone();
                            debug!(
                                "[{:?}] AdaptiveController decision: old_target={}, new_target={}, reason='{}', rolling_bps={}",
                                now, from, to, reason, self.rolling_throughput_bps
                            );
                            return AdaptiveDecision::ScaleUp { from, to, reason };
                        }
                    }

                    self.mode = AccelerationMode::Stable;
                    self.probe_state = ProbeState::Baseline;
                    self.has_plateaued = true;
                    self.last_adjustment = now;
                    let reason = format!(
                        "Optimal throughput achieved at {} ceiling ({} streams)",
                        self.protocol.as_str(),
                        max_allowed
                    );
                    self.status_reason = reason.clone();
                    return AdaptiveDecision::HoldStable {
                        connections: self.stable_target,
                        reason,
                    };
                }
                ProbeState::Probing { from, to } => {
                    let window_bps = if self.window_secs >= 0.05 && self.window_bytes > 0 {
                        (self.window_bytes as f64 / self.window_secs) * 8.0
                    } else {
                        self.rolling_throughput_bps
                    };
                    self.window_bytes = 0;
                    self.window_secs = 0.0;

                    let delta = if self.previous_baseline_throughput > 0.0 {
                        (window_bps - self.previous_baseline_throughput)
                            / self.previous_baseline_throughput
                    } else {
                        0.0
                    };

                    if delta >= self.config.throughput_gain_threshold {
                        // Sustained meaningful gain (>= +8%)
                        self.consecutive_failed_probes = 0;
                        self.previous_baseline_throughput = window_bps;
                        self.stable_target = to;

                        // On HTTP/2/3, expand cap past 16 up to 32 if per-stream gain >= +8%
                        if self.protocol != TransportProtocol::Http1
                            && to >= self.http2_cap
                            && delta >= HTTP2_PER_STREAM_THROTTLE_GAIN
                            && self.http2_cap < base_max.min(HTTP2_THROTTLED_MAX_CONNECTIONS)
                        {
                            self.http2_cap = next_step_up(
                                self.http2_cap,
                                base_max.min(HTTP2_THROTTLED_MAX_CONNECTIONS),
                            );
                        }

                        let updated_max = self.effective_max_connections();
                        if self.current_connections < updated_max {
                            let next_from = self.current_connections;
                            let single_step = next_step_up(next_from, updated_max);
                            // Multiplicative warmup jump when scaling strongly (>= +50% gain) below 32 streams
                            let next_to = if delta >= 0.50 && next_from < 32 {
                                next_step_up(single_step, updated_max)
                            } else {
                                single_step
                            };
                            if next_to > next_from {
                                self.current_connections = next_to;
                                self.mode = AccelerationMode::Optimizing;
                                self.probe_state = ProbeState::Probing {
                                    from: next_from,
                                    to: next_to,
                                };
                                self.last_adjustment = now;
                                let reason = format!(
                                    "Throughput increased by +{:.1}% ({:.2} Mbps) -> scaling concurrency from {} to {}",
                                    delta * 100.0,
                                    self.rolling_throughput_bps / 1_000_000.0,
                                    next_from,
                                    next_to
                                );
                                self.status_reason = reason.clone();
                                debug!(
                                    "[{:?}] AdaptiveController decision: committed_target={}, probe_target={}, reason='{}', rolling_bps={}",
                                    now, self.stable_target, next_to, reason, self.rolling_throughput_bps
                                );
                                return AdaptiveDecision::ScaleUp {
                                    from: next_from,
                                    to: next_to,
                                    reason,
                                };
                            }
                        }

                        self.mode = AccelerationMode::Stable;
                        self.probe_state = ProbeState::Baseline;
                        self.has_plateaued = true;
                        self.last_adjustment = now;
                        let reason = format!(
                            "Optimal throughput achieved at {} connection cap ({})",
                            self.protocol.as_str(),
                            updated_max
                        );
                        self.status_reason = reason.clone();
                        debug!(
                            "[{:?}] AdaptiveController decision: locked at max_cap={}, reason='{}', rolling_bps={}",
                            now, self.stable_target, reason, self.rolling_throughput_bps
                        );
                        return AdaptiveDecision::HoldStable {
                            connections: self.stable_target,
                            reason,
                        };
                    } else if delta <= self.config.throughput_drop_threshold {
                        // Throughput degraded by <= -15%: step down concurrency by at most 1 step and lock
                        self.mode = AccelerationMode::Stable;
                        self.probe_state = ProbeState::Baseline;
                        self.has_plateaued = true;
                        let step_from = self.current_connections;
                        let step_to = next_step_down(step_from, min_allowed);
                        self.stable_target = step_to;
                        self.current_connections = step_to;
                        self.last_adjustment = now;
                        self.stable_locked_until = Some(now + Duration::from_secs(60));
                        let reason = format!(
                            "Throughput degraded by {:.1}% -> stepping down concurrency from {} to {} and locking for 60s",
                            delta.abs() * 100.0,
                            step_from,
                            step_to
                        );
                        self.status_reason = reason.clone();
                        debug!(
                            "[{:?}] AdaptiveController decision: old_target={}, new_target={}, reason='{}', rolling_bps={}",
                            now, step_from, step_to, reason, self.rolling_throughput_bps
                        );
                        return AdaptiveDecision::ScaleDown {
                            from: step_from,
                            to: step_to,
                            reason,
                        };
                    } else {
                        // Diminishing returns (between -15% and +10%): revert probe to stable_target
                        self.consecutive_failed_probes += 1;
                        self.stable_target = from.min(self.stable_target).max(min_allowed);
                        self.current_connections = self.stable_target;
                        self.last_adjustment = now;

                        let conn_ratio = if self.stable_target > 0 {
                            (to as f64 - self.stable_target as f64) / (self.stable_target as f64)
                        } else {
                            1.0
                        };
                        let marginal_efficiency = if conn_ratio > 0.0 {
                            delta / conn_ratio
                        } else {
                            0.0
                        };

                        if self.consecutive_failed_probes >= 2 {
                            self.mode = AccelerationMode::Stable;
                            self.probe_state = ProbeState::Baseline;
                            self.has_plateaued = true;
                            self.stable_locked_until = Some(now + Duration::from_secs(60));
                            let reason = format!(
                                "BBR Saturation Detected: Throughput plateaued at {:.2} Mbps (marginal stream efficiency {:.1}%). Concurrency locked at {} streams for 60s",
                                self.rolling_throughput_bps / 1_000_000.0,
                                marginal_efficiency * 100.0,
                                self.stable_target
                            );
                            self.status_reason = reason.clone();
                            debug!(
                                "[{:?}] AdaptiveController decision: locked at {}, reason='{}', rolling_bps={}",
                                now, self.stable_target, reason, self.rolling_throughput_bps
                            );
                            return AdaptiveDecision::HoldStable {
                                connections: self.stable_target,
                                reason,
                            };
                        } else {
                            // Cool down at stable_target before attempting a real 2nd probe
                            self.probe_state = ProbeState::CooldownBeforeRetry;
                            let reason = format!(
                                "BBR Probe: Concurrency {} yielded {:+.1}% gain (marginal efficiency {:.1}%). Reverted to stable target ({})",
                                to,
                                delta * 100.0,
                                marginal_efficiency * 100.0,
                                self.stable_target
                            );
                            self.status_reason = reason.clone();
                            debug!(
                                "[{:?}] AdaptiveController decision: reverted probe {} -> {}, reason='{}'",
                                now, to, self.stable_target, reason
                            );
                            return AdaptiveDecision::HoldStable {
                                connections: self.stable_target,
                                reason,
                            };
                        }
                    }
                }
            }
        }

        // In Stable mode (or after lock): keep steady
        let reason = format!(
            "Steady state ({}) maintained at {} streams ({:.2} Mbps)",
            self.mode.as_str(),
            self.current_connections,
            self.rolling_throughput_bps / 1_000_000.0
        );
        self.status_reason = reason.clone();
        AdaptiveDecision::HoldStable {
            connections: self.current_connections,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_steps_ladder() {
        assert_eq!(next_step_up(1, 64), 4);
        assert_eq!(next_step_up(4, 64), 8);
        assert_eq!(next_step_up(8, 64), 16);
        assert_eq!(next_step_up(16, 64), 24);
        assert_eq!(next_step_up(24, 64), 32);
        assert_eq!(next_step_up(32, 64), 48);
        assert_eq!(next_step_up(48, 64), 64);
        assert_eq!(next_step_up(64, 64), 64);
        assert_eq!(next_step_up(16, 16), 16);

        assert_eq!(next_step_down(64, 1), 48);
        assert_eq!(next_step_down(48, 1), 32);
        assert_eq!(next_step_down(32, 1), 24);
        assert_eq!(next_step_down(24, 1), 16);
        assert_eq!(next_step_down(16, 1), 8);
        assert_eq!(next_step_down(8, 1), 4);
        assert_eq!(next_step_down(4, 1), 1);
        assert_eq!(next_step_down(1, 1), 1);
    }

    #[test]
    fn test_controller_clamps_initial_against_host_ceiling() {
        let config = AdaptiveConfig {
            initial_connections: 16,
            min_connections: 1,
            max_connections: 32,
            max_connections_per_host: 6,
            ..Default::default()
        };
        let controller = AdaptiveController::new(config, true);
        assert_eq!(controller.current_connections(), 6);
        assert_eq!(controller.stable_target(), 6);
    }

    #[test]
    fn test_controller_scales_up_to_64_on_http1() {
        let config = AdaptiveConfig {
            initial_connections: 32,
            min_connections: 1,
            max_connections: 64,
            max_connections_per_host: 64,
            cooldown_duration: Duration::from_secs(4),
            throughput_gain_threshold: 0.10,
            throughput_drop_threshold: -0.15,
            ..Default::default()
        };
        let mut controller = AdaptiveController::new(config, true);
        let start = Instant::now();
        controller.last_adjustment = start;

        let t1 = start + Duration::from_secs(4);
        let d1 =
            controller.evaluate_sample_at(t1, 4_000_000, Duration::from_secs(1), 0, false, None);
        assert!(matches!(
            d1,
            AdaptiveDecision::ScaleUp {
                from: 32,
                to: 48,
                ..
            }
        ));

        let t2 = t1 + Duration::from_secs(4);
        let d2 =
            controller.evaluate_sample_at(t2, 8_000_000, Duration::from_secs(1), 0, false, None);
        assert!(matches!(
            d2,
            AdaptiveDecision::ScaleUp {
                from: 48,
                to: 64,
                ..
            }
        ));

        let t3 = t2 + Duration::from_secs(4);
        let d3 =
            controller.evaluate_sample_at(t3, 14_000_000, Duration::from_secs(1), 0, false, None);
        assert!(matches!(
            d3,
            AdaptiveDecision::HoldStable {
                connections: 64,
                ..
            }
        ));
        assert_eq!(controller.stable_target(), 64);
    }

    #[test]
    fn test_controller_http2_protocol_awareness() {
        let config = AdaptiveConfig {
            initial_connections: 8,
            min_connections: 1,
            max_connections: 32,
            max_connections_per_host: 32,
            cooldown_duration: Duration::from_secs(4),
            throughput_gain_threshold: 0.08,
            throughput_drop_threshold: -0.15,
            ..Default::default()
        };
        let mut controller = AdaptiveController::new(config, true);
        controller.set_protocol(TransportProtocol::Http2);
        let start = Instant::now();
        controller.last_adjustment = start;

        // Probe 8 -> 16
        let t1 = start + Duration::from_secs(4);
        let d1 =
            controller.evaluate_sample_at(t1, 10_000_000, Duration::from_secs(1), 0, false, None);
        assert!(matches!(
            d1,
            AdaptiveDecision::ScaleUp { from: 8, to: 16, .. }
        ));

        // +60% gain (>= +8% and >= +50% warmup) expands H2 cap 16 -> 24 and jumps 16 -> 24
        let t2 = t1 + Duration::from_secs(4);
        let d2 =
            controller.evaluate_sample_at(t2, 16_000_000, Duration::from_secs(1), 0, false, None);
        assert!(matches!(
            d2,
            AdaptiveDecision::ScaleUp { from: 16, to: 24, .. }
        ));
    }

    #[test]
    fn test_controller_limited_recovery_and_transient_errors() {
        let config = AdaptiveConfig {
            initial_connections: 8,
            min_connections: 1,
            max_connections: 32,
            max_connections_per_host: 32,
            cooldown_duration: Duration::from_secs(4),
            ..Default::default()
        };
        let mut controller = AdaptiveController::new(config, true);
        let start = Instant::now();
        controller.last_adjustment = start;

        // 3 transient errors within 10s trigger ScaleDown -> Limited (8 -> 4)
        controller.record_transient_error_at(start + Duration::from_secs(1));
        controller.record_transient_error_at(start + Duration::from_secs(2));
        controller.record_transient_error_at(start + Duration::from_secs(3));
        let d_err = controller.evaluate_sample_at(
            start + Duration::from_secs(3),
            1_000_000,
            Duration::from_secs(1),
            0,
            false,
            None,
        );
        assert!(matches!(
            d_err,
            AdaptiveDecision::ScaleDown { from: 8, to: 4, .. }
        ));
        assert_eq!(controller.mode(), AccelerationMode::Limited);

        // During backoff window: remains Limited
        let d_hold = controller.evaluate_sample_at(
            start + Duration::from_secs(10),
            1_000_000,
            Duration::from_secs(1),
            0,
            false,
            None,
        );
        assert!(matches!(
            d_hold,
            AdaptiveDecision::HoldStable { connections: 4, .. }
        ));
        assert_eq!(controller.mode(), AccelerationMode::Limited);

        // Once backoff expires (15s): recovers Limited -> Auto
        let d_rec = controller.evaluate_sample_at(
            start + Duration::from_secs(19),
            1_000_000,
            Duration::from_secs(1),
            0,
            false,
            None,
        );
        assert!(matches!(
            d_rec,
            AdaptiveDecision::HoldStable { connections: 4, .. }
        ));
        assert_eq!(controller.mode(), AccelerationMode::Auto);

        // HTTP 429 also recovers cleanly after Retry-After window
        let t_429 = start + Duration::from_secs(25);
        let d_429 = controller.evaluate_sample_at(
            t_429,
            1_000_000,
            Duration::from_secs(1),
            0,
            true,
            Some(Duration::from_secs(6)),
        );
        assert!(matches!(d_429, AdaptiveDecision::LimitRate { .. }));
        assert_eq!(controller.mode(), AccelerationMode::Limited);

        let d_after_429 = controller.evaluate_sample_at(
            t_429 + Duration::from_secs(7),
            1_000_000,
            Duration::from_secs(1),
            0,
            false,
            None,
        );
        assert!(matches!(d_after_429, AdaptiveDecision::HoldStable { .. }));
        assert_eq!(controller.mode(), AccelerationMode::Auto);
    }

    #[test]
    fn test_controller_no_phantom_second_probe_and_preserves_reason() {
        let config = AdaptiveConfig {
            initial_connections: 8,
            min_connections: 1,
            max_connections: 16,
            max_connections_per_host: 16,
            cooldown_duration: Duration::from_secs(4),
            throughput_gain_threshold: 0.10,
            throughput_drop_threshold: -0.15,
            ..Default::default()
        };
        let mut controller = AdaptiveController::new(config, true);
        let start = Instant::now();
        controller.last_adjustment = start;

        // Baseline + Probe #1 (8 -> 16)
        let t1 = start + Duration::from_secs(4);
        let d1 =
            controller.evaluate_sample_at(t1, 1_000_000, Duration::from_secs(1), 0, false, None);
        assert!(matches!(
            d1,
            AdaptiveDecision::ScaleUp {
                from: 8,
                to: 16,
                ..
            }
        ));

        // Probe #1 fails (flat throughput) -> reverts to 8, preserves BBR Probe reason, stays Optimizing
        let t2 = t1 + Duration::from_secs(4);
        let d2 =
            controller.evaluate_sample_at(t2, 1_000_000, Duration::from_secs(1), 0, false, None);
        assert!(matches!(
            d2,
            AdaptiveDecision::HoldStable { connections: 8, .. }
        ));
        assert!(controller.status_reason().contains("BBR Probe:"));
        assert_eq!(controller.mode(), AccelerationMode::Optimizing);

        // After widened cooldown (8s), must launch a REAL Probe #2 (8 -> 16), NOT lock Stable without probing!
        let t3 = t2 + Duration::from_secs(8);
        let d3 =
            controller.evaluate_sample_at(t3, 1_000_000, Duration::from_secs(1), 0, false, None);
        assert!(matches!(
            d3,
            AdaptiveDecision::ScaleUp {
                from: 8,
                to: 16,
                ..
            }
        ));

        // Probe #2 also fails -> now locks Stable at 8
        let t4 = t3 + Duration::from_secs(8);
        let d4 =
            controller.evaluate_sample_at(t4, 1_000_000, Duration::from_secs(1), 0, false, None);
        assert!(matches!(
            d4,
            AdaptiveDecision::HoldStable { connections: 8, .. }
        ));
        assert_eq!(controller.mode(), AccelerationMode::Stable);
    }

    #[test]
    fn test_controller_simulation_flat_and_noisy() {
        let config = AdaptiveConfig {
            initial_connections: 8,
            min_connections: 1,
            max_connections: 16,
            max_connections_per_host: 16,
            cooldown_duration: Duration::from_secs(15),
            throughput_gain_threshold: 0.10,
            throughput_drop_threshold: -0.15,
            sample_interval: Duration::from_millis(250),
            ..Default::default()
        };

        let mut controller = AdaptiveController::new(config, true);
        assert_eq!(controller.current_connections(), 8);

        let start_time = Instant::now();
        let mut sim_clock = start_time;
        controller.last_adjustment = sim_clock;

        let mut change_events: Vec<(u32, u32, Duration)> = Vec::new();
        let mut prev_conns = 8;

        // Simulate 120 seconds with noisy throughput (e.g. 5 MB/s +/- 5%)
        let total_seconds = 120;
        let ticks_per_sec = 4; // 250ms ticks
        let mut pseudo_rand: u64 = 12345;

        for tick in 1..=(total_seconds * ticks_per_sec) {
            let elapsed_sim = Duration::from_millis(tick as u64 * 250);
            sim_clock = start_time + elapsed_sim;

            pseudo_rand = pseudo_rand
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
            let noise_factor = 0.95 + ((pseudo_rand % 100) as f64 / 1000.0); // 0.95 .. 1.05
            let bytes_transferred = (1_250_000.0 * noise_factor) as u64; // ~5 MB/s

            let _ = controller.evaluate_sample_at(
                sim_clock,
                bytes_transferred,
                Duration::from_millis(250),
                0,
                false,
                None,
            );

            let cur = controller.stable_target();
            if cur != prev_conns {
                change_events.push((prev_conns, cur, elapsed_sim));
                prev_conns = cur;
            }
        }

        // Verify: Changes must respect the >= 15s cooldown
        for window in change_events.windows(2) {
            let (from1, to1, t1) = window[0];
            let (from2, to2, t2) = window[1];
            let diff = t2.saturating_sub(t1);
            assert!(
                diff >= Duration::from_secs(15),
                "Target connection count changed too fast ({:?} between changes: {}->{} at {:?}, {}->{} at {:?})",
                diff, from1, to1, t1, from2, to2, t2
            );

            // Assert no oscillation (no A->B->A within 60s)
            if from1 == to2 {
                assert!(
                    diff >= Duration::from_secs(60),
                    "Oscillation detected: {} -> {} -> {} within {:?}",
                    from1,
                    to1,
                    to2,
                    diff
                );
            }
        }

        // Must settle in Stable mode
        assert_eq!(controller.mode(), AccelerationMode::Stable);
    }

    #[test]
    fn test_eta_smoothing_with_zero_speed_dips() {
        let total_size: u64 = 100 * 1024 * 1024; // 100 MB
        let mut total_downloaded: u64 = 0;
        let mut smoothed_speed_bytes: f64 = 0.0;
        let mut smoothed_eta_sec: Option<u64> = None;
        let start_time = Instant::now() - Duration::from_secs(5); // already past 3s warmup
        let mut last_progress_time = Instant::now();

        // Simulate 20 ticks at 5 MB/s, then 2 ticks of 0 MB/s (reconnect dip), then 10 ticks at 5 MB/s
        let sample_duration = Duration::from_millis(250);
        let normal_chunk = 1_250_000; // 5 MB/s / 4

        for tick in 0..32 {
            let chunk = if (10..13).contains(&tick) {
                0 // 0.75s dip to zero
            } else {
                normal_chunk
            };

            total_downloaded += chunk;
            let secs = sample_duration.as_secs_f64();
            let instant_rate = (chunk as f64 / secs) as u64;

            if chunk > 0 {
                last_progress_time = Instant::now();
            }

            if smoothed_speed_bytes <= 0.0 {
                smoothed_speed_bytes = instant_rate as f64;
            } else {
                smoothed_speed_bytes = 0.20 * (instant_rate as f64) + 0.80 * smoothed_speed_bytes;
            }

            let effective_rate = smoothed_speed_bytes as u64;
            let is_warmed_up =
                start_time.elapsed() >= Duration::from_secs(3) || total_downloaded >= 1024 * 1024;
            assert!(is_warmed_up);

            let rem = total_size.saturating_sub(total_downloaded);
            let eta = if let Some(raw_eta) = rem.checked_div(effective_rate) {
                let smoothed = if let Some(prev) = smoothed_eta_sec {
                    let max_delta = (prev as f64 * 0.20).max(1.0) as u64;
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
            };

            // Assert: ETA NEVER reverts to None (Calculating) during brief zero-speed dips
            assert!(
                eta.is_some(),
                "ETA reverted to None/Calculating during tick {}",
                tick
            );
        }
    }

    #[test]
    fn test_controller_trace_under_10pct_gain_never_oscillates() {
        let config = AdaptiveConfig {
            initial_connections: 8,
            min_connections: 1,
            max_connections: 16,
            max_connections_per_host: 16,
            cooldown_duration: Duration::from_secs(20),
            throughput_gain_threshold: 0.10,
            throughput_drop_threshold: -0.15,
            sample_interval: Duration::from_millis(250),
            ..Default::default()
        };

        let mut controller = AdaptiveController::new(config, true);
        assert_eq!(controller.stable_target(), 8);

        let start_time = Instant::now();
        let mut sim_clock = start_time;
        controller.last_adjustment = sim_clock;

        // Baseline: 4 MB/s (1 MB per 250ms)
        for _ in 0..8 {
            sim_clock += Duration::from_millis(250);
            let _ = controller.evaluate_sample_at(
                sim_clock,
                1_000_000,
                Duration::from_millis(250),
                0,
                false,
                None,
            );
        }

        // Advance 21 seconds to trigger probe upward
        sim_clock += Duration::from_secs(21);
        let dec_probe1 = controller.evaluate_sample_at(
            sim_clock,
            1_000_000,
            Duration::from_millis(250),
            0,
            false,
            None,
        );

        match dec_probe1 {
            AdaptiveDecision::ScaleUp { from, to, .. } => {
                assert_eq!(from, 8);
                assert_eq!(to, 16);
            }
            other => panic!("Expected ScaleUp probe, got {:?}", other),
        }
        // UI still sees stable target 8!
        assert_eq!(controller.stable_target(), 8);

        // Throughput only increases by 3% (under 10% gain requirement)
        for _ in 0..8 {
            sim_clock += Duration::from_millis(250);
            let _ = controller.evaluate_sample_at(
                sim_clock,
                1_030_000,
                Duration::from_millis(250),
                0,
                false,
                None,
            );
        }

        // Advance past 20s cooldown
        sim_clock += Duration::from_secs(21);
        let _ = controller.evaluate_sample_at(
            sim_clock,
            1_030_000,
            Duration::from_millis(250),
            0,
            false,
            None,
        );

        // Sub-10% gain: probe reverted to stable target 8 without changing stable target!
        assert_eq!(controller.stable_target(), 8);
        assert_eq!(controller.current_connections(), 8);
    }
}
