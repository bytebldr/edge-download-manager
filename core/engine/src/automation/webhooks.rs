use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct AutomationConfig {
    pub rpc_enabled: bool,
    pub rpc_port: u16,
    pub rpc_secret: String,
    pub webhook_url: Option<String>,
    pub on_complete_enabled: bool,
    pub trim_memory_on_tray: bool,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            rpc_enabled: false,
            rpc_port: 6800,
            rpc_secret: String::new(),
            webhook_url: None,
            on_complete_enabled: false,
            trim_memory_on_tray: true,
        }
    }
}

/// Validate that a webhook URL is well-formed and uses `http://` or `https://`.
pub fn validate_webhook_url(raw: &str) -> Result<Url, String> {
    let parsed = Url::parse(raw.trim()).map_err(|e| format!("Invalid webhook URL: {e}"))?;
    match parsed.scheme() {
        "http" | "https" if parsed.has_host() => Ok(parsed),
        _ => Err("Webhook URL must use http:// or https:// with a valid host".to_string()),
    }
}

/// POST a JSON completion payload to `url` with a 10-second timeout.
pub async fn dispatch_completion_webhook(url: &str, payload: Value) -> Result<u16, String> {
    let parsed = validate_webhook_url(url)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let resp = client
        .post(parsed)
        .header("Content-Type", "application/json")
        .body(payload.to_string())
        .send()
        .await
        .map_err(|e| format!("Webhook request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("Webhook returned HTTP {}", status.as_u16()));
    }
    Ok(status.as_u16())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Json, Router};
    use serde_json::json;
    use tokio::net::TcpListener;

    #[test]
    fn config_round_trips_camel_case() {
        let cfg = AutomationConfig {
            rpc_enabled: true,
            rpc_port: 6801,
            rpc_secret: "tok".into(),
            webhook_url: Some("https://hooks.example.com/done".into()),
            on_complete_enabled: true,
            trim_memory_on_tray: false,
        };
        let raw = serde_json::to_string(&cfg).unwrap();
        assert!(raw.contains("rpcEnabled"));
        let decoded: AutomationConfig = serde_json::from_str(&raw).unwrap();
        assert_eq!(decoded, cfg);
    }

    #[test]
    fn validates_http_and_https_urls_only() {
        assert!(validate_webhook_url("https://example.com/hook").is_ok());
        assert!(validate_webhook_url("http://127.0.0.1:9000/hook").is_ok());
        assert!(validate_webhook_url("ftp://example.com/hook").is_err());
        assert!(validate_webhook_url("javascript:alert(1)").is_err());
        assert!(validate_webhook_url("not a url").is_err());
    }

    #[tokio::test]
    async fn dispatches_webhook_payload_over_http() {
        let app = Router::new().route(
            "/hook",
            post(|Json(body): Json<Value>| async move {
                assert_eq!(body["event"], "edm.test");
                axum::http::StatusCode::OK
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let url = format!("http://{addr}/hook");
        let status = dispatch_completion_webhook(&url, json!({ "event": "edm.test" }))
            .await
            .unwrap();
        assert_eq!(status, 200);
    }

    #[tokio::test]
    async fn rejects_invalid_scheme_before_network() {
        let err = dispatch_completion_webhook("file:///etc/passwd", json!({}))
            .await
            .unwrap_err();
        assert!(err.contains("http:// or https://"));
    }
}
