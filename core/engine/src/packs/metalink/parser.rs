//! Pure-Rust Metalink Parser
//! Supports both RFC 5854 (`.meta4`) and Metalink v3 (`.metalink`) XML manifests.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetalinkHash {
    pub hash_type: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetalinkUrl {
    pub url: String,
    pub priority: u32,
    pub location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetalinkFile {
    pub filename: String,
    pub size: Option<u64>,
    pub hashes: Vec<MetalinkHash>,
    pub urls: Vec<MetalinkUrl>,
}

/// Parse an RFC 5854 (`.meta4`) or Metalink v3 (`.metalink`) XML document.
/// URLs in each `MetalinkFile` are sorted by `priority` descending.
pub fn parse_metalink(xml: &str) -> Result<Vec<MetalinkFile>, String> {
    let cleaned = strip_comments(xml);
    if !cleaned.contains("<metalink") {
        return Err("Invalid Metalink document: missing <metalink> root element".into());
    }

    let mut files = Vec::new();
    let mut cursor = 0;

    while let Some((attrs, inner, next_pos)) = find_next_element(&cleaned, "file", cursor) {
        cursor = next_pos;

        let filename = extract_attr(&attrs, "name")
            .map(|s| decode_xml_entities(&s).trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Metalink <file> element missing non-empty 'name' attribute".to_string())?;

        let size = extract_element_text(&inner, "size")
            .and_then(|s| s.trim().parse::<u64>().ok());

        let hashes = parse_hashes(&inner);
        let mut urls = parse_urls(&inner);
        urls.sort_by(|a, b| b.priority.cmp(&a.priority));

        files.push(MetalinkFile {
            filename,
            size,
            hashes,
            urls,
        });
    }

    if files.is_empty() {
        return Err("Metalink document contains no <file> entries".into());
    }

    Ok(files)
}

fn parse_hashes(file_inner: &str) -> Vec<MetalinkHash> {
    let mut hashes = Vec::new();
    let mut cursor = 0;

    while let Some((attrs, inner, next_pos)) = find_next_element(file_inner, "hash", cursor) {
        cursor = next_pos;
        let Some(hash_type) = extract_attr(&attrs, "type")
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let value = decode_xml_entities(&inner).trim().to_string();
        if !value.is_empty() {
            hashes.push(MetalinkHash { hash_type, value });
        }
    }

    hashes
}

fn parse_urls(file_inner: &str) -> Vec<MetalinkUrl> {
    let mut urls = Vec::new();
    let mut cursor = 0;

    while let Some((attrs, inner, next_pos)) = find_next_element(file_inner, "url", cursor) {
        cursor = next_pos;
        let url = decode_xml_entities(&inner).trim().to_string();
        if url.is_empty() {
            continue;
        }

        // RFC 5854 uses `priority`; Metalink v3 uses `preference` (fallback to `priority`).
        let priority = extract_attr(&attrs, "priority")
            .or_else(|| extract_attr(&attrs, "preference"))
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);

        let location = extract_attr(&attrs, "location")
            .map(|s| decode_xml_entities(&s).trim().to_string())
            .filter(|s| !s.is_empty());

        urls.push(MetalinkUrl {
            url,
            priority,
            location,
        });
    }

    urls
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        if let Some(end) = rest[start + 4..].find("-->") {
            rest = &rest[start + 4 + end + 3..];
        } else {
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Find the next `<tag ...>inner</tag>` starting from `from`.
/// Returns `(raw_attrs, inner_xml, index_after_closing_tag)`.
fn find_next_element(xml: &str, tag: &str, from: usize) -> Option<(String, String, usize)> {
    let open_prefix = format!("<{}", tag);
    let mut search_from = from;

    while let Some(rel_start) = xml[search_from..].find(&open_prefix) {
        let start = search_from + rel_start;
        let after_name = start + open_prefix.len();
        let next_ch = xml[after_name..].chars().next()?;

        // Ensure we matched the full tag name (followed by whitespace, '>', or '/')
        if !next_ch.is_whitespace() && next_ch != '>' && next_ch != '/' {
            search_from = after_name;
            continue;
        }

        let gt_rel = xml[after_name..].find('>')?;
        let gt_pos = after_name + gt_rel;
        let raw_attrs = xml[after_name..gt_pos].trim();

        // Self-closing tag `<tag ... />`
        if raw_attrs.ends_with('/') {
            let attrs = raw_attrs.trim_end_matches('/').trim().to_string();
            return Some((attrs, String::new(), gt_pos + 1));
        }

        let close_tag = format!("</{}>", tag);
        let inner_start = gt_pos + 1;
        let close_rel = xml[inner_start..].find(&close_tag)?;
        let close_pos = inner_start + close_rel;
        let inner = xml[inner_start..close_pos].to_string();

        return Some((raw_attrs.to_string(), inner, close_pos + close_tag.len()));
    }

    None
}

fn extract_element_text(xml: &str, tag: &str) -> Option<String> {
    let (_, inner, _) = find_next_element(xml, tag, 0)?;
    Some(decode_xml_entities(&inner))
}

fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    let mut rest = attrs;
    while let Some(idx) = rest.find(name) {
        // Check word boundary before attribute name
        if idx > 0 {
            let prev = rest[..idx].chars().next_back()?;
            if !prev.is_whitespace() {
                rest = &rest[idx + name.len()..];
                continue;
            }
        }
        let after_name = rest[idx + name.len()..].trim_start();
        if !after_name.starts_with('=') {
            rest = &rest[idx + name.len()..];
            continue;
        }
        let after_eq = after_name[1..].trim_start();
        let quote = after_eq.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let val_start = 1;
        let val_end = after_eq[val_start..].find(quote)?;
        return Some(after_eq[val_start..val_start + val_end].to_string());
    }
    None
}

fn decode_xml_entities(input: &str) -> String {
    let trimmed = input.trim();
    let unwrapped = if trimmed.starts_with("<![CDATA[") && trimmed.ends_with("]]>") {
        &trimmed[9..trimmed.len() - 3]
    } else {
        input
    };

    unwrapped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc5854_meta4_sorted_by_priority_desc() {
        let meta4 = r#"<?xml version="1.0" encoding="UTF-8"?>
<metalink xmlns="urn:ietf:params:xml:ns:metalink">
  <published>2026-01-01T00:00:00Z</published>
  <file name="ubuntu-24.04.iso">
    <size>5368709120</size>
    <hash type="sha-256">abcdef0123456789</hash>
    <hash type="md5">0011223344556677</hash>
    <url location="de" priority="20">https://mirror.de.example.org/ubuntu.iso</url>
    <url location="us" priority="100">https://mirror.us.example.org/ubuntu.iso?a=1&amp;b=2</url>
    <url priority="50">https://mirror.jp.example.org/ubuntu.iso</url>
  </file>
</metalink>"#;

        let files = parse_metalink(meta4).expect("should parse valid meta4");
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.filename, "ubuntu-24.04.iso");
        assert_eq!(f.size, Some(5_368_709_120));
        assert_eq!(f.hashes.len(), 2);
        assert_eq!(f.hashes[0].hash_type, "sha-256");
        assert_eq!(f.hashes[0].value, "abcdef0123456789");
        assert_eq!(f.hashes[1].hash_type, "md5");

        assert_eq!(f.urls.len(), 3);
        assert_eq!(f.urls[0].priority, 100);
        assert_eq!(
            f.urls[0].url,
            "https://mirror.us.example.org/ubuntu.iso?a=1&b=2"
        );
        assert_eq!(f.urls[0].location.as_deref(), Some("us"));
        assert_eq!(f.urls[1].priority, 50);
        assert_eq!(f.urls[2].priority, 20);
    }

    #[test]
    fn parses_metalink_v3_with_preference_and_verification() {
        let v3 = r#"<?xml version="1.0" encoding="UTF-8"?>
<metalink version="3.0" xmlns="http://www.metalinker.org/">
  <files>
    <file name="archive.tar.gz">
      <size>1048576</size>
      <verification>
        <hash type="sha256">deadbeef</hash>
      </verification>
      <resources>
        <url type="http" location="uk" preference="40">http://uk.example.com/archive.tar.gz</url>
        <url type="https" location="us" preference="95">https://us.example.com/archive.tar.gz</url>
        <url type="ftp">ftp://ftp.example.com/archive.tar.gz</url>
      </resources>
    </file>
  </files>
</metalink>"#;

        let files = parse_metalink(v3).expect("should parse valid v3 metalink");
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.filename, "archive.tar.gz");
        assert_eq!(f.size, Some(1_048_576));
        assert_eq!(
            f.hashes,
            vec![MetalinkHash {
                hash_type: "sha256".into(),
                value: "deadbeef".into(),
            }]
        );
        assert_eq!(f.urls.len(), 3);
        assert_eq!(f.urls[0].priority, 95);
        assert_eq!(f.urls[1].priority, 40);
        assert_eq!(f.urls[2].priority, 0);
    }

    #[test]
    fn rejects_invalid_or_empty_metalink() {
        assert!(parse_metalink("").is_err());
        assert!(parse_metalink("<metalink></metalink>").is_err());
        assert!(parse_metalink("<metalink><file><size>10</size></file></metalink>").is_err());
    }
}
