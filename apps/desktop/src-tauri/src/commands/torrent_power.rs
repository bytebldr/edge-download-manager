use serde::Serialize;
use tauri::command;

use edm_core::adaptive::types::TorrentPowerConfig;
use edm_core::packs::torrent_power::{fetch_public_trackers, IpBlocklist};

#[derive(Debug, Clone, Serialize)]
pub struct BlocklistValidationResult {
    pub valid: bool,
    pub rule_count: usize,
    pub invalid_lines: usize,
    pub error: Option<String>,
}

#[command]
pub fn get_torrent_power_defaults() -> TorrentPowerConfig {
    TorrentPowerConfig::default()
}

#[command]
pub async fn refresh_torrent_trackers() -> Result<Vec<String>, String> {
    Ok(fetch_public_trackers().await)
}

#[command]
pub fn validate_ip_blocklist(rules: String) -> Result<BlocklistValidationResult, String> {
    let (_list, stats) = IpBlocklist::parse_with_stats(&rules);
    let valid = stats.invalid_lines == 0;
    let error = if valid {
        None
    } else {
        Some(format!(
            "{} invalid blocklist line(s) found",
            stats.invalid_lines
        ))
    };
    Ok(BlocklistValidationResult {
        valid,
        rule_count: stats.rule_count,
        invalid_lines: stats.invalid_lines,
        error,
    })
}
