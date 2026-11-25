use edm_core::packs::dash::{parse_mpd, DashTrack};
use edm_core::packs::{configure_hidden_command, YtDlpPack};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::command;

use crate::os::validate_url_security;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaStreamFormat {
    pub format_id: String,
    pub ext: String,
    pub resolution: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub bandwidth: Option<u64>,
    pub filesize: Option<u64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub language: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaSubtitleTrack {
    pub lang: String,
    pub name: Option<String>,
    pub ext: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPlaylistEntry {
    pub index: usize,
    pub id: String,
    pub title: String,
    pub duration_secs: Option<f64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaProbeInfo {
    pub url: String,
    pub title: String,
    pub is_live: bool,
    pub is_playlist: bool,
    pub duration_secs: Option<f64>,
    pub thumbnail: Option<String>,
    pub video_formats: Vec<MediaStreamFormat>,
    pub audio_formats: Vec<MediaStreamFormat>,
    pub subtitles: Vec<MediaSubtitleTrack>,
    pub entries: Vec<MediaPlaylistEntry>,
}

fn dash_track_to_format(track: &DashTrack, is_video: bool) -> MediaStreamFormat {
    let resolution = match (track.width, track.height) {
        (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
        _ => None,
    };
    let ext = track
        .mime_type
        .as_deref()
        .and_then(|m| m.split('/').nth(1))
        .unwrap_or("mp4")
        .to_string();

    MediaStreamFormat {
        format_id: track.id.clone(),
        ext,
        resolution,
        width: track.width,
        height: track.height,
        fps: None,
        bandwidth: Some(track.bandwidth),
        filesize: None,
        vcodec: if is_video { track.codecs.clone() } else { None },
        acodec: if !is_video { track.codecs.clone() } else { None },
        language: track.language.clone(),
        url: track.base_url.clone(),
    }
}

async fn probe_mpd_url(url: &str) -> Result<MediaProbeInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to initialize HTTP client: {}", e))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch MPD manifest: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} fetching MPD manifest", resp.status()));
    }

    let xml = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read MPD response: {}", e))?;

    let manifest = parse_mpd(&xml, url)?;
    let title = url
        .split('?')
        .next()
        .and_then(|p| p.rsplit('/').next())
        .filter(|s| !s.is_empty())
        .unwrap_or("DASH Stream")
        .to_string();

    Ok(MediaProbeInfo {
        url: url.to_string(),
        title,
        is_live: manifest.is_live,
        is_playlist: false,
        duration_secs: manifest.duration_secs,
        thumbnail: None,
        video_formats: manifest
            .video_tracks
            .iter()
            .map(|t| dash_track_to_format(t, true))
            .collect(),
        audio_formats: manifest
            .audio_tracks
            .iter()
            .map(|t| dash_track_to_format(t, false))
            .collect(),
        subtitles: manifest
            .subtitle_tracks
            .iter()
            .map(|t| MediaSubtitleTrack {
                lang: t.language.clone().unwrap_or_else(|| t.id.clone()),
                name: Some(t.id.clone()),
                ext: t
                    .mime_type
                    .as_deref()
                    .and_then(|m| m.split('/').nth(1))
                    .unwrap_or("vtt")
                    .to_string(),
                url: t.base_url.clone(),
            })
            .collect(),
        entries: Vec::new(),
    })
}

async fn probe_with_ytdlp(url: &str) -> Result<MediaProbeInfo, String> {
    let bin = YtDlpPack::locate_binary()
        .ok_or_else(|| "yt-dlp binary not found for media stream probing".to_string())?;

    let output = configure_hidden_command(
        &bin,
        [
            "--dump-single-json",
            "--no-warnings",
            "--flat-playlist",
            url,
        ],
    )
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .output()
    .await
    .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp probe failed: {}", stderr.trim()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Invalid JSON from yt-dlp: {}", e))?;

    let title = json["title"].as_str().unwrap_or("Untitled Media").to_string();
    let is_live = json["is_live"].as_bool().unwrap_or(false);
    let duration_secs = json["duration"].as_f64();
    let thumbnail = json["thumbnail"].as_str().map(str::to_string);

    let mut video_formats = Vec::new();
    let mut audio_formats = Vec::new();

    if let Some(formats) = json["formats"].as_array() {
        for f in formats {
            let vcodec = f["vcodec"].as_str().map(str::to_string);
            let acodec = f["acodec"].as_str().map(str::to_string);
            let has_video = vcodec.as_deref().is_some_and(|v| v != "none");
            let has_audio = acodec.as_deref().is_some_and(|a| a != "none");

            let width = f["width"].as_u64().map(|w| w as u32);
            let height = f["height"].as_u64().map(|h| h as u32);
            let resolution = f["resolution"]
                .as_str()
                .map(str::to_string)
                .or_else(|| match (width, height) {
                    (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
                    _ => None,
                });

            let item = MediaStreamFormat {
                format_id: f["format_id"].as_str().unwrap_or("unknown").to_string(),
                ext: f["ext"].as_str().unwrap_or("mp4").to_string(),
                resolution,
                width,
                height,
                fps: f["fps"].as_f64(),
                bandwidth: f["tbr"].as_f64().map(|kbps| (kbps * 1000.0) as u64),
                filesize: f["filesize"]
                    .as_u64()
                    .or_else(|| f["filesize_approx"].as_u64()),
                vcodec,
                acodec,
                language: f["language"].as_str().map(str::to_string),
                url: f["url"].as_str().map(str::to_string),
            };

            if has_video {
                video_formats.push(item);
            } else if has_audio {
                audio_formats.push(item);
            }
        }
    }

    video_formats.sort_by(|a, b| {
        b.height
            .unwrap_or(0)
            .cmp(&a.height.unwrap_or(0))
            .then_with(|| b.bandwidth.unwrap_or(0).cmp(&a.bandwidth.unwrap_or(0)))
    });
    audio_formats.sort_by(|a, b| b.bandwidth.unwrap_or(0).cmp(&a.bandwidth.unwrap_or(0)));

    let mut subtitles = Vec::new();
    let mut seen_langs = HashSet::new();
    for key in ["subtitles", "automatic_captions"] {
        if let Some(subs_obj) = json[key].as_object() {
            for (lang, tracks) in subs_obj {
                if !seen_langs.insert(lang.clone()) {
                    continue;
                }
                if let Some(first) = tracks.as_array().and_then(|arr| arr.first()) {
                    subtitles.push(MediaSubtitleTrack {
                        lang: lang.clone(),
                        name: first["name"].as_str().map(str::to_string),
                        ext: first["ext"].as_str().unwrap_or("vtt").to_string(),
                        url: first["url"].as_str().map(str::to_string),
                    });
                }
            }
        }
    }

    let mut entries = Vec::new();
    if let Some(raw_entries) = json["entries"].as_array() {
        for (idx, entry) in raw_entries.iter().enumerate() {
            let entry_url = entry["url"]
                .as_str()
                .or_else(|| entry["webpage_url"].as_str())
                .unwrap_or("")
                .to_string();
            if entry_url.is_empty() {
                continue;
            }
            entries.push(MediaPlaylistEntry {
                index: idx + 1,
                id: entry["id"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{}", idx + 1)),
                title: entry["title"]
                    .as_str()
                    .unwrap_or("Playlist Item")
                    .to_string(),
                duration_secs: entry["duration"].as_f64(),
                url: entry_url,
            });
        }
    }

    let is_playlist = !entries.is_empty() || json["_type"].as_str() == Some("playlist");

    Ok(MediaProbeInfo {
        url: url.to_string(),
        title,
        is_live,
        is_playlist,
        duration_secs,
        thumbnail,
        video_formats,
        audio_formats,
        subtitles,
        entries,
    })
}

/// Probe a DASH `.mpd` manifest or media URL via `yt-dlp` for stream/track selection.
#[command]
pub async fn probe_media_streams(url: String) -> Result<MediaProbeInfo, String> {
    validate_url_security(&url)?;

    let clean_path = url.split('?').next().unwrap_or("").to_ascii_lowercase();
    if clean_path.ends_with(".mpd") {
        if let Ok(info) = probe_mpd_url(&url).await {
            return Ok(info);
        }
    }

    probe_with_ytdlp(&url).await
}
