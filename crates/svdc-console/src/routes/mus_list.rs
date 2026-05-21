//! `GET /south/mus` — Merging Units list.
//!
//! Phase 0 stub: returns the base layout with a placeholder note.
//! Real implementation lands in WBS-9.3b (Antigravity).

use axum::{routing::get, Router};
use maud::{html, Markup};

use crate::templates::base::{layout, Section};

/// Build the MU-list sub-router.
pub fn router() -> Router {
    Router::new().route("/south/mus", get(mus_list))
}

async fn mus_list() -> Markup {
    layout(
        Section::Southbound,
        "Merging Units",
        html! {
            section.placeholder {
                h2 { "Merging Units" }
                p.muted {
                    "Phase 0 stub. Per-MU cards with status, sample rate, jitter, "
                    "missing-sample count, and last-seen timestamp land under "
                    "WBS-9.3b."
                }
            }
        },
    )
}
