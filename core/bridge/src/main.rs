mod config;
mod handlers;
mod http_api;
mod nmh;

use edm_ipc::AppReply;
use std::env;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[tokio::main]
async fn main() -> io::Result<()> {
    config::init_nmh_logging();
    let host_start = Instant::now();
    let args: Vec<String> = env::args().collect();
    tracing::info!(
        "Host started. PID: {}, Args: {:?}",
        std::process::id(),
        args
    );

    if !config::verify_caller_origin() {
        tracing::error!("Security violation: Untrusted caller origin");
        let reply = AppReply::Error {
            message: "Security violation: Untrusted caller origin".into(),
        };
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let _ = nmh::write_reply(&mut writer, &reply);
        return Ok(());
    }

    let store = match handlers::open_store() {
        Ok(s) => Arc::new(Mutex::new(s)),
        Err(e) => {
            tracing::error!("{}", e);
            let reply = AppReply::Error { message: e };
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            let _ = nmh::write_reply(&mut writer, &reply);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Database initialization failed",
            ));
        }
    };

    // If --serve argument is provided, start local loopback HTTP API for fallback connectivity
    if args.iter().any(|a| a == "--serve") {
        return http_api::serve(store).await;
    }

    // Default: Native Messaging Host over length-prefixed stdin/stdout
    nmh::run_stdio_loop(&store, host_start).await;
    Ok(())
}
