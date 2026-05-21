//! `svdc-api` — management HTTP/JSON server (SDD §8.4).
//!
//! Exposes four endpoints that external monitoring tools (Prometheus
//! scrapers, the master-node QSE, factory test harnesses) consume:
//!
//! | Method | Path                          | Purpose                                        |
//! | ------ | ----------------------------- | ---------------------------------------------- |
//! | GET    | `/health`                     | Liveness + data-plane integrity verdict (JSON) |
//! | GET    | `/channels`                   | Channel registry snapshot (JSON)               |
//! | GET    | `/metrics`                    | Prometheus exposition format                   |
//! | POST   | `/calibration/{channel_id}`   | Update one channel's `(gain, offset, unit_scale)` |
//!
//! This is **not** the operator-console UI — `svdc-console` serves
//! the dashboard and operator-facing `/api/*` routes. ADR-0013
//! documents the separation. The management API listens on its own
//! port (the daemon picks a non-loopback bind, the console keeps
//! loopback per ADR-0005).
//!
//! Phase 0 scope: routes + DTOs + happy-path tests against an
//! in-memory [`ManagementContext`]. Daemon wiring (where
//! `svdc-bin` constructs a context and serves this router) is a
//! follow-up PR — explicitly so this scaffold can land without
//! touching the file Antigravity is concurrently editing.
//!
//! OWNER: claude-code (scaffold + ADR-0013). Phase 3 owner extends
//! the calibration route to write through to the operational state
//! and adds auth (Phase 5).
//! NFR-10: English-only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod model;
pub mod routes;
pub mod state;

pub use state::ManagementContext;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

/// Build the management router. Wire it into the daemon with
/// `axum::serve(listener, management_router(ctx)).await`. The router
/// owns the context via `with_state` so handlers can read shared
/// data-plane handles without globals.
pub fn management_router(ctx: Arc<ManagementContext>) -> Router {
    Router::new()
        .route("/health", get(routes::health::handler))
        .route("/channels", get(routes::channels::handler))
        .route("/metrics", get(routes::metrics::handler))
        .route(
            "/calibration/:channel_id",
            post(routes::calibration::handler),
        )
        .with_state(ctx)
}
