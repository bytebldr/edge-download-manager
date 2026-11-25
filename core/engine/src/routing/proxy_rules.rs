use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DomainProxyRule {
    pub id: String,
    pub pattern: String,
    pub proxy_url: String,
    pub enabled: bool,
}

/// Validates that a proxy URL has a supported scheme and no control characters.
pub fn validate_proxy_url(proxy_url: &str) -> Result<(), String> {
    let trimmed = proxy_url.trim();
    if trimmed.is_empty() {
        return Err("Proxy URL cannot be empty".into());
    }
    if trimmed.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
        return Err("Proxy URL contains invalid control characters".into());
    }
    let parsed = Url::parse(trimmed).map_err(|e| format!("Invalid proxy URL: {e}"))?;
    match parsed.scheme() {
        "http" | "https" | "socks4" | "socks5" | "socks5h" => {}
        other => {
            return Err(format!(
                "Unsupported proxy scheme '{other}' (expected http, https, socks4, socks5, or socks5h)"
            ));
        }
    }
    if parsed.host_str().is_none() {
        return Err("Proxy URL must include a host".into());
    }
    Ok(())
}

/// Matches a hostname against a domain pattern (case-insensitive).
/// Supports `*` (match any), `*.domain.tld` (any subdomain of `domain.tld`),
/// and general `*` wildcards.
pub fn matches_domain_pattern(host: &str, pattern: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let pattern = pattern.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() || pattern.is_empty() {
        return false;
    }
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        // Require at least one subdomain label before `.suffix`
        return host.len() > suffix.len() + 1 && host.ends_with(&format!(".{suffix}"));
    }
    wildcard_match(&pattern, &host)
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (None, 0);

    while ti < t.len() {
        if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Resolves the first enabled proxy rule matching the URL's host.
pub fn resolve_proxy_for_url(url: &str, rules: &[DomainProxyRule]) -> Option<String> {
    let parsed = Url::parse(url.trim()).ok()?;
    let host = parsed.host_str()?;

    rules.iter().find_map(|rule| {
        if !rule.enabled {
            return None;
        }
        let proxy = rule.proxy_url.trim();
        if proxy.is_empty() {
            return None;
        }
        if matches_domain_pattern(host, &rule.pattern) {
            Some(proxy.to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, pattern: &str, proxy_url: &str, enabled: bool) -> DomainProxyRule {
        DomainProxyRule {
            id: id.into(),
            pattern: pattern.into(),
            proxy_url: proxy_url.into(),
            enabled,
        }
    }

    #[test]
    fn resolves_exact_and_wildcard_domains() {
        let rules = vec![
            rule("1", "*.github.com", "socks5://127.0.0.1:1080", true),
            rule("2", "huggingface.co", "http://127.0.0.1:8080", true),
        ];

        assert_eq!(
            resolve_proxy_for_url("https://api.github.com/repos/a/b", &rules),
            Some("socks5://127.0.0.1:1080".into())
        );
        assert_eq!(
            resolve_proxy_for_url("https://objects.raw.github.com/file.zip", &rules),
            Some("socks5://127.0.0.1:1080".into())
        );
        assert_eq!(
            resolve_proxy_for_url("https://github.com/login", &rules),
            None
        );
        assert_eq!(
            resolve_proxy_for_url("https://HUGGINGFACE.CO:443/model.bin", &rules),
            Some("http://127.0.0.1:8080".into())
        );
    }

    #[test]
    fn skips_disabled_rules_and_respects_order() {
        let rules = vec![
            rule("1", "*.example.com", "http://disabled:8000", false),
            rule("2", "*.example.com", "socks5://active:1080", true),
            rule("3", "*", "http://fallback:3128", true),
        ];

        assert_eq!(
            resolve_proxy_for_url("https://cdn.example.com/a.tar.gz", &rules),
            Some("socks5://active:1080".into())
        );
        assert_eq!(
            resolve_proxy_for_url("https://other.org/file", &rules),
            Some("http://fallback:3128".into())
        );
    }

    #[test]
    fn rejects_invalid_proxy_urls() {
        assert!(validate_proxy_url("socks5://127.0.0.1:1080").is_ok());
        assert!(validate_proxy_url("http://proxy.local:3128").is_ok());
        assert!(validate_proxy_url("").is_err());
        assert!(validate_proxy_url("ftp://proxy.local:21").is_err());
        assert!(validate_proxy_url("http://proxy.local:3128\r\nX-Inject: 1").is_err());
    }
}
