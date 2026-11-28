use crate::sanitizer::Sanitizer;
use reqwest::header::{ACCEPT_ENCODING, RANGE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProbeError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Server returned status {0}")]
    Status(u16),
    #[error("Invalid URL")]
    InvalidUrl,
    #[error("Probe request timed out")]
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub url: String,
    pub final_url: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size_total: Option<u64>,
    pub resumable: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    #[serde(default)]
    pub status_code: u16,
    #[serde(default)]
    pub response_headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub repr_digest: Option<(String, String)>,
}

pub struct Probe;

impl Probe {
    pub const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

    pub async fn probe_url(client: &Client, url: &str) -> Result<ProbeResult, ProbeError> {
        Self::probe_url_with_headers(client, url, None, None, None).await
    }

    pub async fn probe_url_with_headers(
        client: &Client,
        url: &str,
        cookies: Option<&str>,
        referer: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<ProbeResult, ProbeError> {
        Self::probe_url_with_headers_timeout(
            client,
            url,
            cookies,
            referer,
            user_agent,
            Self::PROBE_TIMEOUT,
        )
        .await
    }

    async fn probe_url_with_headers_timeout(
        client: &Client,
        url: &str,
        cookies: Option<&str>,
        referer: Option<&str>,
        user_agent: Option<&str>,
        request_timeout: Duration,
    ) -> Result<ProbeResult, ProbeError> {
        // Try HEAD first
        let mut head_req = client.head(url).header(ACCEPT_ENCODING, "identity");
        if let Some(c) = cookies {
            head_req = head_req.header(reqwest::header::COOKIE, c);
        }
        if let Some(r) = referer {
            head_req = head_req.header(reqwest::header::REFERER, r);
        }
        if let Some(u) = user_agent {
            head_req = head_req.header(reqwest::header::USER_AGENT, u);
        }

        let head_resp = tokio::time::timeout(request_timeout, head_req.send()).await;

        let (resp, tested_range_directly) = match head_resp {
            Ok(Ok(r)) if r.status().is_success() => (r, false),
            _ => {
                // Fallback to GET with Range: bytes=0-0
                let mut get_req = client
                    .get(url)
                    .header(ACCEPT_ENCODING, "identity")
                    .header(RANGE, "bytes=0-0");
                if let Some(c) = cookies {
                    get_req = get_req.header(reqwest::header::COOKIE, c);
                }
                if let Some(r) = referer {
                    get_req = get_req.header(reqwest::header::REFERER, r);
                }
                if let Some(u) = user_agent {
                    get_req = get_req.header(reqwest::header::USER_AGENT, u);
                }

                let r = tokio::time::timeout(request_timeout, get_req.send())
                    .await
                    .map_err(|_| ProbeError::Timeout)??;
                (r, true)
            }
        };

        let status = resp.status();
        if !status.is_success() {
            return Err(ProbeError::Status(status.as_u16()));
        }
        let final_url = resp.url().to_string();
        let headers = resp.headers();

        let mut header_map = std::collections::HashMap::new();
        for (k, v) in headers {
            if let Ok(v_str) = v.to_str() {
                header_map.insert(k.as_str().to_string(), v_str.to_string());
            }
        }

        let etag = headers
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let last_modified = headers
            .get(reqwest::header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let mut filename = Self::extract_filename(headers, &final_url);

        // Check for YouTube URLs and fetch official human-readable video title
        if Self::is_youtube_url(url) || Self::is_youtube_url(&final_url) {
            if let Some(yt_title) = Self::fetch_youtube_title(client, url).await {
                filename = Some(yt_title);
            } else if let Some(yt_title) = Self::fetch_youtube_title(client, &final_url).await {
                filename = Some(yt_title);
            }
        } else if filename.as_deref().is_none_or(Self::is_generic_name) {
            // Check if query parameters specify a title or filename
            if let Some(extracted) = Self::extract_filename_from_query(&final_url) {
                filename = Some(extracted);
            } else if let Some(extracted) = Self::extract_filename_from_query(url) {
                filename = Some(extracted);
            }
        }

        // Ensure proper file extension matching content-type
        if let Some(f) = filename {
            filename = Some(Self::ensure_extension(f, content_type.as_deref()));
        }

        let mut resumable = false;
        let mut size_total = None;

        if status == reqwest::StatusCode::PARTIAL_CONTENT {
            resumable = true;
            if let Some(content_range) = headers
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
            {
                // e.g. "bytes 0-0/1048576"
                if let Some(slash_idx) = content_range.rfind('/') {
                    let total_str = &content_range[slash_idx + 1..];
                    if let Ok(total) = total_str.parse::<u64>() {
                        size_total = Some(total);
                    }
                }
            }
        } else if tested_range_directly && status == reqwest::StatusCode::OK {
            // Server returned 200 OK despite Range request: ignores byte-ranges
            resumable = false;
        } else if let Some(accept_ranges) = headers
            .get(reqwest::header::ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
        {
            if accept_ranges.to_ascii_lowercase().contains("none") {
                resumable = false;
            } else if accept_ranges.to_ascii_lowercase().contains("bytes") {
                resumable = true;
            }
        }

        if size_total.is_none() {
            if let Some(cl) = headers
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
            {
                if let Ok(length) = cl.parse::<u64>() {
                    size_total = Some(length);
                }
            }
        }

        let mut repr_digest = headers
            .get("repr-digest")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_repr_digest);

        if (!resumable || size_total.is_none()) && !tested_range_directly && status.is_success() {
            // Verify if server supports ranges and/or total size via a lightweight Range: bytes=0-0 probe
            let mut range_test = client
                .get(&final_url)
                .header(ACCEPT_ENCODING, "identity")
                .header(RANGE, "bytes=0-0");
            if let Some(c) = cookies {
                range_test = range_test.header(reqwest::header::COOKIE, c);
            }
            if let Some(r) = referer {
                range_test = range_test.header(reqwest::header::REFERER, r);
            }
            if let Some(u) = user_agent {
                range_test = range_test.header(reqwest::header::USER_AGENT, u);
            }

            if let Ok(Ok(rt_resp)) = tokio::time::timeout(request_timeout, range_test.send()).await
            {
                if repr_digest.is_none() {
                    repr_digest = rt_resp
                        .headers()
                        .get("repr-digest")
                        .and_then(|v| v.to_str().ok())
                        .and_then(parse_repr_digest);
                }
                if rt_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                    resumable = true;
                    if size_total.is_none() {
                        if let Some(content_range) = rt_resp
                            .headers()
                            .get(reqwest::header::CONTENT_RANGE)
                            .and_then(|v| v.to_str().ok())
                        {
                            if let Some(slash_idx) = content_range.rfind('/') {
                                let total_str = &content_range[slash_idx + 1..];
                                if let Ok(total) = total_str.parse::<u64>() {
                                    size_total = Some(total);
                                }
                            }
                        }
                    }
                }
                // Drop immediately without draining multi-GB 200 OK body
                drop(rt_resp);
            }
        }

        Ok(ProbeResult {
            url: url.to_string(),
            final_url,
            filename,
            content_type,
            size_total,
            resumable,
            etag,
            last_modified,
            status_code: status.as_u16(),
            response_headers: header_map,
            repr_digest,
        })
    }

    fn extract_filename(headers: &reqwest::header::HeaderMap, final_url: &str) -> Option<String> {
        if let Some(cd) = headers
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
        {
            if let Some(name) = Self::parse_content_disposition(cd) {
                return Some(name);
            }
        }

        if let Ok(parsed_url) = url::Url::parse(final_url) {
            if let Some(mut segments) = parsed_url.path_segments() {
                if let Some(last) = segments.next_back() {
                    let decoded = urlencoding::decode(last).unwrap_or_else(|_| last.into());
                    if !decoded.is_empty() {
                        return Some(decoded.to_string());
                    }
                }
            }
        }

        None
    }

    pub fn parse_content_disposition(header: &str) -> Option<String> {
        // Handle filename* first (RFC 5987) e.g. filename*=UTF-8''foo%20bar.zip
        for part in header.split(';') {
            let part = part.trim();
            if part.to_ascii_lowercase().starts_with("filename*=") {
                let val = &part["filename*=".len()..];
                if let Some(quote_stripped) =
                    val.strip_prefix('"').and_then(|v| v.strip_suffix('"'))
                {
                    return Self::decode_rfc5987(quote_stripped);
                } else {
                    return Self::decode_rfc5987(val);
                }
            }
        }

        // Handle standard filename="foo.zip" or filename=foo.zip
        for part in header.split(';') {
            let part = part.trim();
            if part.to_ascii_lowercase().starts_with("filename=") {
                let val = &part["filename=".len()..];
                let clean = val.trim_matches('"').trim_matches('\'');
                if !clean.is_empty() {
                    return Some(clean.to_string());
                }
            }
        }

        None
    }

    fn decode_rfc5987(val: &str) -> Option<String> {
        let parts: Vec<&str> = val.splitn(3, '\'').collect();
        if parts.len() == 3 {
            let encoded = parts[2];
            urlencoding::decode(encoded).ok().map(|s| s.into_owned())
        } else {
            urlencoding::decode(val).ok().map(|s| s.into_owned())
        }
    }

    pub fn is_youtube_url(url: &str) -> bool {
        let lower = url.to_ascii_lowercase();
        lower.contains("youtube.com/watch")
            || lower.contains("youtu.be/")
            || lower.contains("youtube.com/shorts/")
            || lower.contains("youtube.com/embed/")
    }

    pub async fn fetch_youtube_title(client: &Client, url: &str) -> Option<String> {
        let oembed_url = format!(
            "https://www.youtube.com/oembed?url={}&format=json",
            urlencoding::encode(url)
        );
        let resp = tokio::time::timeout(Self::PROBE_TIMEOUT, client.get(&oembed_url).send())
            .await
            .ok()?
            .ok()?;
        if resp.status().is_success() {
            if let Ok(Ok(text)) = tokio::time::timeout(Self::PROBE_TIMEOUT, resp.text()).await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(title) = json.get("title").and_then(|v| v.as_str()) {
                        let clean = Sanitizer::sanitize_filename(&format!("{}.mp4", title));
                        return Some(clean);
                    }
                }
            }
        }
        None
    }

    pub fn is_generic_name(name: &str) -> bool {
        let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
        matches!(
            stem.as_str(),
            "watch"
                | "videoplayback"
                | "download"
                | "file"
                | "index"
                | "stream"
                | "media"
                | "video"
                | "audio"
                | "get"
                | "play"
                | ""
        )
    }

    pub fn extract_filename_from_query(url_str: &str) -> Option<String> {
        if let Ok(parsed) = url::Url::parse(url_str) {
            for (k, v) in parsed.query_pairs() {
                let k_lower = k.to_ascii_lowercase();
                if matches!(
                    k_lower.as_str(),
                    "title" | "filename" | "file_name" | "file"
                ) && !v.trim().is_empty()
                {
                    return Some(Sanitizer::sanitize_filename(&v));
                }
            }
        }
        None
    }

    pub fn ensure_extension(filename: String, content_type: Option<&str>) -> String {
        if filename.contains('.') {
            return filename;
        }
        if let Some(ct) = content_type {
            let ext = match ct
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
                .as_str()
            {
                "video/mp4" => ".mp4",
                "video/webm" => ".webm",
                "video/x-matroska" => ".mkv",
                "video/quicktime" => ".mov",
                "audio/mpeg" => ".mp3",
                "audio/mp4" | "audio/m4a" => ".m4a",
                "audio/ogg" => ".ogg",
                "audio/flac" => ".flac",
                "application/zip" | "application/x-zip-compressed" => ".zip",
                "application/x-tar" => ".tar",
                "application/gzip" => ".tar.gz",
                "application/x-7z-compressed" => ".7z",
                "application/x-rar-compressed" => ".rar",
                "application/pdf" => ".pdf",
                "application/octet-stream" => ".bin",
                _ => "",
            };
            if !ext.is_empty() {
                return format!("{}{}", filename, ext);
            }
        }
        filename
    }
}

/// Parses RFC 9530 `Repr-Digest` SFV Byte Sequence (e.g. `sha-256=:base64:`) into `("sha256", lowercase_hex)`.
pub fn parse_repr_digest(header_val: &str) -> Option<(String, String)> {
    for part in header_val.split(',') {
        let trimmed = part.trim();
        let Some((algo_raw, val_raw)) = trimmed.split_once('=') else {
            continue;
        };
        let algo = algo_raw.trim().to_ascii_lowercase();
        if algo != "sha-256" && algo != "sha-512" && algo != "md5" {
            continue;
        }
        let val = val_raw.trim();
        if let Some(b64) = val.strip_prefix(':').and_then(|s| s.strip_suffix(':')) {
            if let Some(bytes) = decode_base64_std(b64) {
                let norm_algo = match algo.as_str() {
                    "sha-256" => "sha256",
                    "sha-512" => "sha512",
                    other => other,
                };
                return Some((norm_algo.to_string(), hex::encode(bytes)));
            }
        }
    }
    None
}

fn decode_base64_std(input: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if clean.is_empty() || clean.len() % 4 != 0 {
        return None;
    }
    let val = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity((clean.len() / 4) * 3);
    for chunk in clean.chunks_exact(4) {
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        out.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            let c = val(chunk[2])?;
            out.push((b << 4) | (c >> 2));
            if chunk[3] != b'=' {
                let d = val(chunk[3])?;
                out.push((c << 6) | d);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_probe_timeout_returns_timeout_error_on_hung_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");

        // Accept connections and hold them open without sending any HTTP response
        let _server = tokio::spawn(async move {
            let mut sockets = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                sockets.push(socket);
            }
        });

        let client = Client::new();
        let url = format!("http://{}/hung.bin", addr);
        let res = Probe::probe_url_with_headers_timeout(
            &client,
            &url,
            None,
            None,
            None,
            Duration::from_millis(80),
        )
        .await;

        assert!(matches!(res, Err(ProbeError::Timeout)));
    }

    #[tokio::test]
    async fn test_probe_rejects_http_error_status_on_get_fallback() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");

        let _server = tokio::spawn(async move {
            for _ in 0..2 {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let mut buf = [0u8; 1024];
                    let _ = socket.read(&mut buf).await;
                    let body = "<html><body>403 Forbidden</body></html>";
                    let resp = format!(
                        "HTTP/1.1 403 Forbidden\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                }
            }
        });

        let client = Client::new();
        let url = format!("http://{}/protected.zip", addr);
        let res = Probe::probe_url_with_headers_timeout(
            &client,
            &url,
            None,
            None,
            None,
            Duration::from_secs(2),
        )
        .await;

        assert!(matches!(res, Err(ProbeError::Status(403))));
    }
}
