/* SVDC Northbound Adapters Router
   OWNER: claude-code
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers

   Note: This file is assigned to Claude Code under WBS-9.4a for the route shell
   and POST handler endpoints. Antigravity scaffolds the complete interactive L0/L1/L2/L3
   layer cards and dynamic HTMX toggles (WBS-9.4b) to allow parallel verification.
*/

use axum::{
    extract::Path,
    response::Html,
    routing::{get, post},
    Router,
};
use maud::html;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::templates::{base, components};

// Thread-safe atomic states to persist user toggles across dashboard sessions
static L0_ENABLED: AtomicBool = AtomicBool::new(true);
static L1_ENABLED: AtomicBool = AtomicBool::new(true);
static L2_ENABLED: AtomicBool = AtomicBool::new(false);
static L3_ENABLED: AtomicBool = AtomicBool::new(true);

/// Register routes related to northbound adapters controls and API
pub fn register(router: Router) -> Router {
    router
        .route("/north", get(northbound_page))
        .route("/api/v1/northbound/:layer/toggle", post(toggle_adapter))
}

/// Renders the Northbound Controls page
async fn northbound_page() -> Html<String> {
    let l0_active = L0_ENABLED.load(Ordering::Relaxed);
    let l1_active = L1_ENABLED.load(Ordering::Relaxed);
    let l2_active = L2_ENABLED.load(Ordering::Relaxed);
    let l3_active = L3_ENABLED.load(Ordering::Relaxed);

    let content = html! {
        div class="screen-layout gap-6" {
            // High-level explanation block
            div class="glass-card mb-4" {
                div class="card-header flex items-center gap-2" {
                    h2 class="card-title" { "Northbound Adapters Controller" }
                }
                div class="card-body mt-2 text-sm text-text-secondary" {
                    p {
                        "The northbound adapters layer exposes calibrated, aligned telemetry streams "
                        "to all node-local and enterprise operational applications. "
                        "Each layer serves a distinct communication architecture, and can be dynamically "
                        "enabled, disabled, or isolated to optimize compute resources and network overhead."
                    }
                }
            }

            // Grid for L0, L1, L2, L3 adapters
            div class="grid grid-cols-1 md:grid-cols-2 gap-6" {
                // L0: In-Process Shared Memory
                (components::northbound_card(
                    "L0",
                    "Shared Memory RingBuffer",
                    if l0_active { "Active" } else { "Inactive" },
                    "/dev/shm/svdc_l0_ring",
                    if l0_active { 3 } else { 0 },
                    if l0_active { 4000 } else { 0 },
                    l0_active,
                ))

                // L1: SCADA OPC UA Server
                (components::northbound_card(
                    "L1",
                    "SCADA OPC UA Server",
                    if l1_active { "Locked" } else { "Inactive" },
                    "opc.tcp://127.0.0.1:4840/free/svdc/server",
                    if l1_active { 2 } else { 0 },
                    if l1_active { 4000 } else { 0 },
                    l1_active,
                ))

                // L2: MQTT Cloud Publisher
                (components::northbound_card(
                    "L2",
                    "MQTT Cloud Publisher",
                    if l2_active { "Active" } else { "Inactive" },
                    "mqtt://broker.hivemq.com:1883/ssiec/svdc/telemetry",
                    if l2_active { 1 } else { 0 },
                    if l2_active { 4000 } else { 0 },
                    l2_active,
                ))

                // L3: TimescaleDB Sidecar
                (components::northbound_card(
                    "L3",
                    "TimescaleDB Sidecar",
                    if l3_active { "Active" } else { "Inactive" },
                    "postgresql://svdc_user:pass@127.0.0.1:5432/svdc_archive",
                    if l3_active { 1 } else { 0 },
                    if l3_active { 4000 } else { 0 },
                    l3_active,
                ))
            }
        }
    };

    let rendered = base::layout("Northbound Controls", "northbound", content);
    Html(rendered.into_string())
}

/// Dynamic POST endpoint for enabling/disabling northbound adapters.
/// Triggered via Alpine/HTMX switch toggle and returns the updated card HTML.
async fn toggle_adapter(Path(layer): Path<String>) -> Html<String> {
    let layer_upper = layer.to_uppercase();

    let (name, endpoint, consumers, throughput, is_now_enabled) = match layer.as_str() {
        "l0" => {
            let next_state = !L0_ENABLED.load(Ordering::Relaxed);
            L0_ENABLED.store(next_state, Ordering::Relaxed);
            (
                "Shared Memory RingBuffer",
                "/dev/shm/svdc_l0_ring",
                if next_state { 3 } else { 0 },
                if next_state { 4000 } else { 0 },
                next_state,
            )
        }
        "l1" => {
            let next_state = !L1_ENABLED.load(Ordering::Relaxed);
            L1_ENABLED.store(next_state, Ordering::Relaxed);
            (
                "SCADA OPC UA Server",
                "opc.tcp://127.0.0.1:4840/free/svdc/server",
                if next_state { 2 } else { 0 },
                if next_state { 4000 } else { 0 },
                next_state,
            )
        }
        "l2" => {
            let next_state = !L2_ENABLED.load(Ordering::Relaxed);
            L2_ENABLED.store(next_state, Ordering::Relaxed);
            (
                "MQTT Cloud Publisher",
                "mqtt://broker.hivemq.com:1883/ssiec/svdc/telemetry",
                if next_state { 1 } else { 0 },
                if next_state { 4000 } else { 0 },
                next_state,
            )
        }
        _ => {
            let next_state = !L3_ENABLED.load(Ordering::Relaxed);
            L3_ENABLED.store(next_state, Ordering::Relaxed);
            (
                "TimescaleDB Sidecar",
                "postgresql://svdc_user:pass@127.0.0.1:5432/svdc_archive",
                if next_state { 1 } else { 0 },
                if next_state { 4000 } else { 0 },
                next_state,
            )
        }
    };

    let display_status = if is_now_enabled {
        if layer == "l1" {
            "Locked"
        } else {
            "Active"
        }
    } else {
        "Inactive"
    };

    let card_markup = components::northbound_card(
        &layer_upper,
        name,
        display_status,
        endpoint,
        consumers,
        throughput,
        is_now_enabled,
    );

    Html(card_markup.into_string())
}
