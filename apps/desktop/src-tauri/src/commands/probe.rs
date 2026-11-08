use edm_core::Probe;
use reqwest::Client;
use std::path::PathBuf;
use tauri::{command, State};
use uuid::Uuid;

use crate::features;
use crate::os::validate_url_security;
use crate::services::download_manager::execute_download;
use crate::state::AppState;

#[command]
pub async fn probe_url(url: String) -> Result<serde_json::Value, String> {
    validate_url_security(&url)?;
    let pack_target = edm_core::FeaturePackManager::classify_target(&url);

    // 1. BitTorrent Magnet Link Probe
    if pack_target.pack_id == edm_core::FeaturePackId::BitTorrentPack {
        return Ok(serde_json::json!({
            "url": url,
            "final_url": url,
            "filename": pack_target.suggested_filename.or(Some("Torrent_Task".into())),
            "content_type": Some("application/x-bittorrent"),
            "size_total": null,
            "resumable": true,
            "etag": null,
            "last_modified": null
        }));
    }

    // 2. eDonkey2000 Link Probe
    if pack_target.pack_id == edm_core::FeaturePackId::Ed2kPack {
        return Ok(serde_json::json!({
            "url": url,
            "final_url": url,
            "filename": pack_target.suggested_filename.or(Some("eD2k_File".into())),
            "content_type": Some("application/x-ed2k"),
            "size_total": pack_target.estimated_size,
            "resumable": true,
            "etag": null,
            "last_modified": null
        }));
    }

    // 3. Video Stream Probe (Prioritize yt-dlp binary with Node JS solver, then native extractors)
    if pack_target.pack_id == edm_core::FeaturePackId::YtDlpPack {
        if edm_core::packs::YtDlpPack::locate_binary().is_some() {
            if let Ok(info) = edm_core::packs::YtDlpPack::extract_info(&url).await {
                let best_size = info.formats.iter().filter_map(|f| f.filesize).max();
                let sanitized = edm_core::sanitizer::Sanitizer::sanitize_filename(&info.title);
                let final_stream_url = info.best_video_url.unwrap_or_else(|| info.webpage_url.clone());
                return Ok(serde_json::json!({
                    "url": url,
                    "final_url": final_stream_url,
                    "filename": Some(format!("{}.mp4", sanitized)),
                    "content_type": Some("video/mp4"),
                    "size_total": best_size,
                    "resumable": true,
                    "etag": null,
                    "last_modified": null
                }));
            }
        }

        use edm_core::packs::extractors::{SiteExtractor, VimeoExtractor, YouTubeExtractor};
        let yt = YouTubeExtractor;
        let vimeo = VimeoExtractor;
        let probe_client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default();

        if yt.can_extract(&url) {
            if let Ok(meta) = yt.extract(&url, &probe_client).await {
                let best_size = meta.formats.iter().filter_map(|f| f.filesize).max();
                let sanitized = edm_core::sanitizer::Sanitizer::sanitize_filename(&meta.title);
                let final_stream_url = meta.best_stream_url.unwrap_or_else(|| url.clone());
                return Ok(serde_json::json!({
                    "url": url,
                    "final_url": final_stream_url,
                    "filename": Some(format!("{}.mp4", sanitized)),
                    "content_type": Some("video/mp4"),
                    "size_total": best_size,
                    "resumable": true,
                    "etag": null,
                    "last_modified": null
                }));
            }
        } else if vimeo.can_extract(&url) {
            if let Ok(meta) = vimeo.extract(&url, &probe_client).await {
                let best_size = meta.formats.iter().filter_map(|f| f.filesize).max();
                let sanitized = edm_core::sanitizer::Sanitizer::sanitize_filename(&meta.title);
                let final_stream_url = meta.best_stream_url.unwrap_or_else(|| url.clone());
                return Ok(serde_json::json!({
                    "url": url,
                    "final_url": final_stream_url,
                    "filename": Some(format!("{}.mp4", sanitized)),
                    "content_type": Some("video/mp4"),
                    "size_total": best_size,
                    "resumable": true,
                    "etag": null,
                    "last_modified": null
                }));
            }
        }

        let video_name = pack_target.suggested_filename.unwrap_or_else(|| "YouTube_Video.mp4".into());
        return Ok(serde_json::json!({
            "url": url,
            "final_url": url,
            "filename": Some(video_name),
            "content_type": Some("video/mp4"),
            "size_total": pack_target.estimated_size,
            "resumable": true,
            "etag": null,
            "last_modified": null
        }));
    }

    // 4. Standard HTTP / M3U8 / GitHub / HuggingFace Probe
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::limited(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36 Edg/135.0.0.0")
        .build()
        .unwrap_or_default();
    let res = Probe::probe_url(&client, &url)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(res).map_err(|e| e.to_string())
}

#[command]
pub async fn refresh_download_link(
    state: State<'_, AppState>,
    task_id: String,
    new_url: String,
) -> Result<(), String> {
    let id = Uuid::parse_str(&task_id).map_err(|e| e.to_string())?;
    let task = features::validate_and_refresh_link(&state.store, id, &new_url, None).await?;
    let _ = execute_download(
        &state,
        task.id,
        task.url,
        Some(task.filename),
        Some(PathBuf::from(task.save_dir)),
        Some(task.connections),
        task.size_total,
        None,
        task.referer,
        task.user_agent,
    );
    Ok(())
}

#[command]
pub async fn probe_media_page(media_url: String) -> Result<features::MediaProbeResult, String> {
    features::probe_media(&media_url).await
}
