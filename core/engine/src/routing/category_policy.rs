use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CategoryPolicy {
    pub category: String,
    pub max_concurrent: Option<u32>,
    pub max_speed_bps: Option<u64>,
    pub save_dir: Option<String>,
}

/// Default per-category policies for standard download categories.
pub fn default_category_policies() -> Vec<CategoryPolicy> {
    ["Video", "Music", "Documents", "Programs", "Compressed", "Other"]
        .into_iter()
        .map(|category| CategoryPolicy {
            category: category.to_string(),
            max_concurrent: None,
            max_speed_bps: None,
            save_dir: None,
        })
        .collect()
}

/// Finds a policy by category name (case-insensitive, trimmed).
pub fn find_category_policy<'a>(
    category: &str,
    policies: &'a [CategoryPolicy],
) -> Option<&'a CategoryPolicy> {
    let target = category.trim();
    if target.is_empty() {
        return None;
    }
    policies
        .iter()
        .find(|p| p.category.trim().eq_ignore_ascii_case(target))
}

/// Merges persisted custom policies onto defaults, preserving custom additions.
pub fn merge_category_policies(custom: Vec<CategoryPolicy>) -> Vec<CategoryPolicy> {
    let mut result = default_category_policies();
    for policy in custom {
        let name = policy.category.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(existing) = result
            .iter_mut()
            .find(|p| p.category.eq_ignore_ascii_case(name))
        {
            *existing = policy;
        } else {
            result.push(policy);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_standard_categories() {
        let defaults = default_category_policies();
        assert_eq!(defaults.len(), 6);
        assert!(find_category_policy("video", &defaults).is_some());
        assert!(find_category_policy("  Music ", &defaults).is_some());
        assert!(find_category_policy("", &defaults).is_none());
    }

    #[test]
    fn merge_overrides_defaults_and_keeps_custom() {
        let merged = merge_category_policies(vec![
            CategoryPolicy {
                category: "Video".into(),
                max_concurrent: Some(2),
                max_speed_bps: Some(5_000_000),
                save_dir: Some("/media/videos".into()),
            },
            CategoryPolicy {
                category: "Models".into(),
                max_concurrent: Some(1),
                max_speed_bps: None,
                save_dir: None,
            },
        ]);

        let video = find_category_policy("video", &merged).unwrap();
        assert_eq!(video.max_concurrent, Some(2));
        assert_eq!(video.max_speed_bps, Some(5_000_000));
        assert_eq!(video.save_dir.as_deref(), Some("/media/videos"));

        let models = find_category_policy("models", &merged).unwrap();
        assert_eq!(models.max_concurrent, Some(1));
        assert_eq!(merged.len(), 7);
    }
}
