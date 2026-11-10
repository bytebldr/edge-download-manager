use crate::adaptive::AdaptiveDownloadTask;
use crate::adaptive_wrapper::traits::AdaptiveWrapperAdapter;
use crate::adaptive_wrapper::types::{
    ChunkingStrategy, PostFlightContext, PreFlightExecution, PreFlightPlan,
};
use crate::adaptive::TelemetryCallback;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct BitTorrentAdapter;

impl BitTorrentAdapter {
    pub const TIER1_TRACKERS: &'static [&'static str] = &[
        "udp://tracker.opentrackr.org:1337/announce",
        "udp://open.stealth.si:80/announce",
        "udp://tracker.torrent.eu.org:451/announce",
    ];

    /// Injects high-availability public Tier-1 trackers into a magnet link if not already present
    pub fn inject_trackers(magnet: &str) -> String {
        if !magnet.to_ascii_lowercase().starts_with("magnet:") {
            return magnet.to_string();
        }

        let mut enriched = magnet.to_string();
        for tracker in Self::TIER1_TRACKERS {
            let encoded = urlencoding::encode(tracker);
            if !enriched.contains(&encoded.to_string()) && !enriched.contains(*tracker) {
                enriched.push_str("&tr=");
                enriched.push_str(&encoded);
            }
        }
        enriched
    }
}

#[async_trait]
impl AdaptiveWrapperAdapter for BitTorrentAdapter {
    async fn pre_flight(
        &self,
        task: &AdaptiveDownloadTask,
        _client: &Client,
    ) -> Result<PreFlightPlan, String> {
        let mut clean_task = task.clone();
        if clean_task.url.to_ascii_lowercase().starts_with("magnet:") {
            clean_task.url = Self::inject_trackers(&clean_task.url);
        }

        let is_media = clean_task.filename.ends_with(".mp4")
            || clean_task.filename.ends_with(".mkv")
            || clean_task.filename.ends_with(".avi")
            || clean_task.filename.ends_with(".mov");

        let mut metadata = HashMap::new();
        if is_media {
            metadata.insert("sequential_bias".to_string(), "true".to_string());
        }

        Ok(PreFlightPlan {
            execution: PreFlightExecution::DirectProtocol {
                task: clean_task,
                custom_metadata: metadata,
            },
            chunk_strategy: ChunkingStrategy::DirectStreamPiped,
            expected_size: None,
            expected_hash: None,
            suggested_filename: None,
            fallback_urls: Vec::new(),
        })
    }

    async fn post_flight(
        &self,
        ctx: PostFlightContext,
        _telemetry_cb: Option<TelemetryCallback>,
    ) -> Result<PathBuf, String> {
        if !ctx.final_path.exists() {
            return Err("BitTorrent payload path does not exist".to_string());
        }
        Ok(ctx.final_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_injection() {
        let raw = "magnet:?xt=urn:btih:d6b631d8c11718919fe0cfe4552b95b8d2342ef5&dn=test";
        let enriched = BitTorrentAdapter::inject_trackers(raw);
        assert!(enriched.contains("tracker.opentrackr.org"));
        assert!(enriched.contains("open.stealth.si"));
    }
}
