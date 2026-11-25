use crate::config;
use crate::handlers::handle_message;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use edm_ipc::{AppReply, ExtensionMessage};
use edm_store::Store;
use std::io;
use std::sync::{Arc, Mutex};
use tower_http::cors::{AllowOrigin, CorsLayer};

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    std::hint::black_box(diff) == 0
}

async fn handle_api_request(
    headers: HeaderMap,
    State(store): State<Arc<Mutex<Store>>>,
    Json(msg): Json<ExtensionMessage>,
) -> Result<Json<AppReply>, (StatusCode, &'static str)> {
    // Security Guard 1: Exact loopback Host enforcement
    let host_ok = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(config::is_loopback_host);
    if !host_ok {
        return Err((StatusCode::FORBIDDEN, "Forbidden: Host must be loopback"));
    }

    // Security Guard 2: Anti-CSRF Origin check (allowlisted extensions only)
    if let Some(origin) = headers.get(axum::http::header::ORIGIN) {
        let valid_origin = origin.to_str().is_ok_and(config::is_allowed_http_origin);
        if !valid_origin {
            return Err((StatusCode::FORBIDDEN, "Forbidden: Untrusted Origin"));
        }
    }

    // Security Guard 3: Block cross-site browser fetches
    if let Some(sec_site) = headers.get("sec-fetch-site") {
        if sec_site.to_str().ok() == Some("cross-site") {
            let is_allowed_ext = headers
                .get(axum::http::header::ORIGIN)
                .and_then(|o| o.to_str().ok())
                .is_some_and(config::is_allowed_http_origin);
            if !is_allowed_ext {
                return Err((
                    StatusCode::FORBIDDEN,
                    "Forbidden: Cross-site request rejected",
                ));
            }
        }
    }

    // Security Guard 4: Constant-time x-edm-token verification
    let expected_token = config::get_ws_token().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Service Unavailable: Auth token not initialized",
    ))?;
    let provided_token = headers
        .get("x-edm-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !constant_time_eq(provided_token.as_bytes(), expected_token.as_bytes()) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Unauthorized: Invalid or missing x-edm-token",
        ));
    }

    let reply = handle_message(msg, &store).await;
    Ok(Json(reply))
}

pub async fn serve(store: Arc<Mutex<Store>>) -> io::Result<()> {
    let app = Router::new()
        .route("/api", post(handle_api_request))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
                    origin.to_str().is_ok_and(config::is_allowed_http_origin)
                }))
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(store);

    tracing::info!(
        "EDM Native Host HTTP Bridge listening on http://{}/api",
        config::HTTP_BRIDGE_ADDR
    );
    let listener = tokio::net::TcpListener::bind(config::HTTP_BRIDGE_ADDR).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
