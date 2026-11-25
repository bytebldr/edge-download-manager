use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RpcRequest {
    pub jsonrpc: Option<String>,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RpcAction {
    GetVersion,
    AddUri {
        uris: Vec<String>,
        options: Map<String, Value>,
    },
    Pause {
        gid: String,
    },
    Unpause {
        gid: String,
    },
    Remove {
        gid: String,
    },
    TellActive,
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    let len = a.len().min(b.len());
    for i in 0..len {
        diff |= (a[i] ^ b[i]) as usize;
    }
    diff == 0
}

fn strip_and_verify_token<'a>(
    params: &'a [Value],
    expected_secret: &str,
) -> Result<&'a [Value], (i32, &'static str)> {
    let first_token = params
        .first()
        .and_then(Value::as_str)
        .and_then(|s| s.strip_prefix("token:"));

    if !expected_secret.is_empty() {
        let Some(provided) = first_token else {
            return Err((-32600, "Unauthorized: missing RPC secret token"));
        };
        if !constant_time_eq(provided.as_bytes(), expected_secret.as_bytes()) {
            return Err((-32600, "Unauthorized: invalid RPC secret token"));
        }
        return Ok(&params[1..]);
    }

    if first_token.is_some() {
        Ok(&params[1..])
    } else {
        Ok(params)
    }
}

fn require_gid(params: &[Value]) -> Result<String, (i32, &'static str)> {
    params
        .first()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or((-32602, "Invalid params: expected non-empty gid string"))
}

/// Parse and authenticate an `aria2`-compatible JSON-RPC 2.0 request.
pub fn parse_aria2_rpc(
    raw: &str,
    expected_secret: &str,
) -> Result<(Value, RpcAction), RpcResponse> {
    let req: RpcRequest = serde_json::from_str(raw)
        .map_err(|_| RpcResponse::err(Value::Null, -32700, "Parse error"))?;

    let id = req.id.unwrap_or(Value::Null);

    if let Some(version) = req.jsonrpc.as_deref() {
        if version != "2.0" {
            return Err(RpcResponse::err(
                id,
                -32600,
                "Invalid Request: jsonrpc must be 2.0",
            ));
        }
    }

    let args = strip_and_verify_token(&req.params, expected_secret)
        .map_err(|(code, msg)| RpcResponse::err(id.clone(), code, msg))?;

    let action = match req.method.as_str() {
        "aria2.getVersion" => RpcAction::GetVersion,
        "aria2.addUri" => {
            let uris = args
                .first()
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    RpcResponse::err(
                        id.clone(),
                        -32602,
                        "Invalid params: expected non-empty URI array",
                    )
                })?;

            let options = match args.get(1) {
                Some(Value::Object(map)) => map.clone(),
                Some(Value::Null) | None => Map::new(),
                Some(_) => {
                    return Err(RpcResponse::err(
                        id,
                        -32602,
                        "Invalid params: options must be an object",
                    ));
                }
            };

            RpcAction::AddUri { uris, options }
        }
        "aria2.pause" => {
            let gid = require_gid(args).map_err(|(c, m)| RpcResponse::err(id.clone(), c, m))?;
            RpcAction::Pause { gid }
        }
        "aria2.unpause" => {
            let gid = require_gid(args).map_err(|(c, m)| RpcResponse::err(id.clone(), c, m))?;
            RpcAction::Unpause { gid }
        }
        "aria2.remove" => {
            let gid = require_gid(args).map_err(|(c, m)| RpcResponse::err(id.clone(), c, m))?;
            RpcAction::Remove { gid }
        }
        "aria2.tellActive" => RpcAction::TellActive,
        _ => return Err(RpcResponse::err(id, -32601, "Method not found")),
    };

    Ok((id, action))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_get_version_with_valid_secret() {
        let raw =
            r#"{"jsonrpc":"2.0","id":"1","method":"aria2.getVersion","params":["token:mysecret"]}"#;
        let (id, action) = parse_aria2_rpc(raw, "mysecret").unwrap();
        assert_eq!(id, json!("1"));
        assert_eq!(action, RpcAction::GetVersion);
    }

    #[test]
    fn rejects_invalid_or_missing_secret() {
        let missing = r#"{"jsonrpc":"2.0","id":1,"method":"aria2.tellActive","params":[]}"#;
        let err = parse_aria2_rpc(missing, "secret").unwrap_err();
        assert_eq!(err.error.as_ref().unwrap().code, -32600);

        let wrong =
            r#"{"jsonrpc":"2.0","id":2,"method":"aria2.tellActive","params":["token:nope"]}"#;
        let err = parse_aria2_rpc(wrong, "secret").unwrap_err();
        assert_eq!(err.error.as_ref().unwrap().code, -32600);
    }

    #[test]
    fn parses_add_uri_with_options_and_empty_secret() {
        let raw = r#"{"jsonrpc":"2.0","id":7,"method":"aria2.addUri","params":[["https://example.com/a.iso"],{"dir":"/tmp"}]}"#;
        let (id, action) = parse_aria2_rpc(raw, "").unwrap();
        assert_eq!(id, json!(7));
        match action {
            RpcAction::AddUri { uris, options } => {
                assert_eq!(uris, vec!["https://example.com/a.iso"]);
                assert_eq!(options.get("dir").unwrap(), &json!("/tmp"));
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn parses_pause_unpause_remove_and_tell_active() {
        for (method, expected) in [
            ("aria2.pause", RpcAction::Pause { gid: "g1".into() }),
            ("aria2.unpause", RpcAction::Unpause { gid: "g1".into() }),
            ("aria2.remove", RpcAction::Remove { gid: "g1".into() }),
        ] {
            let raw = json!({"jsonrpc":"2.0","id":"x","method":method,"params":["token:s","g1"]})
                .to_string();
            let (_, action) = parse_aria2_rpc(&raw, "s").unwrap();
            assert_eq!(action, expected);
        }

        let active = r#"{"jsonrpc":"2.0","id":9,"method":"aria2.tellActive","params":["token:s"]}"#;
        let (_, action) = parse_aria2_rpc(active, "s").unwrap();
        assert_eq!(action, RpcAction::TellActive);
    }

    #[test]
    fn rejects_invalid_json_and_unknown_method() {
        let bad_json = parse_aria2_rpc("{not json", "").unwrap_err();
        assert_eq!(bad_json.error.unwrap().code, -32700);

        let unknown = parse_aria2_rpc(r#"{"jsonrpc":"2.0","id":1,"method":"aria2.unknown"}"#, "")
            .unwrap_err();
        assert_eq!(unknown.error.unwrap().code, -32601);
    }
}
