//! `GET /assets/{path}` — serve embedded static assets (CSS, JS, fonts).
//!
//! Reads from the `rust_embed::RustEmbed` struct defined in
//! `crate::assets`. Single-binary delivery is preserved: nothing on
//! disk is read at runtime.
//!
//! OWNER: claude-code (WBS-9.1a; reads Antigravity-owned asset blobs)

use axum::extract::Path;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;

use crate::assets::Assets;

/// Build the assets sub-router.
pub fn router() -> Router {
    Router::new().route("/assets/*path", get(serve_asset))
}

async fn serve_asset(Path(path): Path<String>) -> impl IntoResponse {
    match <Assets as RustEmbed>::get(&path) {
        Some(file) => {
            let mut headers = HeaderMap::new();
            if let Ok(value) = HeaderValue::from_str(file.metadata.mimetype()) {
                headers.insert(header::CONTENT_TYPE, value);
            }
            // Embedded assets never change between builds; let browsers
            // cache aggressively. The deploy binary version is what
            // invalidates caches in practice.
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=86400"),
            );
            (StatusCode::OK, headers, file.data.into_owned()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
