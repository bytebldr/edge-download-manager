use clap::{Parser, Subcommand};
use edm_core::adaptive::{AdaptiveConfig, AdaptiveDownloadTask, AdaptiveEngine, AdaptiveTelemetry};
use edm_core::Probe;
use edm_store::Store;
use reqwest::Client;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(author, version, about = "EDM Headless CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Probe a remote URL and inspect server headers/capabilities
    Probe { url: String },
    /// Download a file using the adaptive acceleration engine
    Download {
        url: String,
        #[arg(short, long, default_value = ".")]
        output_dir: PathBuf,
        #[arg(short, long, default_value_t = 8)]
        connections: u32,
        #[arg(long, default_value_t = true)]
        adaptive: bool,
    },
    /// Compare two files and report sizes, SHA-256 hashes, and differing byte ranges
    Verify {
        file_a: PathBuf,
        file_b: PathBuf,
        #[arg(short, long)]
        chunk_size: Option<u64>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::WARN)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let cli = Cli::parse();
    match cli.command {
        Commands::Probe { url } => {
            let client = Client::new();
            println!("Probing {} ...", url);
            let result = Probe::probe_url(&client, &url).await?;
            println!("{:#?}", result);
        }
        Commands::Download {
            url,
            output_dir,
            connections,
            adaptive,
        } => {
            let store = Store::open_in_memory()?;
            let engine = AdaptiveEngine::new(store);
            let clamped = connections.clamp(1, 64);

            let config = if adaptive {
                AdaptiveConfig {
                    max_connections: clamped,
                    max_connections_per_host: clamped,
                    ..AdaptiveConfig::default()
                }
            } else {
                AdaptiveConfig {
                    auto_scale: false,
                    initial_connections: clamped,
                    min_connections: clamped,
                    max_connections: clamped,
                    max_connections_per_host: clamped,
                    ..AdaptiveConfig::default()
                }
            };

            let task = AdaptiveDownloadTask {
                id: Uuid::new_v4(),
                url: url.clone(),
                filename: "download".into(),
                save_dir: output_dir,
                config,
                limiter: None,
                cancelled: Arc::new(AtomicBool::new(false)),
                cookies: None,
                referer: None,
                user_agent: None,
                selected_files: None,
                torrent_power: None,
            };

            let telemetry_cb = Arc::new(|telem: AdaptiveTelemetry| {
                let speed_mbps = telem.instant_throughput_bps as f64 / 1_000_000.0;
                let speed_mbs = speed_mbps / 8.0;
                let eta_str = telem
                    .eta_seconds
                    .map(|s| format!("{}s", s))
                    .unwrap_or_else(|| "--".into());

                print!(
                    "\rProgress: {:>5.1}% | Speed: {:>5.1} MB/s | Streams: {:>2} [{:<10}] | ETA: {:>4} | {}",
                    telem.progress_pct,
                    speed_mbs,
                    telem.active_connections,
                    telem.mode.as_str(),
                    eta_str,
                    telem.decision_reason
                );
                let _ = stdout().flush();
            });

            if adaptive {
                println!("Starting Adaptive Download for {}", url);
            } else {
                println!("Starting Fixed-Concurrency Download ({} streams) for {}", clamped, url);
            }
            let saved_path = engine.download(task, Some(telemetry_cb)).await?;
            println!("\nDownloaded successfully to: {:?}", saved_path);
        }
        Commands::Verify {
            file_a,
            file_b,
            chunk_size,
        } => {
            let report = edm_core::compare_files(&file_a, &file_b, chunk_size, &[])?;
            print!("{}", report.format_summary());
            if !report.is_identical {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
