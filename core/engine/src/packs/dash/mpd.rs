//! Pure-Rust MPEG-DASH (.mpd) Manifest Parser
//! Parses static & dynamic MPD manifests into video, audio, and subtitle tracks.

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashSegmentTemplate {
    pub initialization: Option<String>,
    pub media: Option<String>,
    pub timescale: u64,
    pub duration: Option<u64>,
    pub start_number: u64,
}

impl DashSegmentTemplate {
    /// Resolve template placeholders ($RepresentationID$, $Number$, $Number%0Nd$, $Bandwidth$, $Time$).
    pub fn resolve_pattern(
        pattern: &str,
        rep_id: &str,
        number: Option<u64>,
        bandwidth: u64,
        time: Option<u64>,
    ) -> String {
        let mut out = pattern.replace("$RepresentationID$", rep_id);
        out = out.replace("$Bandwidth$", &bandwidth.to_string());
        if let Some(t) = time {
            out = out.replace("$Time$", &t.to_string());
        }
        if let Some(n) = number {
            out = out.replace("$Number$", &n.to_string());
            // Handle $Number%0Nd$ format specifiers
            while let Some(start) = out.find("$Number%0") {
                if let Some(rel_end) = out[start..].find("d$") {
                    let width_str = &out[start + 9..start + rel_end];
                    if let Ok(width) = width_str.parse::<usize>() {
                        let formatted = format!("{:0width$}", n, width = width);
                        out.replace_range(start..start + rel_end + 2, &formatted);
                        continue;
                    }
                }
                break;
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashTrack {
    pub id: String,
    pub mime_type: Option<String>,
    pub codecs: Option<String>,
    pub bandwidth: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub language: Option<String>,
    pub base_url: Option<String>,
    pub segment_template: Option<DashSegmentTemplate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashManifest {
    pub is_live: bool,
    pub duration_secs: Option<f64>,
    pub base_url: Option<String>,
    pub video_tracks: Vec<DashTrack>,
    pub audio_tracks: Vec<DashTrack>,
    pub subtitle_tracks: Vec<DashTrack>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackKind {
    Video,
    Audio,
    Subtitle,
}

/// Parse an MPEG-DASH `.mpd` XML manifest into a structured `DashManifest`.
pub fn parse_mpd(xml: &str, manifest_url: &str) -> Result<DashManifest, String> {
    let trimmed = xml.trim();
    if !trimmed.contains("<MPD") {
        return Err("Invalid MPD manifest: missing <MPD> root element".into());
    }

    let mpd_attrs = extract_tag_attrs(trimmed, "MPD").unwrap_or_default();
    let mpd_type = get_attr(&mpd_attrs, "type").unwrap_or("static");
    let is_live = mpd_type.eq_ignore_ascii_case("dynamic");

    let duration_secs = get_attr(&mpd_attrs, "mediaPresentationDuration").and_then(parse_iso_duration);

    let mpd_base = extract_direct_base_url(trimmed)
        .map(|u| resolve_url(manifest_url, &u))
        .unwrap_or_else(|| manifest_url.to_string());

    let mut video_tracks = Vec::new();
    let mut audio_tracks = Vec::new();
    let mut subtitle_tracks = Vec::new();

    let periods = extract_blocks(trimmed, "Period");
    let period_slices: Vec<&str> = if periods.is_empty() {
        vec![trimmed]
    } else {
        periods
    };

    for period in period_slices {
        let period_base = extract_direct_base_url(period)
            .map(|u| resolve_url(&mpd_base, &u))
            .unwrap_or_else(|| mpd_base.clone());

        for adapt in extract_blocks(period, "AdaptationSet") {
            let adapt_attrs = extract_tag_attrs(adapt, "AdaptationSet").unwrap_or_default();
            let adapt_content_type = get_attr(&adapt_attrs, "contentType").map(str::to_string);
            let adapt_mime = get_attr(&adapt_attrs, "mimeType").map(str::to_string);
            let adapt_lang = get_attr(&adapt_attrs, "lang").map(str::to_string);
            let adapt_codecs = get_attr(&adapt_attrs, "codecs").map(str::to_string);
            let adapt_width = get_attr(&adapt_attrs, "width").and_then(|v| v.parse().ok());
            let adapt_height = get_attr(&adapt_attrs, "height").and_then(|v| v.parse().ok());

            let adapt_base = extract_direct_base_url(adapt)
                .map(|u| resolve_url(&period_base, &u))
                .unwrap_or_else(|| period_base.clone());

            let adapt_template = extract_segment_template(adapt);

            for rep in extract_blocks(adapt, "Representation") {
                let rep_attrs = extract_tag_attrs(rep, "Representation").unwrap_or_default();
                let id = get_attr(&rep_attrs, "id")
                    .unwrap_or("0")
                    .to_string();
                let bandwidth = get_attr(&rep_attrs, "bandwidth")
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                let width = get_attr(&rep_attrs, "width")
                    .and_then(|v| v.parse::<u32>().ok())
                    .or(adapt_width);
                let height = get_attr(&rep_attrs, "height")
                    .and_then(|v| v.parse::<u32>().ok())
                    .or(adapt_height);
                let codecs = get_attr(&rep_attrs, "codecs")
                    .map(str::to_string)
                    .or_else(|| adapt_codecs.clone());
                let mime_type = get_attr(&rep_attrs, "mimeType")
                    .map(str::to_string)
                    .or_else(|| adapt_mime.clone());

                let rep_base = extract_direct_base_url(rep)
                    .map(|u| resolve_url(&adapt_base, &u))
                    .unwrap_or_else(|| adapt_base.clone());

                let segment_template = extract_segment_template(rep).or_else(|| adapt_template.clone());

                let kind = classify_track(
                    adapt_content_type.as_deref(),
                    mime_type.as_deref(),
                    width,
                    height,
                );

                let track = DashTrack {
                    id,
                    mime_type,
                    codecs,
                    bandwidth,
                    width,
                    height,
                    language: adapt_lang.clone(),
                    base_url: Some(rep_base),
                    segment_template,
                };

                match kind {
                    TrackKind::Video => video_tracks.push(track),
                    TrackKind::Audio => audio_tracks.push(track),
                    TrackKind::Subtitle => subtitle_tracks.push(track),
                }
            }
        }
    }

    video_tracks.sort_by(|a, b| b.bandwidth.cmp(&a.bandwidth));
    audio_tracks.sort_by(|a, b| b.bandwidth.cmp(&a.bandwidth));

    Ok(DashManifest {
        is_live,
        duration_secs,
        base_url: Some(mpd_base),
        video_tracks,
        audio_tracks,
        subtitle_tracks,
    })
}

fn classify_track(
    content_type: Option<&str>,
    mime_type: Option<&str>,
    width: Option<u32>,
    height: Option<u32>,
) -> TrackKind {
    if let Some(ct) = content_type {
        let lower = ct.to_ascii_lowercase();
        if lower.contains("audio") {
            return TrackKind::Audio;
        }
        if lower.contains("text") || lower.contains("subtitle") {
            return TrackKind::Subtitle;
        }
        if lower.contains("video") {
            return TrackKind::Video;
        }
    }
    if let Some(mime) = mime_type {
        let lower = mime.to_ascii_lowercase();
        if lower.starts_with("audio/") {
            return TrackKind::Audio;
        }
        if lower.starts_with("text/") || lower.contains("ttml") || lower.contains("vtt") {
            return TrackKind::Subtitle;
        }
        if lower.starts_with("video/") {
            return TrackKind::Video;
        }
    }
    if width.is_some() || height.is_some() {
        TrackKind::Video
    } else {
        TrackKind::Audio
    }
}

fn extract_segment_template(block: &str) -> Option<DashSegmentTemplate> {
    let attrs = extract_tag_attrs(block, "SegmentTemplate")?;
    let initialization = get_attr(&attrs, "initialization").map(str::to_string);
    let media = get_attr(&attrs, "media").map(str::to_string);
    let timescale = get_attr(&attrs, "timescale")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);
    let duration = get_attr(&attrs, "duration").and_then(|v| v.parse::<u64>().ok());
    let start_number = get_attr(&attrs, "startNumber")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);

    Some(DashSegmentTemplate {
        initialization,
        media,
        timescale,
        duration,
        start_number,
    })
}

fn extract_direct_base_url(block: &str) -> Option<String> {
    let start_tag = "<BaseURL";
    let start = block.find(start_tag)?;
    let after_open = block[start..].find('>')? + start + 1;
    let end = block[after_open..].find("</BaseURL>")? + after_open;
    let val = block[after_open..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

fn extract_blocks<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let mut blocks = Vec::new();
    let open_pat = format!("<{}", tag);
    let close_pat = format!("</{}>", tag);
    let mut cursor = 0;

    while let Some(rel_start) = xml[cursor..].find(&open_pat) {
        let start = cursor + rel_start;
        // Ensure exact tag boundary (next char is whitespace, '>', or '/')
        let next_ch = xml[start + open_pat.len()..].chars().next();
        if !matches!(next_ch, Some(' ' | '\t' | '\r' | '\n' | '>' | '/')) {
            cursor = start + open_pat.len();
            continue;
        }

        // Check for self-closing tag before a closing tag
        if let Some(gt_rel) = xml[start..].find('>') {
            let gt_idx = start + gt_rel;
            if xml[..gt_idx].ends_with('/') {
                blocks.push(&xml[start..=gt_idx]);
                cursor = gt_idx + 1;
                continue;
            }
        }

        if let Some(rel_end) = xml[start..].find(&close_pat) {
            let end = start + rel_end + close_pat.len();
            blocks.push(&xml[start..end]);
            cursor = end;
        } else {
            break;
        }
    }

    blocks
}

fn extract_tag_attrs(xml: &str, tag: &str) -> Option<Vec<(String, String)>> {
    let open_pat = format!("<{}", tag);
    let mut cursor = 0;
    while let Some(rel_start) = xml[cursor..].find(&open_pat) {
        let start = cursor + rel_start;
        let after_name = start + open_pat.len();
        let next_ch = xml[after_name..].chars().next();
        if !matches!(next_ch, Some(' ' | '\t' | '\r' | '\n' | '>' | '/')) {
            cursor = after_name;
            continue;
        }
        let gt_rel = xml[after_name..].find('>')?;
        let raw_attrs = xml[after_name..after_name + gt_rel].trim_end_matches('/');
        return Some(parse_attrs(raw_attrs));
    }
    None
}

fn parse_attrs(raw: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let bytes = raw.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || key_start == i {
            break;
        }
        let key = raw[key_start..i].to_string();

        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'=') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let quote = bytes[i];
        if quote == b'"' || quote == b'\'' {
            i += 1;
            let val_start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            let val = raw[val_start..i].to_string();
            attrs.push((key, val));
            if i < bytes.len() {
                i += 1;
            }
        } else {
            let val_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            attrs.push((key, raw[val_start..i].to_string()));
        }
    }

    attrs
}

fn get_attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Parse ISO-8601 duration strings such as `PT1H2M3.5S`, `PT120S`, `PT5M30S`.
pub fn parse_iso_duration(input: &str) -> Option<f64> {
    let s = input.trim();
    let body = s.strip_prefix("PT").or_else(|| s.strip_prefix('P'))?;
    let body = body.strip_prefix('T').unwrap_or(body);

    let mut total_secs = 0.0;
    let mut num_buf = String::new();
    let mut parsed_any = false;

    for ch in body.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num_buf.push(ch);
        } else {
            let val: f64 = num_buf.parse().ok()?;
            num_buf.clear();
            parsed_any = true;
            match ch {
                'H' => total_secs += val * 3600.0,
                'M' => total_secs += val * 60.0,
                'S' => total_secs += val,
                'D' => total_secs += val * 86400.0,
                _ => return None,
            }
        }
    }

    if parsed_any {
        Some(total_secs)
    } else {
        None
    }
}

pub fn resolve_url(base: &str, relative: &str) -> String {
    if relative.starts_with("http://") || relative.starts_with("https://") {
        return relative.to_string();
    }
    if let Ok(base_url) = Url::parse(base) {
        if let Ok(joined) = base_url.join(relative) {
            return joined.to_string();
        }
    }
    relative.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATIC_MPD: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="static" mediaPresentationDuration="PT2M30.5S">
  <BaseURL>https://cdn.example.com/vod/movie/</BaseURL>
  <Period id="1">
    <AdaptationSet contentType="video" mimeType="video/mp4">
      <SegmentTemplate initialization="init-$RepresentationID$.mp4" media="chunk-$RepresentationID$-$Number%04d$.m4s" timescale="1000" duration="4000" startNumber="1" />
      <Representation id="v720" bandwidth="2500000" width="1280" height="720" codecs="avc1.4d401f" />
      <Representation id="v1080" bandwidth="5000000" width="1920" height="1080" codecs="avc1.640028" />
    </AdaptationSet>
    <AdaptationSet contentType="audio" mimeType="audio/mp4" lang="en">
      <SegmentTemplate initialization="init-audio.mp4" media="chunk-audio-$Number$.m4s" timescale="48000" duration="96000" startNumber="1" />
      <Representation id="a128" bandwidth="128000" codecs="mp4a.40.2" />
    </AdaptationSet>
    <AdaptationSet contentType="text" mimeType="text/vtt" lang="en">
      <Representation id="sub-en" bandwidth="1000">
        <BaseURL>subs/en.vtt</BaseURL>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;

    const LIVE_MPD: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<MPD type="dynamic" availabilityStartTime="2026-01-01T00:00:00Z">
  <Period>
    <AdaptationSet mimeType="video/mp4">
      <Representation id="live_hd" bandwidth="3200000" width="1280" height="720" codecs="avc1.4d401f">
        <SegmentTemplate initialization="live-init.mp4" media="live-$Number$.m4s" timescale="90000" duration="180000" startNumber="420" />
      </Representation>
    </AdaptationSet>
    <AdaptationSet mimeType="audio/mp4" lang="es">
      <Representation id="live_aud" bandwidth="96000" codecs="mp4a.40.2">
        <SegmentTemplate initialization="aud-init.mp4" media="aud-$Number$.m4s" timescale="48000" duration="96000" startNumber="420" />
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;

    #[test]
    fn parses_static_vod_mpd() {
        let manifest = parse_mpd(STATIC_MPD, "https://cdn.example.com/vod/movie/manifest.mpd")
            .expect("should parse static MPD");

        assert!(!manifest.is_live);
        assert_eq!(manifest.duration_secs, Some(150.5));
        assert_eq!(manifest.video_tracks.len(), 2);
        assert_eq!(manifest.audio_tracks.len(), 1);
        assert_eq!(manifest.subtitle_tracks.len(), 1);

        // Sorted descending by bandwidth
        let top_video = &manifest.video_tracks[0];
        assert_eq!(top_video.id, "v1080");
        assert_eq!(top_video.bandwidth, 5_000_000);
        assert_eq!(top_video.width, Some(1920));
        assert_eq!(top_video.height, Some(1080));
        assert_eq!(top_video.codecs.as_deref(), Some("avc1.640028"));

        let tmpl = top_video.segment_template.as_ref().unwrap();
        assert_eq!(tmpl.timescale, 1000);
        assert_eq!(tmpl.duration, Some(4000));
        assert_eq!(tmpl.start_number, 1);
        assert_eq!(
            DashSegmentTemplate::resolve_pattern(
                tmpl.media.as_deref().unwrap(),
                &top_video.id,
                Some(7),
                top_video.bandwidth,
                None
            ),
            "chunk-v1080-0007.m4s"
        );

        let sub = &manifest.subtitle_tracks[0];
        assert_eq!(sub.language.as_deref(), Some("en"));
        assert_eq!(
            sub.base_url.as_deref(),
            Some("https://cdn.example.com/vod/movie/subs/en.vtt")
        );
    }

    #[test]
    fn parses_dynamic_live_mpd() {
        let manifest = parse_mpd(LIVE_MPD, "https://live.example.com/stream/manifest.mpd")
            .expect("should parse dynamic MPD");

        assert!(manifest.is_live);
        assert_eq!(manifest.duration_secs, None);
        assert_eq!(manifest.video_tracks.len(), 1);
        assert_eq!(manifest.audio_tracks.len(), 1);

        let vid = &manifest.video_tracks[0];
        assert_eq!(vid.id, "live_hd");
        let tmpl = vid.segment_template.as_ref().unwrap();
        assert_eq!(tmpl.start_number, 420);
        assert_eq!(tmpl.timescale, 90000);
        assert_eq!(tmpl.duration, Some(180000));
    }

    #[test]
    fn parses_iso_durations_and_rejects_invalid_mpd() {
        assert_eq!(parse_iso_duration("PT1H2M3.5S"), Some(3723.5));
        assert_eq!(parse_iso_duration("PT45S"), Some(45.0));
        assert!(parse_mpd("<html>not mpd</html>", "https://example.com").is_err());
    }
}
