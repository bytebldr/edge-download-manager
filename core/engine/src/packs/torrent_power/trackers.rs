use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

use crate::packs::bittorrent::TIER1_TRACKERS;

const DAILY_TRACKERS_URL: &str =
    "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_best.txt";
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

struct CachedTrackers {
    fetched_at: Instant,
    trackers: Vec<String>,
}

static TRACKER_CACHE: OnceLock<RwLock<Option<CachedTrackers>>> = OnceLock::new();

fn cache() -> &'static RwLock<Option<CachedTrackers>> {
    TRACKER_CACHE.get_or_init(|| RwLock::new(None))
}

fn tier1_fallback() -> Vec<String> {
    TIER1_TRACKERS.iter().map(|s| (*s).to_string()).collect()
}

/// Parse newline-separated tracker URLs, filtering to valid tracker schemes.
pub fn parse_tracker_list(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let valid_scheme = trimmed.starts_with("udp://")
            || trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("wss://");
        if valid_scheme && seen.insert(trimmed.to_ascii_lowercase()) {
            out.push(trimmed.to_string());
        }
    }
    out
}

/// Merge and deduplicate tracker lists while preserving original order.
pub fn merge_and_dedup_trackers(existing: &[String], extra: &[String]) -> Vec<String> {
    let mut merged = Vec::with_capacity(existing.len() + extra.len());
    let mut seen = HashSet::new();
    for tr in existing.iter().chain(extra.iter()) {
        let trimmed = tr.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_ascii_lowercase()) {
            merged.push(trimmed.to_string());
        }
    }
    merged
}

/// Fetch and cache the daily public tracker list, falling back to `TIER1_TRACKERS`.
pub async fn fetch_public_trackers() -> Vec<String> {
    if let Ok(guard) = cache().read() {
        if let Some(ref entry) = *guard {
            if entry.fetched_at.elapsed() < CACHE_TTL && !entry.trackers.is_empty() {
                return entry.trackers.clone();
            }
        }
    }

    let fetched = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
    {
        Ok(client) => match client.get(DAILY_TRACKERS_URL).send().await {
            Ok(resp) if resp.status().is_success() => match resp.text().await {
                Ok(body) => parse_tracker_list(&body),
                Err(_) => Vec::new(),
            },
            _ => Vec::new(),
        },
        Err(_) => Vec::new(),
    };

    let fallback = tier1_fallback();
    let result = if fetched.is_empty() {
        fallback
    } else {
        merge_and_dedup_trackers(&fetched, &fallback)
    };

    if let Ok(mut guard) = cache().write() {
        *guard = Some(CachedTrackers {
            fetched_at: Instant::now(),
            trackers: result.clone(),
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_dedups_trackers() {
        let raw = "\
# comment
udp://tracker.opentrackr.org:1337/announce

https://tracker.example.com/announce
not-a-tracker
UDP://tracker.opentrackr.org:1337/announce
";
        let parsed = parse_tracker_list(raw);
        assert_eq!(
            parsed,
            vec![
                "udp://tracker.opentrackr.org:1337/announce",
                "https://tracker.example.com/announce"
            ]
        );
    }

    #[test]
    fn merges_and_dedups_preserving_order() {
        let a = vec![
            "udp://a.org:1337/announce".to_string(),
            "udp://b.org:1337/announce".to_string(),
        ];
        let b = vec![
            "udp://B.org:1337/announce".to_string(),
            "https://c.org/announce".to_string(),
        ];
        assert_eq!(
            merge_and_dedup_trackers(&a, &b),
            vec![
                "udp://a.org:1337/announce",
                "udp://b.org:1337/announce",
                "https://c.org/announce"
            ]
        );
    }
}
