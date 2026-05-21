/* SVDC Console Dashboard Router
   OWNER: claude-code
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers

   Note: This file is assigned to Claude Code under WBS-9.2a (Dashboard tiles + typed SSE).
   Antigravity scaffolds this high-fidelity dashboard page to establish the end-to-end
   live SSE telemetry stream wireframe.
*/

use axum::{
    response::{Html, Sse},
    routing::get,
    Router,
};
use futures_util::stream::Stream;
use maud::html;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::sse::emitter;
use crate::templates::base;

/// Register dashboard and telemetry stream routes
pub fn register(router: Router) -> Router {
    router
        .route("/", get(dashboard_page))
        .route("/api/events", get(events_handler))
}

/// Renders the main high-density Dashboard page
async fn dashboard_page() -> Html<String> {
    let content = html! {
        // AlpineJS reactive telemetry data-binding wrapper
        div "x-data" "{
            metrics: { 
                ptp_sync_status: 'Locked', 
                ptp_offset_ns: 12, 
                buffer_saturation: 2.4, 
                active_mus: 3, 
                sps_rate: 4000, 
                l1_opcua_active: true, 
                l2_mqtt_active: false, 
                l3_timescaledb_active: true 
            } 
        }"
        "x-init" "
            const es = new EventSource('/api/events');
            es.onmessage = (e) => {
                try {
                    const payload = JSON.parse(e.data);
                    if (payload.event_type === 'Metrics') {
                        metrics = payload.data;
                        
                        // Also update topbar PTP lock status reactively
                        const topBar = document.getElementById('topbar-ptp-status');
                        if (topBar) {
                            topBar.textContent = metrics.ptp_sync_status + ' (' + metrics.ptp_offset_ns + ' ns)';
                        }
                    }
                } catch(err) {
                    console.error('Failed to parse SSE event:', err);
                }
            };
        "
        class="flex flex-col gap-6" {

            // 1. High Density Telemetry Tiles Row
            div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6" {

                // Tile A: PTP Clock
                div class="glass-card telemetry-tile flex flex-col justify-between" {
                    div class="tile-header flex justify-between items-center" {
                        span class="tile-label" { "Grandmaster Sync" }
                        span class="tile-icon" {
                            svg class="w-4 h-4 text-accent-green" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" {}
                            }
                        }
                    }
                    div class="tile-body mt-2" {
                        div class="flex items-baseline gap-2" {
                            span class="tile-value text-accent-green" "x-text" "metrics.ptp_offset_ns + ' ns'" { "12 ns" }
                        }
                        div class="tile-subtext mt-1 text-xs text-text-secondary" {
                            "Sync status: "
                            strong "x-text" "metrics.ptp_sync_status" class="text-accent-green" { "Locked" }
                        }
                    }
                }

                // Tile B: Buffer Saturation
                div class="glass-card telemetry-tile flex flex-col justify-between" {
                    div class="tile-header flex justify-between items-center" {
                        span class="tile-label" { "Circular Buffer" }
                        span class="tile-icon" {
                            svg class="w-4 h-4 text-accent-blue" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" {}
                            }
                        }
                    }
                    div class="tile-body mt-2" {
                        div class="flex items-baseline gap-2" {
                            span class="tile-value text-accent-blue" "x-text" "metrics.buffer_saturation.toFixed(2) + '%'" { "2.40%" }
                        }
                        // Saturation progress bar
                        div class="progressbar-bg mt-2 h-1.5 rounded bg-border-color overflow-hidden" {
                            div class="progressbar-fill h-full bg-accent-blue transition-all duration-300"
                                ":style" "'width: ' + metrics.buffer_saturation + '%'"
                                style="width: 2.4%" {}
                        }
                    }
                }

                // Tile C: Active Southbound MUs
                div class="glass-card telemetry-tile flex flex-col justify-between" {
                    div class="tile-header flex justify-between items-center" {
                        span class="tile-label" { "Merging Units" }
                        span class="tile-icon" {
                            svg class="w-4 h-4 text-text-secondary" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" {}
                            }
                        }
                    }
                    div class="tile-body mt-2" {
                        div class="flex items-baseline gap-2" {
                            span class="tile-value text-text-primary" "x-text" "metrics.active_mus" { "3" }
                            span class="text-sm text-text-secondary" { "connected" }
                        }
                        div class="tile-subtext mt-1 text-xs text-text-secondary" {
                            "Total sampling: "
                            strong "x-text" "metrics.sps_rate + ' sps'" class="text-text-primary" { "4000 sps" }
                        }
                    }
                }

                // Tile D: Northbound Routing Interfaces
                div class="glass-card telemetry-tile flex flex-col justify-between" {
                    div class="tile-header flex justify-between items-center" {
                        span class="tile-label" { "Active Adapters" }
                        span class="tile-icon" {
                            svg class="w-4 h-4 text-text-secondary" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" {}
                            }
                        }
                    }
                    div class="tile-body mt-2" {
                        div class="flex gap-2" {
                            // Badge L1
                            span class="status-badge"
                                  ":class" "metrics.l1_opcua_active ? 'status-badge-healthy' : 'status-badge-fault'" {
                                "L1"
                            }
                            // Badge L2
                            span class="status-badge"
                                  ":class" "metrics.l2_mqtt_active ? 'status-badge-healthy' : 'status-badge-fault'" {
                                "L2"
                            }
                            // Badge L3
                            span class="status-badge"
                                  ":class" "metrics.l3_timescaledb_active ? 'status-badge-healthy' : 'status-badge-fault'" {
                                "L3"
                            }
                        }
                        div class="tile-subtext mt-3 text-xs text-text-secondary" {
                            "L0 In-process: "
                            strong class="text-accent-green" { "Always On" }
                        }
                    }
                }
            }

            // 2. High-fidelity Live SVG Reconstructed Sine Waveforms Section
            div class="glass-card shadow-lg" {
                div class="card-header flex justify-between items-center" {
                    div class="flex items-center gap-2" {
                        span class="card-icon" {
                            svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                path stroke-linecap="round" stroke-linejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 002 2h2a2 2 0 002-2z" {}
                            }
                        }
                        h2 class="card-title" { "Reconstructed 3-Phase AC Waveforms (10 Hz Monitor)" }
                    }
                    span class="text-xs font-mono bg-bg-secondary border border-border-color px-2 py-1 rounded text-text-secondary" {
                        "Streaming via SSE EventSource"
                    }
                }

                // SVG Waveform canvas that subscribes to waveform SSE data and draws them in real-time
                div class="card-body mt-4 flex flex-col lg:flex-row gap-6 items-center"
                     "x-data" "{
                         points: [], 
                         width: 800, 
                         height: 250,
                         maxPoints: 80,
                         scaleY: 0.8
                     }"
                     "x-init" "
                         // Listen to high-speed 10 Hz waveform packets
                         es.addEventListener('message', (e) => {
                             try {
                                 const payload = JSON.parse(e.data);
                                 if (payload.event_type === 'Waveform') {
                                     points.push(payload.data);
                                     if (points.length > maxPoints) {
                                         points.shift();
                                     }
                                 }
                             } catch(err) {}
                         });
                     " {

                     // SVG Grid Waveform Display
                     div class="flex-1 w-full bg-chart-bg rounded-lg border border-border-color p-2 relative overflow-hidden" {

                          // Responsive inline SVG
                          svg viewBox="0 0 800 250" class="w-full h-auto block" style="background: transparent;" {
                              // Grid lines
                              line x1="0" y1="125" x2="800" y2="125" class="stroke-grid-primary" stroke-dasharray="4" {}
                              line x1="200" y1="0" x2="200" y2="250" class="stroke-grid-secondary" stroke-dasharray="2" {}
                              line x1="400" y1="0" x2="400" y2="250" class="stroke-grid-secondary" stroke-dasharray="2" {}
                              line x1="600" y1="0" x2="600" y2="250" class="stroke-grid-secondary" stroke-dasharray="2" {}

                              // Voltage Waves: Va (Red), Vb (Green), Vc (Blue)
                              path ":d"="points.reduce((acc, p, idx) => { const x = (idx / (maxPoints - 1)) * 800; const y = 125 - (p.v1 * scaleY); return acc + (idx === 0 ? 'M' : 'L') + ' ' + x + ' ' + y; }, '')" fill="none" stroke="#dc2626" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" {}

                              path ":d"="points.reduce((acc, p, idx) => { const x = (idx / (maxPoints - 1)) * 800; const y = 125 - (p.v2 * scaleY); return acc + (idx === 0 ? 'M' : 'L') + ' ' + x + ' ' + y; }, '')" fill="none" stroke="#16a34a" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" {}

                              path ":d"="points.reduce((acc, p, idx) => { const x = (idx / (maxPoints - 1)) * 800; const y = 125 - (p.v3 * scaleY); return acc + (idx === 0 ? 'M' : 'L') + ' ' + x + ' ' + y; }, '')" fill="none" stroke="#2563eb" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" {}
                          }
                     }

                     // Waveform Legend Panel
                     div class="w-full lg:w-64 grid grid-cols-3 lg:flex lg:flex-col gap-4 text-xs font-mono text-text-secondary" {
                          div class="flex items-center gap-2" {
                              span class="w-3 h-3 rounded-full bg-[#dc2626]" {}
                              div {
                                  span class="block text-text-primary font-semibold" { "Phase A (Va)" }
                                  span class="text-[10px]" { "110V RMS (Nominal)" }
                              }
                          }
                          div class="flex items-center gap-2" {
                              span class="w-3 h-3 rounded-full bg-[#16a34a]" {}
                              div {
                                  span class="block text-text-primary font-semibold" { "Phase B (Vb)" }
                                  span class="text-[10px]" { "120° Phase shift" }
                              }
                          }
                          div class="flex items-center gap-2" {
                              span class="w-3 h-3 rounded-full bg-[#2563eb]" {}
                              div {
                                  span class="block text-text-primary font-semibold" { "Phase C (Vc)" }
                                  span class="text-[10px]" { "-120° Phase shift" }
                              }
                          }
                     }
                }
            }

            // 3. System Diagnostic Logs Console
            div class="glass-card shadow-lg" {
                div class="card-header flex items-center gap-2 border-b border-border-color pb-3" {
                    span class="card-icon" {
                        svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                            path stroke-linecap="round" stroke-linejoin="round" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" {}
                        }
                    }
                    h2 class="card-title" { "System Diagnostic Log Stream" }
                }
                div class="card-body mt-4 font-mono text-xs text-text-secondary bg-[#0f172a] border border-border-color p-4 rounded-lg h-48 overflow-y-auto flex flex-col gap-1.5" {
                    div {
                        span class="text-[#888880]" { "[09:20:07]" }
                        span class="text-accent-green font-semibold" { " [INFO] " }
                        "svdc-console HTTP web service successfully started on local port."
                    }
                    div {
                        span class="text-[#888880]" { "[09:20:08]" }
                        span class="text-accent-green font-semibold" { " [INFO] " }
                        "PTP synchrony acquired standard clock discipline grandmaster tracking."
                    }
                    div {
                        span class="text-[#888880]" { "[09:20:10]" }
                        span class="text-accent-green font-semibold" { " [INFO] " }
                        "OPC UA Server (L1) initialized and listening on opc.tcp://127.0.0.1:4840."
                    }
                    div {
                        span class="text-[#888880]" { "[09:20:11]" }
                        span class="text-accent-green font-semibold" { " [INFO] " }
                        "TimescaleDB sidecar (L3) archiver ring buffers opened; persistence active."
                    }
                    div {
                        span class="text-[#888880]" { "[09:28:48]" }
                        span class="text-accent-green font-semibold" { " [INFO] " }
                        "Southbound Ingest Merging Unit stream discovered MU-01 at 4000 sps."
                    }
                }
            }
        }
    };

    let rendered = base::layout("Node Diagnostics Dashboard", "dashboard", content);
    Html(rendered.into_string())
}

/// Axum Server-Sent Events (SSE) telemetry stream emitter endpoint.
/// Forwards events in real-time from the global emitter broadcast channel.
async fn events_handler() -> Sse<impl Stream<Item = Result<axum::response::sse::Event, Infallible>>>
{
    let rx = emitter::subscribe();

    let stream = BroadcastStream::new(rx)
        .filter_map(|res| res.ok())
        .map(|json_str| Ok(axum::response::sse::Event::default().data(json_str)));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
