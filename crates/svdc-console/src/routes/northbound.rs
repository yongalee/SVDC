//! `GET /north` and `GET /north/:layer` — North-bound application layers.
//!
//! Phase 0 stub. Shell + enable/disable API contract lands in WBS-9.4a
//! (Claude); per-layer (L0/L1/L2/L3) detail cards land in WBS-9.4b
//! (Antigravity).

use axum::extract::Path;
use axum::{routing::get, Router};
use maud::{html, Markup};

use crate::templates::base::{layout, Section};

/// Build the North-bound sub-router.
pub fn router() -> Router {
    Router::new()
        .route("/north", get(north_index))
        .route("/north/:layer", get(north_layer))
}

async fn north_index() -> Markup {
    layout(
        Section::Northbound,
        "North-bound application layers",
        html! {
            section.placeholder {
                h2 { "North-bound layers" }
                p.muted {
                    "Phase 0 stub. Layer cards (L0 in-process, L1 OPC UA, "
                    "L2 MQTT, L3 TimescaleDB) and enable/disable POST API "
                    "land under WBS-9.4a / 9.4b."
                }
            }
        },
    )
}

async fn north_layer(Path(layer): Path<String>) -> Markup {
    layout(
        Section::Northbound,
        &format!("Layer {layer}"),
        html! {
            section.placeholder {
                h2 { "Layer " (layer) }
                p.muted {
                    "Phase 0 stub. Per-layer detail (clients, throughput, "
                    "config) lands under WBS-9.4b."
                }
            }
        },
    )
}
