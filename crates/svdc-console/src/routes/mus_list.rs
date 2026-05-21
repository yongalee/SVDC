/* SVDC Southbound Merging Units Router
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use axum::{
    extract::Path,
    response::Html,
    routing::{get, post},
    Router,
};
use maud::html;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::templates::{base, components};

/// Register routes related to southbound Merging Units list and actions
pub fn register(router: Router) -> Router {
    router
        .route("/south/mus", get(mus_list_page))
        .route("/api/v1/merging-units/:id/ping", post(ping_mu))
}

/// Renders the Southbound Merging Units page
async fn mus_list_page() -> Html<String> {
    let content = html! {
            div class="screen-layout gap-6" {
                // Summary header
                div class="glass-card mb-4" {
                    div class="card-header flex items-center gap-2" {
                        span class="card-icon" {
                            svg class="w-4 h-4 text-accent-blue" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                path stroke-linecap="round" stroke-linejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" {}
                                path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" {}
                            }
                        }
                        h2 class="card-title" { "Southbound Ingest Core" }
                    }
                    div class="card-body mt-2 text-sm text-text-secondary" {
                        p {
                            "The southbound ingest engine processes raw IEC 61850-9-2 Sampled Values (SV) frames "
                            "broadcast from Merging Units (MUs) connected to the substation process bus. "
                            "Incoming frames are received with zero heap allocation, calibrated using configured offsets, "
                            "and immediately written into the dual-redundant circular buffers."
                        }
                    }
                }

                // Grid layout for MU cards
                div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6" {
                    // MU-01: Active, Healthy
                    (components::mu_card(
                        "MU-01",
                        "192.168.1.101",
                        "00:50:C2:88:99:A1",
                        "Healthy",
                        4000,
                        0,
                        Some(4),
                    ))

                    // MU-02: Standby/Holdover (no packets arriving)
                    (components::mu_card(
                        "MU-02",
                        "192.168.1.102",
                        "00:50:C2:88:99:A2",
                        "Degraded",
                        0,
                        142,
                        None,
                    ))

                    // MU-03: Offline
                    (components::mu_card(
                        "MU-03",
                        "192.168.1.103",
                        "00:50:C2:88:99:A3",
                        "Disconnected",
                        0,
                        8563,
                        None,
                    ))
            }
        }
    };

    let rendered = base::layout("Southbound Merging Units", "southbound", content);
    Html(rendered.into_string())
}

/// Simulated ping endpoint for specific southbound Merging Unit.
/// Triggers via hx-post on the "Ping MU" button and returns the updated card HTML.
async fn ping_mu(Path(id): Path<String>) -> Html<String> {
    // Generate simulated ping results based on current timestamp millisecond
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let (ip, mac, status, rate, dropped, rtt) = match id.as_str() {
        "MU-01" => (
            "192.168.1.101",
            "00:50:C2:88:99:A1",
            "Healthy",
            4000,
            0,
            Some(2 + (now_ms % 4) as u32), // 2 to 5 ms
        ),
        "MU-02" => (
            "192.168.1.102",
            "00:50:C2:88:99:A2",
            "Degraded",
            0,
            142,
            Some(15 + (now_ms % 13) as u32), // 15 to 27 ms
        ),
        _ => (
            "192.168.1.103",
            "00:50:C2:88:99:A3",
            "Disconnected",
            0,
            8563,
            None, // Offline, ping failed
        ),
    };

    let display_status = if rtt.is_none() {
        "Disconnected"
    } else {
        status
    };

    let card_markup = components::mu_card(&id, ip, mac, display_status, rate, dropped, rtt);

    Html(card_markup.into_string())
}
