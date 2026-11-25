use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlocklistParseStats {
    pub rule_count: usize,
    pub invalid_lines: usize,
}

/// IPv4 CIDR and range blocklist (`192.168.0.0/16` or `a.b.c.d-e.f.g.h`).
#[derive(Debug, Clone, Default)]
pub struct IpBlocklist {
    ranges: Vec<(u32, u32)>,
}

impl IpBlocklist {
    /// Parse blocklist text, returning an error if any non-comment line is invalid.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut ranges = Vec::new();
        for (idx, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            let rule = parse_ipv4_rule(line).ok_or_else(|| {
                format!("Invalid IP blocklist rule at line {}: '{}'", idx + 1, line)
            })?;
            ranges.push(rule);
        }
        Ok(Self::from_ranges(ranges))
    }

    /// Parse blocklist text leniently, returning valid rules and count of invalid lines.
    pub fn parse_with_stats(text: &str) -> (Self, BlocklistParseStats) {
        let mut ranges = Vec::new();
        let mut invalid_lines = 0usize;
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            match parse_ipv4_rule(line) {
                Some(rule) => ranges.push(rule),
                None => invalid_lines += 1,
            }
        }
        let list = Self::from_ranges(ranges);
        let stats = BlocklistParseStats {
            rule_count: list.len(),
            invalid_lines,
        };
        (list, stats)
    }

    fn from_ranges(mut ranges: Vec<(u32, u32)>) -> Self {
        ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
        for (start, end) in ranges {
            if let Some(last) = merged.last_mut() {
                if start <= last.1.saturating_add(1) {
                    last.1 = last.1.max(end);
                    continue;
                }
            }
            merged.push((start, end));
        }
        Self { ranges: merged }
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Check whether an IP address falls within any blocked IPv4 CIDR or range.
    pub fn is_blocked(&self, ip: IpAddr) -> bool {
        let v4 = match ip {
            IpAddr::V4(v4) => v4,
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(mapped) => mapped,
                None => return false,
            },
        };
        let target = u32::from(v4);
        let idx = self.ranges.partition_point(|&(start, _)| start <= target);
        if idx == 0 {
            return false;
        }
        let (start, end) = self.ranges[idx - 1];
        target >= start && target <= end
    }
}

fn parse_ipv4_rule(line: &str) -> Option<(u32, u32)> {
    // Strip optional P2P label prefix (e.g. "BadPeers:1.2.3.0-1.2.3.255")
    let expr = line.rsplit_once(':').map(|(_, r)| r.trim()).unwrap_or(line);

    if let Some((ip_str, prefix_str)) = expr.split_once('/') {
        let ip: Ipv4Addr = ip_str.trim().parse().ok()?;
        let prefix: u32 = prefix_str.trim().parse().ok()?;
        if prefix > 32 {
            return None;
        }
        let mask = if prefix == 0 {
            0u32
        } else {
            !0u32 << (32 - prefix)
        };
        let start = u32::from(ip) & mask;
        let end = start | !mask;
        return Some((start, end));
    }

    if let Some((start_str, end_str)) = expr.split_once('-') {
        let start: Ipv4Addr = start_str.trim().parse().ok()?;
        let end: Ipv4Addr = end_str.trim().parse().ok()?;
        let s = u32::from(start);
        let e = u32::from(end);
        if s > e {
            return None;
        }
        return Some((s, e));
    }

    let single: Ipv4Addr = expr.parse().ok()?;
    let v = u32::from(single);
    Some((v, v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_cidr_and_ranges() {
        let rules = "\
# Local network
192.168.0.0/16
// Explicit range
10.0.0.10-10.0.0.20
203.0.113.5
";
        let bl = IpBlocklist::parse(rules).expect("valid blocklist");

        assert!(bl.is_blocked("192.168.1.42".parse().unwrap()));
        assert!(!bl.is_blocked("192.169.0.1".parse().unwrap()));

        assert!(bl.is_blocked("10.0.0.10".parse().unwrap()));
        assert!(bl.is_blocked("10.0.0.15".parse().unwrap()));
        assert!(bl.is_blocked("10.0.0.20".parse().unwrap()));
        assert!(!bl.is_blocked("10.0.0.21".parse().unwrap()));

        assert!(bl.is_blocked("203.0.113.5".parse().unwrap()));
        assert!(bl.is_blocked("::ffff:192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn rejects_invalid_rules() {
        assert!(IpBlocklist::parse("192.168.0.0/33").is_err());
        assert!(IpBlocklist::parse("10.0.0.20-10.0.0.10").is_err());
        assert!(IpBlocklist::parse("not-an-ip").is_err());

        let (bl, stats) = IpBlocklist::parse_with_stats("10.0.0.0/8\nbad_line\n1.2.3.4");
        assert_eq!(stats.rule_count, 2);
        assert_eq!(stats.invalid_lines, 1);
        assert!(bl.is_blocked("10.1.2.3".parse().unwrap()));
    }
}
