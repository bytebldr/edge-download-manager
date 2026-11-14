//! Lifecycle states and transitions for the EDM Desktop Handoff Host.
//!
//! Manages cold start, warm standby, background listening, and graceful termination.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostState {
    /// Initializing subsystems, loading settings and token.
    Booting,
    /// Listening on 127.0.0.1:14370 and accepting downloads.
    Ready,
    /// UI minimized to tray, background engine active.
    Standby,
    /// Shutting down; completing downloads and flushing WAL.
    Terminating,
}

pub struct LifecycleManager {
    is_shutting_down: Arc<AtomicBool>,
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self {
            is_shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_terminating(&self) -> bool {
        self.is_shutting_down.load(Ordering::SeqCst)
    }

    pub fn mark_terminating(&self) {
        self.is_shutting_down.store(true, Ordering::SeqCst);
    }
}
