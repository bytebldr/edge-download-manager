use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

use super::types::TransportProtocol;
use super::worker::epoch_ms_now;

const MAX_HOST_ENTRIES: usize = 256;
const CIRCUIT_DECAY_MS: u64 = 30_000;

/// Dynamic per-host concurrency gate with RAII permit release and lost-wakeup-free waiting.
#[derive(Debug)]
pub struct HostConcurrencyGate {
    active_permits: AtomicU32,
    max_permits: AtomicU32,
    waiters: AtomicU32,
    notify: Notify,
}

impl HostConcurrencyGate {
    pub fn new(initial_max: u32) -> Self {
        Self {
            active_permits: AtomicU32::new(0),
            max_permits: AtomicU32::new(initial_max.max(1)),
            waiters: AtomicU32::new(0),
            notify: Notify::new(),
        }
    }

    /// Raises the ceiling if a newly joined task allows higher concurrency.
    pub fn raise_max_permits(&self, new_max: u32) {
        let clamped = new_max.max(1);
        let prev = self.max_permits.fetch_max(clamped, Ordering::AcqRel);
        if clamped > prev {
            self.notify.notify_waiters();
        }
    }

    /// Updates max permits (used on explicit rate-limit step-down) without revoking in-flight permits.
    pub fn set_max_permits(&self, new_max: u32) {
        let clamped = new_max.max(1);
        let prev = self.max_permits.swap(clamped, Ordering::AcqRel);
        if clamped > prev {
            self.notify.notify_waiters();
        }
    }

    pub fn active_permits(&self) -> u32 {
        self.active_permits.load(Ordering::Acquire)
    }

    pub fn max_permits(&self) -> u32 {
        self.max_permits.load(Ordering::Acquire)
    }

    pub fn is_contended(&self) -> bool {
        self.waiters.load(Ordering::Acquire) > 0
    }

    /// Acquires an RAII `HostPermit`, returning `None` if cancelled or timed out.
    pub async fn acquire(
        self: &Arc<Self>,
        cancelled: &AtomicBool,
        timeout: Duration,
    ) -> Option<HostPermit> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut waiting_counted = false;

        struct WaiterGuard<'a>(&'a AtomicU32, bool);
        impl<'a> Drop for WaiterGuard<'a> {
            fn drop(&mut self) {
                if self.1 {
                    self.0.fetch_sub(1, Ordering::AcqRel);
                }
            }
        }
        let mut wguard = WaiterGuard(&self.waiters, false);

        loop {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }

            // Pin notified() BEFORE reading active/max_permits to guarantee no lost wakeups.
            let notified = self.notify.notified();
            tokio::pin!(notified);

            let max = self.max_permits.load(Ordering::Acquire);
            let cur = self.active_permits.load(Ordering::Acquire);
            if cur < max
                && self
                    .active_permits
                    .compare_exchange(cur, cur + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                return Some(HostPermit {
                    gate: Arc::clone(self),
                });
            }

            if !waiting_counted {
                self.waiters.fetch_add(1, Ordering::AcqRel);
                waiting_counted = true;
                wguard.1 = true;
            }

            let now = tokio::time::Instant::now();
            if now >= deadline {
                return None;
            }

            tokio::select! {
                _ = &mut notified => {}
                _ = tokio::time::sleep_until(deadline) => {
                    return None;
                }
                _ = async {
                    while !cancelled.load(Ordering::Relaxed) {
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                } => {
                    return None;
                }
            }
        }
    }
}

/// RAII permit guard: unconditionally releases slot even on task abort/panic.
#[derive(Debug)]
pub struct HostPermit {
    gate: Arc<HostConcurrencyGate>,
}

impl Drop for HostPermit {
    fn drop(&mut self) {
        self.gate.active_permits.fetch_sub(1, Ordering::AcqRel);
        self.gate.notify.notify_waiters();
    }
}

/// Shared host-level circuit breaker & learned transport profile across concurrent tasks.
#[derive(Debug)]
pub struct HostCircuitState {
    pub paused_until_ms: AtomicU64,
    pub consecutive_429s: AtomicU32,
    last_429_epoch_ms: AtomicU64,
    /// 0 = unknown, 1 = Http1, 2 = Http2, 3 = Http3
    preferred_proto: AtomicU8,
    safe_initial_conns: AtomicU32,
}

impl HostCircuitState {
    pub fn new() -> Self {
        Self {
            paused_until_ms: AtomicU64::new(0),
            consecutive_429s: AtomicU32::new(0),
            last_429_epoch_ms: AtomicU64::new(0),
            preferred_proto: AtomicU8::new(0),
            safe_initial_conns: AtomicU32::new(0),
        }
    }

    pub fn record_rate_limit(&self, backoff: Duration) -> u32 {
        let now_ms = epoch_ms_now();
        let until_ms = now_ms.saturating_add(backoff.as_millis() as u64);
        self.paused_until_ms.fetch_max(until_ms, Ordering::AcqRel);
        self.last_429_epoch_ms.store(now_ms, Ordering::Release);
        self.consecutive_429s.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn decay_if_healthy(&self) {
        let last = self.last_429_epoch_ms.load(Ordering::Acquire);
        if last > 0 && epoch_ms_now().saturating_sub(last) >= CIRCUIT_DECAY_MS {
            self.consecutive_429s.store(0, Ordering::Release);
        }
    }

    pub fn set_preferred_protocol(&self, proto: TransportProtocol) {
        let code = match proto {
            TransportProtocol::Http1 => 1,
            TransportProtocol::Http2 => 2,
            TransportProtocol::Http3 => 3,
        };
        self.preferred_proto.store(code, Ordering::Release);
    }

    pub fn preferred_protocol(&self) -> Option<TransportProtocol> {
        match self.preferred_proto.load(Ordering::Acquire) {
            1 => Some(TransportProtocol::Http1),
            2 => Some(TransportProtocol::Http2),
            3 => Some(TransportProtocol::Http3),
            _ => None,
        }
    }

    pub fn set_safe_initial_conns(&self, conns: u32) {
        self.safe_initial_conns.store(conns, Ordering::Release);
    }

    pub fn safe_initial_conns(&self) -> Option<u32> {
        let v = self.safe_initial_conns.load(Ordering::Acquire);
        if v == 0 {
            None
        } else {
            Some(v)
        }
    }
}

impl Default for HostCircuitState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HostRegistry {
    inner: Mutex<HostRegistryInner>,
}

struct HostRegistryInner {
    gates: HashMap<String, Arc<HostConcurrencyGate>>,
    circuits: HashMap<String, Arc<HostCircuitState>>,
    order: VecDeque<String>,
    capacity: usize,
}

impl HostRegistry {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(HostRegistryInner {
                gates: HashMap::new(),
                circuits: HashMap::new(),
                order: VecDeque::new(),
                capacity: capacity.max(1),
            }),
        }
    }

    pub fn get_or_create(
        &self,
        host_key: &str,
        max_per_host: u32,
    ) -> (Arc<HostConcurrencyGate>, Arc<HostCircuitState>) {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let (Some(gate), Some(circuit)) = (
            inner.gates.get(host_key).cloned(),
            inner.circuits.get(host_key).cloned(),
        ) {
            gate.raise_max_permits(max_per_host);
            inner.order.retain(|k| k != host_key);
            inner.order.push_back(host_key.to_string());
            return (gate, circuit);
        }

        if inner.gates.len() >= inner.capacity {
            if let Some(evicted) = inner.order.pop_front() {
                inner.gates.remove(&evicted);
                inner.circuits.remove(&evicted);
            }
        }

        let gate = Arc::new(HostConcurrencyGate::new(max_per_host));
        let circuit = Arc::new(HostCircuitState::new());
        let key = host_key.to_string();
        inner.order.push_back(key.clone());
        inner.gates.insert(key.clone(), gate.clone());
        inner.circuits.insert(key, circuit.clone());
        (gate, circuit)
    }
}

impl Default for HostRegistry {
    fn default() -> Self {
        Self::new(MAX_HOST_ENTRIES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_host_gate_dynamic_raise_and_raii_drop() {
        let gate = Arc::new(HostConcurrencyGate::new(1));
        let cancel = AtomicBool::new(false);

        let p1 = gate.acquire(&cancel, Duration::from_secs(1)).await.unwrap();
        assert_eq!(gate.active_permits(), 1);

        // Second acquire should block until raised
        let gate2 = Arc::clone(&gate);
        let cancel2 = Arc::new(AtomicBool::new(false));
        let c2 = Arc::clone(&cancel2);
        let h = tokio::spawn(async move { gate2.acquire(&c2, Duration::from_secs(2)).await });

        tokio::time::sleep(Duration::from_millis(30)).await;
        gate.raise_max_permits(2);
        let p2 = h.await.unwrap().expect("should acquire after raise");
        assert_eq!(gate.active_permits(), 2);

        drop(p1);
        drop(p2);
        assert_eq!(gate.active_permits(), 0);
    }
}
