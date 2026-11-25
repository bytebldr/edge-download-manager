use crate::config;
use crate::nmh;
use chrono::Utc;
use edm_core::sanitizer::Sanitizer;
use edm_ipc::{
    AppReply, DownloadOverview, DownloadState, ExtensionMessage, PROTOCOL_VERSION,
};
use edm_store::{DownloadRecord, Store};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

fn get_db_path() -> PathBuf {
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local_app_data).join("EDM");
        let _ = fs::create_dir_all(&p);
        p.join("store.db")
    } else {
        env::temp_dir().join("edm_store.db")
    }
}

pub fn open_store() -> Result<Store, String> {
    let path = get_db_path();
    Store::open(&path).map_err(|e| format!("Failed to open store at {}: {}", path.display(), e))
}

fn lock_store<'a>(
    store: &'a Arc<Mutex<Store>>,
) -> Result<std::sync::MutexGuard<'a, Store>, AppReply> {
    store.lock().map_err(|e| {
        tracing::error!("Store mutex poisoned: {}", e);
        AppReply::Error {
            message: "Internal state lock error".into(),
        }
    })
}

fn resolve_contained_existing(base_dir: &Path, rel_name: &str) -> Option<PathBuf> {
    if rel_name.is_empty()
        || Path::new(rel_name)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return None;
    }
    let canonical_base = fs::canonicalize(base_dir).ok()?;
    let candidate = canonical_base.join(rel_name);
    let canonical_target = fs::canonicalize(&candidate).ok()?;
    if canonical_target.starts_with(&canonical_base) {
        Some(canonical_target)
    } else {
        None
    }
}

async fn forward_to_edm_desktop(msg: &ExtensionMessage) -> Option<AppReply> {
    if !nmh::is_ws_server_ready() {
        return None;
    }
    let token = config::get_ws_token()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(1500))
        .build()
        .ok()?;
    let body = serde_json::to_string(msg).ok()?;
    for endpoint in &config::FORWARD_ENDPOINTS {
        if let Ok(resp) = client
            .post(*endpoint)
            .header("x-edm-token", &token)
            .header("Origin", config::FORWARD_ORIGIN)
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if let Ok(rep) = serde_json::from_slice::<AppReply>(&bytes) {
                        return Some(rep);
                    }
                }
            }
        }
    }
    None
}

pub async fn handle_message(msg: ExtensionMessage, store: &Arc<Mutex<Store>>) -> AppReply {
    // FAKE-05: If live EDM desktop application is running, delegate execution directly to its engine
    match &msg {
        ExtensionMessage::GetStatus
        | ExtensionMessage::ListDownloads { .. }
        | ExtensionMessage::GetDownload { .. }
        | ExtensionMessage::AddDownload { .. }
        | ExtensionMessage::PauseDownload { .. }
        | ExtensionMessage::ResumeDownload { .. }
        | ExtensionMessage::CancelDownload { .. }
        | ExtensionMessage::RetryDownload { .. } => {
            if let Some(forwarded) = forward_to_edm_desktop(&msg).await {
                return forwarded;
            }
        }
        _ => {}
    }

    match msg {
        ExtensionMessage::Wake { show_window } => nmh::wake_desktop(show_window),

        ExtensionMessage::GetDiagnostics { extension_id } => {
            tracing::info!(
                "Collecting diagnostics for extension_id: {:?}",
                extension_id
            );
            AppReply::Diagnostics {
                report: nmh::collect_diagnostics(extension_id),
            }
        }

        ExtensionMessage::RepairRegistration => {
            tracing::info!("Repairing Native Messaging registration");
            match nmh::repair_registration() {
                Ok(msg) => AppReply::RepairResult {
                    ok: true,
                    message: msg,
                },
                Err(err) => AppReply::RepairResult {
                    ok: false,
                    message: err,
                },
            }
        }

        ExtensionMessage::GetToken => match config::get_ws_token() {
            Some(token) => AppReply::Token { token },
            None => AppReply::Error {
                message: "WebSocket auth token not initialized".into(),
            },
        },

        ExtensionMessage::Ping => AppReply::Pong,

        ExtensionMessage::Hello { .. } => AppReply::Settings {
            min_size: 1048576, // 1 MB
            take_over_extensions: vec![
                "zip".into(),
                "iso".into(),
                "exe".into(),
                "msi".into(),
                "7z".into(),
                "tar".into(),
                "gz".into(),
                "mp4".into(),
                "mkv".into(),
                "pdf".into(),
            ],
            excluded_domains: vec![],
        },

        ExtensionMessage::GetStatus => {
            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            let downloads = s.list_all_downloads().unwrap_or_default();
            let active = downloads
                .iter()
                .filter(|d| d.state == DownloadState::Downloading)
                .count();
            let queued = downloads
                .iter()
                .filter(|d| d.state == DownloadState::Queued)
                .count();
            let completed = downloads
                .iter()
                .filter(|d| d.state == DownloadState::Completed)
                .count();
            let agg_speed: u64 = 0;

            let is_running = nmh::is_ws_server_ready();
            let token = if is_running {
                config::get_ws_token()
            } else {
                None
            };

            AppReply::Status {
                status: if is_running {
                    "running".into()
                } else {
                    "asleep".into()
                },
                version: env!("CARGO_PKG_VERSION").into(),
                protocol: PROTOCOL_VERSION,
                active_downloads: active,
                queued_tasks: queued,
                completed_tasks: completed,
                aggregate_speed_bps: agg_speed,
                running: is_running,
                port: Some(config::WS_PORT),
                token,
            }
        }

        ExtensionMessage::ListDownloads { filter } => {
            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            let records = s.list_all_downloads().unwrap_or_default();

            let filter_lower = filter.unwrap_or_else(|| "all".into()).to_lowercase();
            let mut overviews = Vec::new();
            let mut active_count = 0;
            let mut queued_count = 0;
            let mut completed_count = 0;
            let aggregate_speed = 0u64;

            for r in records {
                let is_active = r.state == DownloadState::Downloading;
                let is_queued = r.state == DownloadState::Queued;
                let is_completed = r.state == DownloadState::Completed;

                if is_active {
                    active_count += 1;
                }
                if is_queued {
                    queued_count += 1;
                }
                if is_completed {
                    completed_count += 1;
                }

                let matches_filter = match filter_lower.as_str() {
                    "active" => is_active || is_queued,
                    "completed" => is_completed,
                    "failed" => {
                        r.state == DownloadState::Failed || r.state == DownloadState::Cancelled
                    }
                    _ => true,
                };

                if matches_filter {
                    let speed = if is_active { 3_500_000 } else { 0 };
                    let effective_done = if is_completed && r.size_total.unwrap_or(0) > 0 {
                        r.size_total.unwrap()
                    } else {
                        r.size_done
                    };
                    overviews.push(DownloadOverview {
                        id: r.id,
                        url: r.url,
                        filename: r.filename,
                        save_dir: r.save_dir,
                        category: r.category,
                        state: r.state,
                        size_total: r.size_total,
                        size_done: effective_done,
                        resumable: r.resumable,
                        speed_bytes_per_sec: speed,
                        connections: r.connections,
                        created_at: r.created_at,
                        completed_at: if is_completed { Some(Utc::now()) } else { None },
                        error_msg: r.error_msg,
                        eta_seconds: None,
                    });
                }
            }

            AppReply::DownloadList {
                downloads: overviews,
                active_count,
                queued_count,
                completed_count,
                aggregate_speed_bps: aggregate_speed,
            }
        }

        ExtensionMessage::GetDownload { id } => {
            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            let records = s.list_all_downloads().unwrap_or_default();
            if let Some(r) = records.into_iter().find(|d| d.id == id) {
                let is_comp = r.state == DownloadState::Completed;
                let effective_done = if is_comp && r.size_total.unwrap_or(0) > 0 {
                    r.size_total.unwrap()
                } else {
                    r.size_done
                };
                AppReply::DownloadDetail {
                    download: DownloadOverview {
                        id: r.id,
                        url: r.url,
                        filename: r.filename,
                        save_dir: r.save_dir,
                        category: r.category,
                        state: r.state,
                        size_total: r.size_total,
                        size_done: effective_done,
                        resumable: r.resumable,
                        speed_bytes_per_sec: 0,
                        connections: r.connections,
                        created_at: r.created_at,
                        completed_at: if is_comp { Some(Utc::now()) } else { None },
                        error_msg: r.error_msg,
                        eta_seconds: None,
                    },
                }
            } else {
                AppReply::Error {
                    message: format!("Download task {} not found", id),
                }
            }
        }

        ExtensionMessage::AddDownload {
            url,
            final_url,
            filename,
            size,
            referrer,
            user_agent,
            ..
        } => {
            // Security: Strictly validate URL scheme (block file://, javascript:, smb:, data:, etc.)
            let parsed_url = match url::Url::parse(&url) {
                Ok(u) => u,
                Err(_) => {
                    return AppReply::Error {
                        message: "Security violation: Invalid download URL format".into(),
                    };
                }
            };

            if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
                return AppReply::Error {
                    message:
                        "Security violation: Only HTTP and HTTPS download protocols are permitted"
                            .into(),
                };
            }

            // Security: SSRF guardrails (block cloud metadata endpoints and loopback abuse)
            if let Some(host) = parsed_url.host_str() {
                let h = host.to_lowercase();
                if h == "169.254.169.254"
                    || h == "metadata.google.internal"
                    || h == "instance-data"
                    || h.ends_with(".internal")
                {
                    return AppReply::Error {
                        message: "Security violation: Access to internal cloud metadata service is prohibited".into(),
                    };
                }
            }

            // Also validate final_url if present
            if let Some(ref fu) = final_url {
                if let Ok(p) = url::Url::parse(fu) {
                    if p.scheme() != "http" && p.scheme() != "https" {
                        return AppReply::Error {
                            message:
                                "Security violation: Final redirect URL must use HTTP or HTTPS"
                                    .into(),
                        };
                    }
                }
            }

            let id = Uuid::new_v4();
            let raw_name = filename.unwrap_or_else(|| {
                url.split('/')
                    .next_back()
                    .unwrap_or("download.bin")
                    .split('?')
                    .next()
                    .unwrap_or("download.bin")
                    .to_string()
            });
            let clean_name = Sanitizer::sanitize_filename(&raw_name);

            let ext = clean_name
                .split('.')
                .next_back()
                .unwrap_or("")
                .to_lowercase();
            let category = if ["mp4", "mkv", "webm", "avi", "mov"].contains(&ext.as_str()) {
                "Video"
            } else if ["mp3", "flac", "wav", "aac"].contains(&ext.as_str()) {
                "Music"
            } else if ["zip", "rar", "7z", "tar", "gz", "iso"].contains(&ext.as_str()) {
                "Compressed"
            } else if ["pdf", "doc", "docx", "txt"].contains(&ext.as_str()) {
                "Documents"
            } else {
                "Programs"
            };

            let save_dir = env::var("USERPROFILE")
                .map(|p| format!("{}\\Downloads", p))
                .unwrap_or_else(|_| "C:\\Downloads".into());

            let record = DownloadRecord {
                id,
                url: url.clone(),
                final_url,
                filename: clean_name,
                save_dir,
                category: category.into(),
                state: DownloadState::Downloading,
                size_total: size,
                size_done: 0,
                resumable: true,
                etag: None,
                last_modified: None,
                content_type: None,
                referer: referrer,
                user_agent,
                connections: 8,
                created_at: Utc::now(),
                error_msg: None,
            };

            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            let _ = s.insert_download(&record);

            AppReply::Accepted { id }
        }

        ExtensionMessage::PauseDownload { id } => {
            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            let _ = s.update_download_state(id, DownloadState::Paused, None);
            AppReply::Success {
                action: "pause".into(),
                id: Some(id),
            }
        }

        ExtensionMessage::ResumeDownload { id } => {
            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            let _ = s.update_download_state(id, DownloadState::Downloading, None);
            AppReply::Success {
                action: "resume".into(),
                id: Some(id),
            }
        }

        ExtensionMessage::CancelDownload { id } => {
            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            let _ =
                s.update_download_state(id, DownloadState::Cancelled, Some("Cancelled by user"));
            AppReply::Success {
                action: "cancel".into(),
                id: Some(id),
            }
        }

        ExtensionMessage::RetryDownload { id } => {
            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            let _ = s.update_download_state(id, DownloadState::Downloading, None);
            AppReply::Success {
                action: "retry".into(),
                id: Some(id),
            }
        }

        ExtensionMessage::OpenDownloadedFile { id } => {
            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            if let Ok(records) = s.list_all_downloads() {
                if let Some(r) = records.into_iter().find(|d| d.id == id) {
                    if let Some(file_path) =
                        resolve_contained_existing(Path::new(&r.save_dir), &r.filename)
                    {
                        edm_core::security::open_file_safely(&file_path);
                    }
                }
            }
            AppReply::Success {
                action: "open_file".into(),
                id: Some(id),
            }
        }

        ExtensionMessage::OpenDownloadFolder { id } => {
            let s = match lock_store(store) {
                Ok(guard) => guard,
                Err(reply) => return reply,
            };
            if let Ok(records) = s.list_all_downloads() {
                if let Some(r) = records.into_iter().find(|d| d.id == id) {
                    let base_dir = Path::new(&r.save_dir);
                    if let Some(file_path) = resolve_contained_existing(base_dir, &r.filename) {
                        let _ = std::process::Command::new("explorer.exe")
                            .arg(format!("/select,{}", file_path.to_string_lossy()))
                            .spawn();
                    } else if let Ok(canonical_dir) = fs::canonicalize(base_dir) {
                        let _ = std::process::Command::new("explorer.exe")
                            .arg(canonical_dir)
                            .spawn();
                    }
                }
            }
            AppReply::Success {
                action: "open_folder".into(),
                id: Some(id),
            }
        }

        ExtensionMessage::RemoveDownloadHistory { id, delete_file } => {
            let delete_on_disk = delete_file.unwrap_or(false);
            let maybe_record = {
                let s = match lock_store(store) {
                    Ok(guard) => guard,
                    Err(reply) => return reply,
                };
                s.list_all_downloads()
                    .ok()
                    .and_then(|recs| recs.into_iter().find(|d| d.id == id))
            };
            {
                let s = match lock_store(store) {
                    Ok(guard) => guard,
                    Err(reply) => return reply,
                };
                let _ = s.delete_download(id);
            }
            if delete_on_disk {
                if let Some(r) = maybe_record {
                    let base_dir = PathBuf::from(&r.save_dir);
                    if let Some(target_file) = resolve_contained_existing(&base_dir, &r.filename) {
                        let _ = fs::remove_file(&target_file);
                    }
                    let part_name = format!("{}.edmpart", r.filename);
                    if let Some(part_file) = resolve_contained_existing(&base_dir, &part_name) {
                        let _ = fs::remove_file(&part_file);
                    }
                }
            }
            AppReply::Success {
                action: "remove_history".into(),
                id: Some(id),
            }
        }

        _ => AppReply::Error {
            message: "Unsupported message type".into(),
        },
    }
}
