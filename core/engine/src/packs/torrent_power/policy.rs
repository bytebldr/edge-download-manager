pub use crate::adaptive::types::TorrentPowerConfig;

/// Evaluate whether seeding should stop based on configured ratio or time limits.
/// Returns `Some(reason)` when a limit is reached, or `None` to continue seeding.
pub fn should_stop_seeding(
    policy: &TorrentPowerConfig,
    total_bytes: u64,
    uploaded_bytes: u64,
    seeding_elapsed_secs: u64,
) -> Option<&'static str> {
    if let Some(max_ratio) = policy.seed_ratio_limit {
        if max_ratio <= 0.0 {
            return Some("Seed ratio limit reached");
        }
        if total_bytes > 0 {
            let ratio = uploaded_bytes as f64 / total_bytes as f64;
            if ratio >= max_ratio {
                return Some("Seed ratio limit reached");
            }
        }
    }

    if let Some(max_mins) = policy.seed_time_limit_mins {
        if seeding_elapsed_secs >= max_mins.saturating_mul(60) {
            return Some("Seed time limit reached");
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_when_ratio_reached() {
        let cfg = TorrentPowerConfig {
            seed_ratio_limit: Some(1.5),
            ..Default::default()
        };
        assert_eq!(should_stop_seeding(&cfg, 1000, 1499, 10), None);
        assert_eq!(
            should_stop_seeding(&cfg, 1000, 1500, 10),
            Some("Seed ratio limit reached")
        );
    }

    #[test]
    fn stops_when_seed_time_reached() {
        let cfg = TorrentPowerConfig {
            seed_time_limit_mins: Some(2),
            ..Default::default()
        };
        assert_eq!(should_stop_seeding(&cfg, 1000, 0, 119), None);
        assert_eq!(
            should_stop_seeding(&cfg, 1000, 0, 120),
            Some("Seed time limit reached")
        );
    }

    #[test]
    fn no_limits_never_stops() {
        let cfg = TorrentPowerConfig::default();
        assert_eq!(should_stop_seeding(&cfg, 1000, 5000, 99999), None);
    }
}
