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
                    span class="card-icon" {
                        svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                            path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" {}
                        }
                    }
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
    let icon_markup = match layer.to_lowercase().as_str() {
        "l1" => html! {
            svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" {}
            }
        },
        "l2" => html! {
            svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                path stroke-linecap="round" stroke-linejoin="round" d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z" {}
            }
        },
        "l3" => html! {
            svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                path stroke-linecap="round" stroke-linejoin="round" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4m0 5c0 2.21-3.582 4-8 4s-8-1.79-8-4" {}
            }
        },
        _ => html! {
            svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" {}
            }
        },
    };

    html! {
        div class="glass-card nb-card" id=(format!("nb-card-{}", layer.to_lowercase())) {
            div class="card-header flex justify-between items-center" {
                div class="flex items-center gap-2" {
                    span class="card-icon" { (icon_markup) }
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
