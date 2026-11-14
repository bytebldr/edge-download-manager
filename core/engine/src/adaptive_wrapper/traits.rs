use super::types::{PostFlightContext, PreFlightPlan};
use crate::adaptive::{AdaptiveDownloadTask, TelemetryCallback};
use async_trait::async_trait;
use reqwest::Client;
use std::path::PathBuf;

#[async_trait]
pub trait AdaptiveWrapperAdapter: Send + Sync {
    /// Inspect target, perform DNS/HEAD/race probes, and formulate the execution plan
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        client: &Client,
    ) -> Result<PreFlightPlan, String>;

    /// Execute post-download verification, transmuxing, or temporary file cleanup
    async fn post_flight(
        &self,
        ctx: PostFlightContext,
        telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String>;
}
