//! `GET /config` — Configuration.
//!
//! Phase 0 stub. SCD validator + channel-registry model land under
//! WBS-9.6a (Claude). Configuration form + About page land under
//! WBS-9.6b (Antigravity).

use axum::{routing::get, Router};
use maud::{html, Markup};

use crate::templates::base::{layout, Section};

/// Build the Configuration sub-router.
pub fn router() -> Router {
    Router::new().route("/config", get(config))
}

async fn config() -> Markup {
    layout(
        Section::Configuration,
        "Configuration",
        html! {
            section.placeholder {
                h2 { "Configuration" }
                p.muted {
                    "Phase 0 stub. SCD upload, channel registry, parameters, "
                    "and About page land under WBS-9.6a / 9.6b in Phase 4."
                }
            }
        },
    )
}
