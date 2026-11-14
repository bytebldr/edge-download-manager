use std::cmp::Ordering;

/// Parse a semver string (e.g. "0.0.14", "v0.0.13", "1.2.3-beta") into (major, minor, patch).
pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let clean = v.trim().trim_start_matches(['v', 'V']);
    let core = clean.split(&['-', '+'][..]).next()?.trim();
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch = parts[2].parse::<u64>().ok()?;
    Some((major, minor, patch))
}

/// Compare two semver strings numerically.
/// Returns:
///   Ordering::Less    if a < b
///   Ordering::Equal   if a == b
///   Ordering::Greater if a > b
/// If either version cannot be parsed, falls back to string comparison.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    match (parse_semver(a), parse_semver(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        _ => a.trim().cmp(b.trim()),
    }
}

/// Helper returning -1 | 0 | 1 for compatibility with JS/TS numeric comparisons.
pub fn compare_versions_num(a: &str, b: &str) -> i8 {
    match compare_versions(a, b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_ordering() {
        assert_eq!(compare_versions("0.0.13", "0.0.14"), Ordering::Less);
        assert_eq!(compare_versions("0.0.14", "0.0.13"), Ordering::Greater);
        assert_eq!(compare_versions("0.0.14", "0.0.14"), Ordering::Equal);
        assert_eq!(compare_versions("v0.0.14", "0.0.14"), Ordering::Equal);
        assert_eq!(compare_versions("0.0.9", "0.0.10"), Ordering::Less);
        assert_eq!(compare_versions("0.1.0", "0.0.14"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0", "0.99.99"), Ordering::Greater);
        assert_eq!(compare_versions("v0.0.14-rc1", "0.0.14"), Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_num() {
        assert_eq!(compare_versions_num("0.0.13", "0.0.14"), -1);
        assert_eq!(compare_versions_num("0.0.14", "0.0.14"), 0);
        assert_eq!(compare_versions_num("0.0.14", "0.0.13"), 1);
    }
}
