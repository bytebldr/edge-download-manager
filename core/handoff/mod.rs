//! Core Handoff Subsystem: Isolated Extension <-> Bridge <-> Desktop Communication.

pub mod contract {
    pub mod types;
}

pub mod host {
    pub mod auth;
    pub mod lifecycle;
    pub mod listener;
}

pub mod bridge {
    pub mod manifest;
    pub mod probe;
    pub mod spawn;
}
