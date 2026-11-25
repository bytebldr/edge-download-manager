use std::collections::HashSet;
use serde::{Deserialize, Serialize};

/// A single link extracted from a batch import source (.txt, .csv, .html).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedLink {
    pub url: String,
    pub filename_hint: Option<String>,
    pub category_hint: Option<String>,
}

impl ImportedLink {
    fn new(url: String, filename_hint: Option<String>, category_hint: Option<String>) -> Self {
        Self {
            url,
            filename_hint: clean_optional(filename_hint),
            category_hint: clean_optional(category_hint),
        }
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Validates that a URL uses an allowed download scheme (`http://`, `https://`, or `magnet:`)
/// and contains no whitespace or control characters.
pub fn is_valid_link_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("http://").or_else(|| lower.strip_prefix("https://")) {
        return !rest.is_empty() && !rest.starts_with('/');
    }
    if let Some(rest) = lower.strip_prefix("magnet:") {
        return !rest.is_empty();
    }
    false
}

fn deduplicate_links(links: Vec<ImportedLink>) -> Vec<ImportedLink> {
    let mut seen = HashSet::with_capacity(links.len());
    let mut out = Vec::with_capacity(links.len());
    for link in links {
        if is_valid_link_url(&link.url) && seen.insert(link.url.clone()) {
            out.push(link);
        }
    }
    out
}

/// Parses a batch import payload in the given format (`txt`, `csv`, `html`, or `htm`).
pub fn parse_batch_import(content: &str, format: &str) -> Result<Vec<ImportedLink>, String> {
    let normalized = format.trim().trim_start_matches('.').to_ascii_lowercase();
    match normalized.as_str() {
        "txt" | "text" => Ok(parse_txt(content)),
        "csv" => Ok(parse_csv(content)),
        "html" | "htm" => Ok(parse_html(content)),
        other => Err(format!("Unsupported batch import format: '{}'", other)),
    }
}

/// Parses newline-separated URLs, ignoring blank lines and `#` comment lines.
pub fn parse_txt(content: &str) -> Vec<ImportedLink> {
    let links = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| ImportedLink::new(line.to_string(), None, None))
        .collect();
    deduplicate_links(links)
}

/// Parses CSV rows into `ImportedLink`s, supporting quoted fields and optional header rows.
pub fn parse_csv(content: &str) -> Vec<ImportedLink> {
    let mut rows = parse_csv_rows(content).into_iter().peekable();
    let (mut url_idx, mut file_idx, mut cat_idx) = (0usize, Some(1usize), Some(2usize));

    if let Some(first_row) = rows.peek() {
        if let Some((u, f, c)) = detect_csv_header(first_row) {
            url_idx = u;
            file_idx = f;
            cat_idx = c;
            rows.next();
        }
    }

    let mut links = Vec::new();
    for row in rows {
        let Some(raw_url) = row.get(url_idx).map(|s| s.trim()) else {
            continue;
        };
        if raw_url.is_empty() || raw_url.starts_with('#') {
            continue;
        }
        let filename_hint = file_idx.and_then(|i| row.get(i).cloned());
        let category_hint = cat_idx.and_then(|i| row.get(i).cloned());
        links.push(ImportedLink::new(raw_url.to_string(), filename_hint, category_hint));
    }
    deduplicate_links(links)
}

fn detect_csv_header(row: &[String]) -> Option<(usize, Option<usize>, Option<usize>)> {
    let normalized: Vec<String> = row.iter().map(|c| c.trim().to_ascii_lowercase()).collect();
    let url_idx = normalized
        .iter()
        .position(|c| matches!(c.as_str(), "url" | "link" | "href" | "uri"))?;
    let file_idx = normalized
        .iter()
        .position(|c| matches!(c.as_str(), "filename" | "file" | "name" | "filename_hint"));
    let cat_idx = normalized
        .iter()
        .position(|c| matches!(c.as_str(), "category" | "cat" | "type" | "category_hint"));
    Some((url_idx, file_idx, cat_idx))
}

fn parse_csv_rows(content: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut current_row = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current_field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                current_field.push(ch);
            }
            continue;
        }

        match ch {
            '"' if current_field.is_empty() => in_quotes = true,
            ',' => {
                current_row.push(std::mem::take(&mut current_field));
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                current_row.push(std::mem::take(&mut current_field));
                if current_row.iter().any(|f| !f.trim().is_empty()) {
                    rows.push(std::mem::take(&mut current_row));
                } else {
                    current_row.clear();
                }
            }
            '\n' => {
                current_row.push(std::mem::take(&mut current_field));
                if current_row.iter().any(|f| !f.trim().is_empty()) {
                    rows.push(std::mem::take(&mut current_row));
                } else {
                    current_row.clear();
                }
            }
            _ => current_field.push(ch),
        }
    }

    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(current_field);
        if current_row.iter().any(|f| !f.trim().is_empty()) {
            rows.push(current_row);
        }
    }
    rows
}

/// Extracts `href` and `src` HTTP/HTTPS/magnet links from HTML markup.
pub fn parse_html(content: &str) -> Vec<ImportedLink> {
    let mut links = Vec::new();
    let lower = content.to_ascii_lowercase();
    let bytes = content.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        let rest_lower = &lower[i..];
        let attr_len = if rest_lower.starts_with("href") && is_attr_boundary(bytes, i, 4) {
            4
        } else if rest_lower.starts_with("src") && is_attr_boundary(bytes, i, 3) {
            3
        } else {
            i += next_char_len(content, i);
            continue;
        };

        let mut j = i + attr_len;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'=' {
            i += attr_len;
            continue;
        }
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }

        let (raw_val, next_pos) = if bytes[j] == b'"' || bytes[j] == b'\'' {
            let quote = bytes[j];
            let start = j + 1;
            match content[start..].find(quote as char) {
                Some(rel_end) => (&content[start..start + rel_end], start + rel_end + 1),
                None => break,
            }
        } else {
            let start = j;
            let end = content[start..]
                .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '<')
                .map(|rel| start + rel)
                .unwrap_or(bytes.len());
            (&content[start..end], end)
        };

        let decoded = decode_html_attr(raw_val.trim());
        links.push(ImportedLink::new(decoded, None, None));
        i = next_pos;
    }

    deduplicate_links(links)
}

fn is_attr_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let before_ok = start == 0
        || bytes[start - 1].is_ascii_whitespace()
        || bytes[start - 1] == b'<';
    let after = start + len;
    let after_ok = after < bytes.len()
        && (bytes[after].is_ascii_whitespace() || bytes[after] == b'=');
    before_ok && after_ok
}

fn next_char_len(s: &str, byte_idx: usize) -> usize {
    s[byte_idx..].chars().next().map_or(1, char::len_utf8)
}

fn decode_html_attr(raw: &str) -> String {
    raw.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_txt_ignoring_comments_blanks_and_duplicates() {
        let input = "\n# Comment line\n  https://example.com/a.zip  \n\nftp://ignored.com/x\nhttps://example.com/a.zip\nhttp://example.com/b.iso\nmagnet:?xt=urn:btih:123456\n";
        let links = parse_txt(input);
        assert_eq!(
            links,
            vec![
                ImportedLink::new("https://example.com/a.zip".into(), None, None),
                ImportedLink::new("http://example.com/b.iso".into(), None, None),
                ImportedLink::new("magnet:?xt=urn:btih:123456".into(), None, None),
            ]
        );
    }

    #[test]
    fn parses_csv_with_header_and_quoted_fields() {
        let input = "category,url,filename\n\"Compressed, Archive\",https://example.com/a.zip,\"my \"\"file\"\".zip\"\nVideo,https://example.com/v.mp4,clip.mp4\nVideo,https://example.com/v.mp4,dup.mp4\nInvalid,javascript:alert(1),bad\n";
        let links = parse_csv(input);
        assert_eq!(
            links,
            vec![
                ImportedLink::new(
                    "https://example.com/a.zip".into(),
                    Some("my \"file\".zip".into()),
                    Some("Compressed, Archive".into()),
                ),
                ImportedLink::new(
                    "https://example.com/v.mp4".into(),
                    Some("clip.mp4".into()),
                    Some("Video".into()),
                ),
            ]
        );
    }

    #[test]
    fn parses_csv_without_header() {
        let input = "https://example.com/one.pdf,doc.pdf,Documents\nhttp://example.com/two.mp3,,Music\n";
        let links = parse_csv(input);
        assert_eq!(
            links,
            vec![
                ImportedLink::new(
                    "https://example.com/one.pdf".into(),
                    Some("doc.pdf".into()),
                    Some("Documents".into()),
                ),
                ImportedLink::new(
                    "http://example.com/two.mp3".into(),
                    None,
                    Some("Music".into()),
                ),
            ]
        );
    }

    #[test]
    fn parses_html_href_and_src_links() {
        let input = r#"
            <html><body>
              <a href="https://example.com/dl?a=1&amp;b=2">Download</a>
              <a href='http://example.com/file.zip'>Zip</a>
              <a href="https://example.com/file.zip">Duplicate</a>
              <a href="javascript:void(0)">Ignore</a>
              <a href="magnet:?xt=urn:btih:abcdef">Magnet</a>
              <video src="https://cdn.example.com/video.mp4"></video>
              <source SRC="https://cdn.example.com/audio.mp3" />
            </body></html>
        "#;
        let links = parse_html(input);
        assert_eq!(
            links,
            vec![
                ImportedLink::new("https://example.com/dl?a=1&b=2".into(), None, None),
                ImportedLink::new("http://example.com/file.zip".into(), None, None),
                ImportedLink::new("https://example.com/file.zip".into(), None, None),
                ImportedLink::new("magnet:?xt=urn:btih:abcdef".into(), None, None),
                ImportedLink::new("https://cdn.example.com/video.mp4".into(), None, None),
                ImportedLink::new("https://cdn.example.com/audio.mp3".into(), None, None),
            ]
        );
    }

    #[test]
    fn rejects_unsupported_format() {
        let err = parse_batch_import("https://example.com/a.zip", "xml").unwrap_err();
        assert!(err.contains("Unsupported"));
    }
}
