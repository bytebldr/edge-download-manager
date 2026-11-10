use super::types::EndpointCandidate;
use reqwest::header::{HeaderMap, AUTHORIZATION, COOKIE};
use reqwest::Client;
use std::time::{Duration, Instant};
use tokio::time::timeout;

pub struct Scout;

impl Scout {
    /// Latency budget for speculative mirror racing
    pub const RACE_TIMEOUT: Duration = Duration::from_millis(600);

    /// Strip authentication cookies and Authorization headers for third-party mirror URLs
    pub fn sanitize_headers_for_mirror(is_mirror: bool, headers: &mut HeaderMap) {
        if is_mirror {
            headers.remove(AUTHORIZATION);
            headers.remove(COOKIE);
        }
    }

    /// Races multiple endpoint candidates in parallel with a strict 600ms timeout
    /// Returns the fastest responsive URL, or falls back to the default original URL
    pub async fn race_endpoints(
        candidates: &[EndpointCandidate],
        fallback_url: &str,
        client: &Client,
    ) -> String {
        if candidates.is_empty() {
            return fallback_url.to_string();
        }
        if candidates.len() == 1 {
            return candidates[0].url.clone();
        }

        let mut set = tokio::task::JoinSet::new();

        for candidate in candidates {
            let url = candidate.url.clone();
            let c = client.clone();
            set.spawn(async move {
                let start = Instant::now();
                // Probe bytes 0-0 with lightweight range request
                let res = c
                    .get(&url)
                    .header("Range", "bytes=0-0")
                    .header("Accept-Encoding", "identity")
                    .send()
                    .await;

                match res {
                    Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 206 => {
                        let ttfb = start.elapsed();
                        Some((url, ttfb))
                    }
                    _ => None,
                }
            });
        }

        let race_fut = async {
            let mut fastest: Option<(String, Duration)> = None;
            while let Some(res) = set.join_next().await {
                if let Ok(Some((url, ttfb))) = res {
                    match &fastest {
                        None => {
                            fastest = Some((url, ttfb));
                            // Short-circuit if fast enough (<100ms)
                            if ttfb < Duration::from_millis(100) {
                                break;
                            }
                        }
                        Some((_, best_ttfb)) if ttfb < *best_ttfb => {
                            fastest = Some((url, ttfb));
                        }
                        _ => {}
                    }
                }
            }
            fastest.map(|(u, _)| u)
        };

        match timeout(Self::RACE_TIMEOUT, race_fut).await {
            Ok(Some(winner)) => winner,
            _ => fallback_url.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[tokio::test]
    async fn test_sanitize_headers_for_mirror() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret_key"));
        headers.insert(COOKIE, HeaderValue::from_static("session=abc"));

        // Origin preserves auth
        Scout::sanitize_headers_for_mirror(false, &mut headers);
        assert!(headers.contains_key(AUTHORIZATION));
        assert!(headers.contains_key(COOKIE));

        // Third-party mirror strips auth
        Scout::sanitize_headers_for_mirror(true, &mut headers);
        assert!(!headers.contains_key(AUTHORIZATION));
        assert!(!headers.contains_key(COOKIE));
    }
}
