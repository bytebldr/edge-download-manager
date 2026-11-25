use serde::{Deserialize, Serialize};
use url::Url;

use super::proxy_rules::validate_proxy_url;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RequestOverride {
    pub url: Option<String>,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    pub proxy_url: Option<String>,
}

impl RequestOverride {
    /// Validates URL, proxy URL, and HTTP headers, rejecting CRLF/NUL injection.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref raw_url) = self.url {
            let trimmed = raw_url.trim();
            if trimmed.is_empty() {
                return Err("Override URL cannot be empty".into());
            }
            if contains_ctl_break(trimmed) {
                return Err("Override URL contains invalid control characters".into());
            }
            let parsed = Url::parse(trimmed).map_err(|e| format!("Invalid override URL: {e}"))?;
            match parsed.scheme() {
                "http" | "https" => {}
                other => {
                    return Err(format!(
                        "Unsupported URL scheme '{other}' (expected http or https)"
                    ));
                }
            }
            if parsed.host_str().is_none() {
                return Err("Override URL must include a host".into());
            }
        }

        for (name, value) in &self.headers {
            let trimmed_name = name.trim();
            if trimmed_name.is_empty() {
                return Err("Header name cannot be empty".into());
            }
            if !is_valid_header_name(trimmed_name) {
                return Err(format!("Invalid HTTP header name '{trimmed_name}'"));
            }
            if contains_ctl_break(value) {
                return Err(format!(
                    "Header '{trimmed_name}' value contains illegal CR/LF or NUL characters"
                ));
            }
        }

        if let Some(ref proxy) = self.proxy_url {
            let trimmed = proxy.trim();
            if !trimmed.is_empty() {
                validate_proxy_url(trimmed)?;
            }
        }

        Ok(())
    }
}

fn contains_ctl_break(s: &str) -> bool {
    s.bytes().any(|b| b == b'\r' || b == b'\n' || b == b'\0')
}

/// RFC 7230 token characters for HTTP header field names.
fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_request_override() {
        let req = RequestOverride {
            url: Some("https://cdn.example.com/archive.zip?token=abc".into()),
            headers: vec![
                ("Cookie".into(), "session=xyz123".into()),
                ("Referer".into(), "https://example.com/".into()),
                ("Authorization".into(), "Bearer tok_123".into()),
            ],
            proxy_url: Some("socks5://127.0.0.1:1080".into()),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn blocks_crlf_header_injection() {
        let req_cr = RequestOverride {
            url: None,
            headers: vec![("Referer".into(), "https://a.com\r\nX-Evil: 1".into())],
            proxy_url: None,
        };
        assert!(req_cr.validate().is_err());

        let req_lf = RequestOverride {
            url: None,
            headers: vec![("Cookie".into(), "a=1\nSet-Cookie: b=2".into())],
            proxy_url: None,
        };
        assert!(req_lf.validate().is_err());

        let req_name = RequestOverride {
            url: None,
            headers: vec![("Bad Header\r\n".into(), "val".into())],
            proxy_url: None,
        };
        assert!(req_name.validate().is_err());
    }

    #[test]
    fn blocks_unsafe_url_schemes() {
        let req = RequestOverride {
            url: Some("file:///etc/passwd".into()),
            headers: vec![],
            proxy_url: None,
        };
        assert!(req.validate().is_err());
    }
}
