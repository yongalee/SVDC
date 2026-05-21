/* SVDC Console Maud UI Components
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use maud::{html, Markup};

/// Render a status badge with appropriate styling based on the status string
pub fn status_badge(status: &str) -> Markup {
    let status_class = match status.to_lowercase().as_str() {
        "healthy" | "locked" | "active" | "connected" => "status-badge-healthy",
        "degraded" | "holdover" | "warning" | "standby" => "status-badge-degraded",
        "disconnected" | "fault" | "inactive" | "error" => "status-badge-fault",
        _ => "status-badge-unknown",
    };

    html! {
        span class=(format!("status-badge {}", status_class)) {
            span class="status-dot-pulse" {}
            (status)
        }
    }
}

/// Render a single southbound Merging Unit card (WBS-9.3b)
pub fn mu_card(
    id: &str,
    ip: &str,
    mac: &str,
    status: &str,
    rate_hz: u32,
    dropped_packets: u32,
    rtt_ms: Option<u32>,
) -> Markup {
    html! {
        div class="glass-card mu-card" id=(format!("mu-card-{}", id)) {
            div class="card-header flex justify-between items-center" {
                div class="flex items-center gap-2" {
                    span class="card-icon" { "🔌" }
                    h3 class="card-title" { (id) }
                }
                (status_badge(status))
            }

            div class="card-body grid grid-cols-2 gap-y-2 gap-x-4 text-sm mt-3" {
                div class="metric-group" {
                    span class="metric-label" { "IP Address" }
                    span class="metric-value font-mono" { (ip) }
                }
                div class="metric-group" {
                    span class="metric-label" { "MAC Address" }
                    span class="metric-value font-mono text-xs" { (mac) }
                }
                div class="metric-group" {
                    span class="metric-label" { "Sample Rate" }
                    span class="metric-value font-semibold text-accent-blue" { (rate_hz) " sps" }
                }
                div class="metric-group" {
                    span class="metric-label" { "Dropped Frames" }
                    span class=(format!("metric-value font-semibold {}", if dropped_packets > 0 { "text-accent-red" } else { "text-text-primary" })) {
                        (dropped_packets)
                    }
                }
                div class="metric-group col-span-2 border-t border-border-color pt-2 mt-1 flex justify-between items-center" {
                    div class="flex flex-col" {
                        span class="metric-label" { "Layer-2 Ping Latency" }
                        span class="metric-value font-mono text-accent-green" {
                            @if let Some(rtt) = rtt_ms {
                                (rtt) " ms"
                            } @else {
                                "--"
                            }
                        }
                    }
                    button hx-post=(format!("/api/v1/merging-units/{}/ping", id))
                            hx-target=(format!("#mu-card-{}", id))
                            hx-swap="outerHTML"
                            class="btn-primary flex items-center gap-1" {
                        span class="btn-spinner" {}
                        "Ping MU"
                    }
                }
            }
        }
    }
}

/// Render a single northbound Adapter card (WBS-9.4b)
pub fn northbound_card(
    layer: &str,
    name: &str,
    status: &str,
    endpoint: &str,
    consumers: usize,
    throughput_fps: u32,
    enabled: bool,
) -> Markup {
    let icon = match layer.to_lowercase().as_str() {
        "l1" => "🛡️",
        "l2" => "☁️",
        "l3" => "💾",
        _ => "🚀",
    };

    html! {
        div class="glass-card nb-card" id=(format!("nb-card-{}", layer.to_lowercase())) {
            div class="card-header flex justify-between items-center" {
                div class="flex items-center gap-2" {
                    span class="card-icon" { (icon) }
                    div {
                        span class="text-xs font-semibold text-text-secondary uppercase tracking-wider block" { (layer) }
                        h3 class="card-title" { (name) }
                    }
                }
                (status_badge(status))
            }

            div class="card-body grid grid-cols-2 gap-y-2 gap-x-4 text-sm mt-3" {
                div class="metric-group col-span-2" {
                    span class="metric-label" { "Endpoint Destination" }
                    span class="metric-value font-mono text-xs overflow-x-auto block" { (endpoint) }
                }
                div class="metric-group" {
                    span class="metric-label" { "Active Receivers" }
                    span class="metric-value font-semibold text-text-primary" { (consumers) }
                }
                div class="metric-group" {
                    span class="metric-label" { "Throughput" }
                    span class="metric-value font-semibold text-accent-blue" { (throughput_fps) " fps" }
                }

                div class="metric-group col-span-2 border-t border-border-color pt-3 mt-2 flex justify-between items-center" {
                    span class="metric-label font-medium" { "Adapter Status Action" }
                    label class="switch-container" {
                        input type="checkbox"
                               checked?[enabled]
                               hx-post=(format!("/api/v1/northbound/{}/toggle", layer.to_lowercase()))
                               hx-target=(format!("#nb-card-{}", layer.to_lowercase()))
                               hx-swap="outerHTML"
                               class="switch-input";
                        span class="switch-slider" {}
                    }
                }
            }
        }
    }
}
