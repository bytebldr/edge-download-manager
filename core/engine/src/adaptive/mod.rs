pub mod client_registry;
pub mod controller;
pub mod engine;
pub mod host_gate;
pub mod pipeline_writer;
pub mod range_set;
pub mod scheduler;
pub mod types;
pub mod worker;

pub use client_registry::{ClientPair, ClientRegistry, ProxyKey};
pub use controller::AdaptiveController;
pub use engine::{AdaptiveDownloadTask, AdaptiveEngine, TelemetryCallback};
pub use host_gate::{HostCircuitState, HostConcurrencyGate, HostPermit, HostRegistry};
pub use pipeline_writer::{PipelinedWriterHandle, PipelinedWriterTx, WriteChunk};
pub use range_set::RangeSet;
pub use scheduler::WorkScheduler;
pub use types::{
    AccelerationMode, AdaptiveConfig, AdaptiveDecision, AdaptiveTelemetry, TransportProtocol,
    WorkerStatus, WorkerTelemetry,
};
pub use worker::{AdaptiveWorker, AdaptiveWorkerEvent};
