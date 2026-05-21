//! `GET /sse/dashboard` and `GET /api/events` — Server-Sent Events
//! stream for live tiles. Both URLs map to the same handler; the
//! `/api/events` alias matches the convention the frontend lane
//! aligned on (see `docs/telemetry_sse_simulator_sync.md`).
//!
//! Wraps the `tokio::sync::broadcast::Receiver<String>` produced by
//! `crate::sse::emitter::subscribe()` into an axum SSE response. Each
//! broadcast item is already a JSON-encoded `SsePayload` (per
//! `sse::mod::emitter`); we just box it into an SSE Event.
//!
//! OWNER: claude-code (WBS-9.2a — the contract / consumer wiring)

use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use axum::Router;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::sse::emitter;

/// Build the SSE sub-router. `/api/events` is the canonical URL
/// (per the telemetry sync spec); `/sse/dashboard` is kept as an
/// alias so older htmx attributes continue to work.
pub fn router() -> Router {
    Router::new()
        .route("/api/events", get(dashboard_stream))
        .route("/sse/dashboard", get(dashboard_stream))
}

async fn dashboard_stream() -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = emitter::subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|item| {
        match item {
            Ok(json) => Some(Ok(Event::default().data(json))),
            // Lagging receivers drop messages; we just skip those rather
            // than tear the stream down. The next message arrives in <=1 s.
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// Both URLs must reach the same handler. The handler returns
    /// 200 + an open SSE stream; we just confirm the status code
    /// and the content-type header here — pulling events out of
    /// the broadcast in a sync test is fragile.
    async fn route_returns_sse(uri: &'static str) {
        let app = router();
        let resp = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{uri} should return 200");
        let ct = resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.starts_with("text/event-stream"),
            "{uri} content-type should be text/event-stream, got {ct:?}"
        );
    }

    #[tokio::test]
    async fn canonical_api_events_route_returns_sse() {
        route_returns_sse("/api/events").await;
    }

    #[tokio::test]
    async fn legacy_sse_dashboard_alias_returns_sse() {
        route_returns_sse("/sse/dashboard").await;
    }
}
