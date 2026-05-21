//! `GET /south/mus/:id` — Merging Unit detail (live waveform).
//!
//! Phase 0 stub. Real implementation lands in WBS-9.3a (Claude).

use axum::extract::Path;
use axum::{routing::get, Router};
use maud::{html, Markup};

use crate::templates::base::{layout, Section};

/// Build the MU-detail sub-router.
pub fn router() -> Router {
    Router::new().route("/south/mus/:id", get(mu_detail))
}

async fn mu_detail(Path(id): Path<String>) -> Markup {
    layout(
        Section::Southbound,
        &format!("MU {id}"),
        html! {
            section.placeholder {
                h2 { "Merging Unit " (id) }
                p.muted {
                    "Phase 0 stub. 8-channel SVG waveform + 10 Hz downsampled "
                    "live stream land under WBS-9.3a."
                }
            }
        },
    )
}
