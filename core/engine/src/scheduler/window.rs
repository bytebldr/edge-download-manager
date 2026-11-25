use chrono::{DateTime, Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};

/// Time-of-day and day-of-week window controlling when a queue may run.
///
/// Days of week use `0 = Sunday .. 6 = Saturday`. An empty `days_of_week`
/// vector matches every day. When `enabled` is `false`, `is_within_window`
/// returns `true` (no schedule restriction).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleWindow {
    pub enabled: bool,
    pub start_hhmm: String,
    pub end_hhmm: String,
    pub days_of_week: Vec<u8>,
}

impl Default for ScheduleWindow {
    fn default() -> Self {
        Self {
            enabled: false,
            start_hhmm: "00:00".to_string(),
            end_hhmm: "23:59".to_string(),
            days_of_week: vec![0, 1, 2, 3, 4, 5, 6],
        }
    }
}

impl ScheduleWindow {
    /// Returns `true` if scheduling is disabled or `now_local` falls within
    /// the configured `[start_hhmm, end_hhmm)` window and `days_of_week`.
    ///
    /// Handles overnight windows (e.g., `23:00` -> `06:00`), checking yesterday's
    /// weekday when `now_local` is in the post-midnight portion.
    pub fn is_within_window(&self, now_local: DateTime<Local>) -> bool {
        if !self.enabled {
            return true;
        }

        let (Some(start_mins), Some(end_mins)) = (
            parse_hhmm(&self.start_hhmm),
            parse_hhmm(&self.end_hhmm),
        ) else {
            return false;
        };

        let now_mins = (now_local.hour() * 60 + now_local.minute()) as u16;
        let today = now_local.weekday().num_days_from_sunday() as u8;

        if start_mins == end_mins {
            return self.day_allowed(today);
        }

        if start_mins < end_mins {
            self.day_allowed(today) && now_mins >= start_mins && now_mins < end_mins
        } else {
            // Overnight window: [start_mins, 24:00) on `today`, or [00:00, end_mins) from yesterday.
            if now_mins >= start_mins {
                self.day_allowed(today)
            } else if now_mins < end_mins {
                let yesterday = (today + 6) % 7;
                self.day_allowed(yesterday)
            } else {
                false
            }
        }
    }

    fn day_allowed(&self, day: u8) -> bool {
        self.days_of_week.is_empty() || self.days_of_week.contains(&day)
    }
}

fn parse_hhmm(value: &str) -> Option<u16> {
    let (h_str, m_str) = value.trim().split_once(':')?;
    let h: u16 = h_str.parse().ok()?;
    let m: u16 = m_str.parse().ok()?;
    if h >= 24 || m >= 60 {
        return None;
    }
    Some(h * 60 + m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn local_dt(year: i32, month: u32, day: u32, hour: u32, min: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(year, month, day, hour, min, 0)
            .single()
            .expect("valid local datetime")
    }

    #[test]
    fn disabled_window_always_allows() {
        let win = ScheduleWindow {
            enabled: false,
            start_hhmm: "23:00".into(),
            end_hhmm: "06:00".into(),
            days_of_week: vec![1],
        };
        assert!(win.is_within_window(local_dt(2026, 11, 25, 12, 0)));
    }

    #[test]
    fn same_day_window_boundaries() {
        let win = ScheduleWindow {
            enabled: true,
            start_hhmm: "09:00".into(),
            end_hhmm: "17:00".into(),
            days_of_week: vec![],
        };
        assert!(!win.is_within_window(local_dt(2026, 11, 25, 8, 59)));
        assert!(win.is_within_window(local_dt(2026, 11, 25, 9, 0)));
        assert!(win.is_within_window(local_dt(2026, 11, 25, 16, 59)));
        assert!(!win.is_within_window(local_dt(2026, 11, 25, 17, 0)));
    }

    #[test]
    fn overnight_window_handles_midnight_cross() {
        // 2026-11-27 is Friday (5), 2026-11-28 is Saturday (6).
        let win = ScheduleWindow {
            enabled: true,
            start_hhmm: "23:00".into(),
            end_hhmm: "06:00".into(),
            days_of_week: vec![5],
        };
        assert!(!win.is_within_window(local_dt(2026, 11, 27, 22, 59)));
        assert!(win.is_within_window(local_dt(2026, 11, 27, 23, 0)));
        assert!(win.is_within_window(local_dt(2026, 11, 27, 23, 45)));
        // Post-midnight on Saturday belongs to Friday's overnight window.
        assert!(win.is_within_window(local_dt(2026, 11, 28, 2, 30)));
        assert!(!win.is_within_window(local_dt(2026, 11, 28, 6, 0)));
        // Saturday night 23:30 is not allowed because only Friday (5) is in days_of_week.
        assert!(!win.is_within_window(local_dt(2026, 11, 28, 23, 30)));
    }

    #[test]
    fn invalid_hhmm_returns_false() {
        let win = ScheduleWindow {
            enabled: true,
            start_hhmm: "25:00".into(),
            end_hhmm: "06:00".into(),
            days_of_week: vec![],
        };
        assert!(!win.is_within_window(local_dt(2026, 11, 25, 12, 0)));
    }
}
