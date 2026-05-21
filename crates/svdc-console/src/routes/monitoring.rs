//! `GET /monitoring` — Performance and audit.
//!
//! Phase 0 stub. Latency histogram + p50/p99 lands under WBS-9.5a
//! (Claude). PTP/CB line charts and audit log table land under
//! WBS-9.5b (Antigravity).

use axum::{routing::get, Router};
use maud::{html, Markup};

use crate::templates::base::{layout, Section};

/// Build the Monitoring sub-router.
pub fn router() -> Router {
    Router::new().route("/monitoring", get(monitoring))
}

async fn monitoring() -> Markup {
    layout(
        Section::Monitoring,
        "Monitoring",
        html! {
            section.placeholder {
                h2 { "Monitoring" }
                p.muted {
                    "Phase 0 stub. Latency histogram, PTP/CB charts, and "
                    "audit-log table land under WBS-9.5a / 9.5b in Phase 5."
                }
            }
        },
    )
}
