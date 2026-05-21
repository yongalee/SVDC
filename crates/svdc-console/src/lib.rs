//! SVDC Operator Console — `svdc-console`
//!
//! Embedded axum-based web UI per `docs/SVDC_UI_Design_Document_v0.1.html`
//! and ADR-0004 (stack), wired into the SVDC daemon via ADR-0005's
//! `--no-ui` / `--ui-bind` toggle.
//!
//! Public surface:
//! - [`router`] returns an `axum::Router` with all routes registered.
//! - [`start_console`] binds the router to a socket and serves until the
//!   process is shut down. Should be invoked from a tokio runtime.
//!
//! OWNER: claude-code (WBS-9.1a)
//! Co-authored with antigravity-subagent-ui-spec (WBS-9.1b scaffold, 9.2b emitter)
//! NFR-10: English-only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Embedded static assets (CSS, JS, fonts) served from the binary.
pub mod assets;
/// Audit log: typed in-memory ring of operator state changes.
pub mod audit;
/// In-process data-plane runner that drives `svdc-ingress` →
/// `svdc-aligner` → `TickBuffer` → `svdc-historian`. Used by the
/// `/dataplane` UI to verify end-to-end behaviour without needing a
/// separate daemon.
pub mod dataplane;
/// SVDC-local operational state (calibration triples, subscription flags).
/// Distinct from the SCD-derived registry — SCD is immutable per IEC 61850-6.
pub mod operational;
/// HTTP route handlers, one module per UI screen plus assets and SSE.
pub mod routes;
/// IEC 61850 SCL/SCD parser and the in-process channel registry.
pub mod scd;
/// Typed Server-Sent Event payloads and the background emitter.
pub mod sse;
/// maud template fragments: base layout and reusable components.
pub mod templates;

use std::net::SocketAddr;

use axum::Router;

/// Build the axum router with every console route registered. Pure
/// function: safe to call multiple times for tests.
///
/// Also mounts the `svdc-api` management router under `/api/mgmt/*`
/// so the SDD §8.4 endpoints (`/health`, `/channels`, `/metrics`,
/// `/calibration`) are reachable from the same listener. ADR-0013
/// allows merging into a single port when the deployment doesn't
/// need separate network ACLs; the merged mount is convenient for
/// Phase 0 verification.
pub fn router() -> Router {
    let mgmt_ctx = std::sync::Arc::new(svdc_api::ManagementContext::new(std::sync::Arc::clone(
        &dataplane::global().buffer,
    )));
    Router::new()
        .merge(routes::dashboard::router())
        .merge(routes::mus_list::router())
        .merge(routes::mu_detail::router())
        .merge(routes::northbound::router())
        .merge(routes::monitoring::router())
        .merge(routes::config::router())
        .merge(routes::calibration::router())
        .merge(routes::audit::router())
        .merge(routes::sse::router())
        .merge(routes::dataplane::router())
        .merge(routes::assets::router())
        .nest("/api/mgmt", svdc_api::management_router(mgmt_ctx))
}

/// Bind the console router to `addr` and serve until interrupted.
///
/// Returns when the process receives Ctrl-C (SIGINT). The function
/// constructs no listening socket beyond the one specified; the
/// management API and the protection data path remain on their own
/// listeners.
pub async fn start_console(addr: SocketAddr) -> std::io::Result<()> {
    let app = router();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "operator console listening");
    println!("svdc-console: listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("operator console: received Ctrl-C, shutting down");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_builds_without_panic() {
        let _ = router();
    }
}
