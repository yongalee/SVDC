//! `GET /sse/dashboard` — Server-Sent Events stream for live tiles.
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

/// Build the SSE sub-router.
pub fn router() -> Router {
    Router::new().route("/sse/dashboard", get(dashboard_stream))
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
