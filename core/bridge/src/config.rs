use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

pub const WS_PORT: u16 = 14370;
pub const WS_ADDR: &str = "127.0.0.1:14370";
pub const HTTP_BRIDGE_ADDR: &str = "127.0.0.1:8998";
pub const FORWARD_ENDPOINTS: [&str; 2] =
    ["http://127.0.0.1:14370/api", "http://127.0.0.1:8998/api"];
pub const FORWARD_ORIGIN: &str = "chrome-extension://ginpdimhcdbojbhemgjedghkeolbcmgj";
pub const MAX_NMH_MESSAGE_BYTES: usize = 10 * 1024 * 1024;

pub fn init_nmh_logging() {
    let log_dir = match env::var("LOCALAPPDATA") {
        Ok(dir) => PathBuf::from(dir).join("EDM").join("logs"),
        Err(_) => env::temp_dir().join("EDM").join("logs"),
    };
    let _ = fs::create_dir_all(&log_dir);

    // Rotating daily log file edm-nmh.YYYY-MM-DD.log
    let file_appender = tracing_appender::rolling::daily(&log_dir, "edm-nmh.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(guard));

    let subscriber = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}

pub fn get_ws_token() -> Option<String> {
    let config_dir = match env::var("APPDATA") {
        Ok(dir) => PathBuf::from(dir).join("EDM").join("config"),
        Err(_) => match env::var("LOCALAPPDATA") {
            Ok(dir) => PathBuf::from(dir).join("EDM").join("config"),
            Err(_) => env::temp_dir().join("EDM").join("config"),
        },
    };
    let token_file = config_dir.join("token.txt");
    if let Ok(content) = fs::read_to_string(&token_file) {
        let trimmed = content.trim().to_string();
        if trimmed.len() >= 32 {
            return Some(trimmed);
        }
    }
    None
}

pub fn verify_caller_origin() -> bool {
    let start = Instant::now();
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        tracing::warn!("Origin verification rejected: no external caller args passed");
        return false;
    }
    for arg in &args[1..] {
        if edm_ipc::extensions::is_allowed_origin(arg) {
            tracing::info!(
                "Origin verified: matched allowed extension origin {} (elapsed: {:?})",
                arg,
                start.elapsed()
            );
            return true;
        }
        if edm_ipc::extensions::is_allowed_extension_id(arg) {
            tracing::info!(
                "Origin verified: matched allowed extension id directly {} (elapsed: {:?})",
                arg,
                start.elapsed()
            );
            return true;
        }
    }
    tracing::warn!(
        "Origin verification rejected: args {:?} not in allowed list (elapsed: {:?})",
        &args[1..],
        start.elapsed()
    );
    false
}

pub fn is_allowed_http_origin(origin: &str) -> bool {
    edm_ipc::extensions::is_allowed_origin(origin)
}

pub fn is_loopback_host(host: &str) -> bool {
    matches!(
        host.trim(),
        "127.0.0.1:8998" | "localhost:8998" | "127.0.0.1" | "localhost"
    )
}
