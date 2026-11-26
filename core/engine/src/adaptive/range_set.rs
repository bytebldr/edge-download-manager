//! Disjoint byte range set for tracking download completion and verifying zero gaps.
//! Every range is inclusive: `[start, end]`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeSet {
    ranges: Vec<(u64, u64)>, // always sorted and non-overlapping/non-adjacent
}

impl RangeSet {
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    pub fn from_ranges(ranges: Vec<(u64, u64)>) -> Self {
        let mut set = Self::new();
        for (s, e) in ranges {
            set.add(s, e);
        }
        set
    }

    /// Adds an inclusive range `[start, end]` to the set in-place, merging any overlapping or adjacent ranges.
    pub fn add(&mut self, start: u64, end: u64) {
        if start > end {
            return;
        }

        // First range whose end is >= start - 1 (i.e. can overlap or touch [start, end]).
        let idx = self
            .ranges
            .partition_point(|&(_, r_end)| r_end.saturating_add(1) < start);

        if idx == self.ranges.len() {
            self.ranges.push((start, end));
            return;
        }

        // Count how many consecutive ranges starting at `idx` overlap or are adjacent to [start, end].
        let merge_limit = end.saturating_add(1);
        let merge_count = self.ranges[idx..].partition_point(|&(r_start, _)| r_start <= merge_limit);

        if merge_count == 0 {
            self.ranges.insert(idx, (start, end));
        } else if merge_count == 1 {
            self.ranges[idx].0 = self.ranges[idx].0.min(start);
            self.ranges[idx].1 = self.ranges[idx].1.max(end);
        } else {
            let merged_start = start.min(self.ranges[idx].0);
            let merged_end = end.max(self.ranges[idx + merge_count - 1].1);
            self.ranges[idx] = (merged_start, merged_end);
            self.ranges.drain(idx + 1..idx + merge_count);
        }
    }

    /// Total number of unique bytes covered across all disjoint ranges.
    pub fn total_covered_bytes(&self) -> u64 {
        self.ranges.iter().fold(0u64, |acc, &(s, e)| {
            acc.saturating_add(e.saturating_sub(s).saturating_add(1))
        })
    }

    /// Returns a slice of the disjoint ranges.
    pub fn ranges(&self) -> &[(u64, u64)] {
        &self.ranges
    }

    /// Number of disjoint ranges currently tracked.
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    /// Returns `true` if no byte ranges are recorded.
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Checks whether the entire byte range `[0, total_size - 1]` has been completely covered with 0 gaps.
    pub fn is_complete(&self, total_size: u64) -> bool {
        if total_size == 0 {
            return true;
        }
        self.ranges.len() == 1 && self.ranges[0] == (0, total_size.saturating_sub(1))
    }

    /// Computes all missing / unwritten byte ranges within `[0, total_size - 1]`.
    pub fn missing_ranges(&self, total_size: u64) -> Vec<(u64, u64)> {
        if total_size == 0 {
            return vec![];
        }

        let last_byte = total_size.saturating_sub(1);
        let mut gaps = Vec::new();
        let mut expected_start = 0u64;

        for &(s, e) in &self.ranges {
            if expected_start >= total_size {
                break;
            }
            if s > expected_start {
                let gap_end = s.min(total_size).saturating_sub(1);
                gaps.push((expected_start, gap_end));
            }
            if e >= last_byte {
                expected_start = total_size;
                break;
            }
            expected_start = expected_start.max(e.saturating_add(1));
        }

        if expected_start < total_size {
            gaps.push((expected_start, last_byte));
        }

        gaps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_and_single() {
        let mut set = RangeSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert_eq!(set.total_covered_bytes(), 0);
        assert!(!set.is_complete(100));

        set.add(0, 99);
        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());
        assert_eq!(set.total_covered_bytes(), 100);
        assert!(set.is_complete(100));
        assert_eq!(set.missing_ranges(100), vec![]);
    }

    #[test]
    fn test_merge_adjacent_and_overlapping() {
        let mut set = RangeSet::new();
        set.add(0, 49);
        set.add(50, 99); // adjacent
        assert_eq!(set.ranges(), &[(0, 99)]);
        assert!(set.is_complete(100));

        set.add(20, 70); // overlapping subset
        assert_eq!(set.ranges(), &[(0, 99)]);

        set.add(90, 150); // extending tail
        assert_eq!(set.ranges(), &[(0, 150)]);
        assert_eq!(set.total_covered_bytes(), 151);
    }

    #[test]
    fn test_in_place_multi_merge_and_insert() {
        let mut set = RangeSet::new();
        set.add(30, 39);
        set.add(10, 19);
        set.add(50, 59);
        assert_eq!(set.ranges(), &[(10, 19), (30, 39), (50, 59)]);
        assert_eq!(set.len(), 3);

        // Bridge all three ranges at once
        set.add(19, 50);
        assert_eq!(set.ranges(), &[(10, 59)]);
        assert_eq!(set.len(), 1);
        assert_eq!(set.total_covered_bytes(), 50);

        // Disjoint prefix insert
        set.add(0, 5);
        assert_eq!(set.ranges(), &[(0, 5), (10, 59)]);
    }

    #[test]
    fn test_gaps_identification() {
        let mut set = RangeSet::new();
        set.add(10, 19);
        set.add(30, 39);

        let gaps = set.missing_ranges(50);
        assert_eq!(gaps, vec![(0, 9), (20, 29), (40, 49)]);
        assert!(!set.is_complete(50));
    }

    #[test]
    fn test_u64_max_saturation() {
        let mut set = RangeSet::new();
        set.add(u64::MAX - 5, u64::MAX);
        set.add(u64::MAX - 10, u64::MAX - 6);
        assert_eq!(set.ranges(), &[(u64::MAX - 10, u64::MAX)]);
        assert_eq!(set.total_covered_bytes(), 11);
        assert_eq!(set.missing_ranges(u64::MAX), vec![(0, u64::MAX - 11)]);
    }
}
