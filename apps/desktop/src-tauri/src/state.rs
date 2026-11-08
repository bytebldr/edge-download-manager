use edm_core::adaptive::{AdaptiveEngine, AdaptiveTelemetry};
use edm_store::Store;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ActiveTaskHandle {
    pub cancelled: Arc<AtomicBool>,
    pub telemetry: Arc<Mutex<Option<AdaptiveTelemetry>>>,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub engine: Arc<AdaptiveEngine>,
    pub tasks: Arc<Mutex<HashMap<String, ActiveTaskHandle>>>,
    pub rt_handle: tokio::runtime::Handle,
    pub ws_manager: Arc<crate::ws_server::WsManager>,
    pub tray_handle: Arc<Mutex<Option<tauri::tray::TrayIcon>>>,
    pub app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
    pub lifecycle: Arc<crate::lifecycle::LifecycleManager>,
    pub power_countdown: Arc<crate::features::PowerCountdownController>,
    pub global_limiter: Arc<edm_core::TokenBucketLimiter>,
}
