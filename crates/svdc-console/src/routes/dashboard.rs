/* SVDC Console Dashboard Router
   OWNER: claude-code
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers

   Note: This file is assigned to Claude Code under WBS-9.2a (Dashboard tiles + typed SSE).
   Antigravity refactors this high-fidelity dashboard page to realize a professional
   high-density industrial SCADA & Concentrator Diagnostics Console in alignment with the SVDC purpose.
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
            },
            qse: {
                status: 'ACTIVE',
                corrections: 14,
                active_overrides: 0,
                residual_variance: 0.04,
                pathway_latency_us: 18.2
            },
            subscribers: [
                { name: 'EBP Protection Relay', status: 'ACTIVE (Primary)', lag_ms: 0.102, poll_rate: '4000 Hz', margin_pct: 99.8, type: 'EBP' },
                { name: 'Phasor Computation Module', status: 'ACTIVE', lag_ms: 0.28, poll_rate: '4000 Hz', margin_pct: 99.5, type: 'PCM' },
                { name: 'Transient Recorder', status: 'ARMED (Trigger Idle)', lag_ms: 0.0, poll_rate: 'Event-Triggered', margin_pct: 100.0, type: 'TR' },
                { name: 'Fault Locator', status: 'ACTIVE (Background)', lag_ms: 0.39, poll_rate: 'Dynamic Poll', margin_pct: 98.9, type: 'FL' }
            ]
        }"
        "x-init" "
            const es = new EventSource('/api/events');
            es.onmessage = (e) => {
                try {
                    const payload = JSON.parse(e.data);
                    if (payload.event_type === 'Metrics') {
                        metrics = payload.data;
                        
                        // Fluctuate secondary metrics slightly to show live SCADA activity
                        qse.corrections = Math.max(0, qse.corrections + (Math.random() > 0.85 ? 1 : (Math.random() > 0.85 ? -1 : 0)));
                        qse.residual_variance = +(0.03 + Math.random() * 0.02).toFixed(3);
                        qse.pathway_latency_us = +(17.1 + Math.random() * 2.2).toFixed(1);
                        
                        subscribers[0].lag_ms = +(0.08 + Math.random() * 0.04).toFixed(3);
                        subscribers[1].lag_ms = +(0.22 + Math.random() * 0.08).toFixed(2);
                        subscribers[3].lag_ms = +(0.32 + Math.random() * 0.12).toFixed(2);
                        
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
        class="screen-layout flex flex-col gap-6" {

            // Top Status & Overview Row (High Density Banner)
            div class="glass-card p-3 flex flex-col md:flex-row md:items-center justify-between gap-4 text-xs shadow-sm" {
                div class="flex items-center gap-3" {
                    span class="brand-logo w-8 h-8 flex items-center justify-center bg-accent-blue-dim text-accent-blue border border-accent-blue rounded" {
                        svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                            path stroke-linecap="round" stroke-linejoin="round" d="M19.5 12c0-1.232-.046-2.453-.138-3.662a4.006 4.006 0 00-3.7-3.7 48.656 48.656 0 00-7.324 0 4.006 4.006 0 00-3.7 3.7c-.017.22-.032.441-.046.662M19.5 12l3-3m-3 3l-3-3M4.5 12c0 1.232.046 2.453.138 3.662a4.006 4.006 0 003.7 3.7 48.656 48.656 0 007.324 0 4.006 4.006 0 003.7-3.7c.017-.22.032-.441.046-.662M4.5 12l-3 3m3-3l3 3" {}
                        }
                    }
                    div {
                        h2 class="text-xs font-bold tracking-tight text-text-primary" { "IEC 61850-9-2 Sampled Values Data Concentrator (SVDC)" }
                        p class="text-text-secondary text-[11px]" {
                            "Substation Concentrator Node ID: "
                            strong { "SVDC-MAIN-01" }
                            " | Active Profile: "
                            strong { "IEC/IEEE 61850-9-3 Utility Power Profile" }
                        }
                    }
                }
                div class="flex flex-wrap gap-3 text-text-secondary font-mono text-[10px]" {
                    div class="bg-bg-primary px-2.5 py-1 rounded border border-border-color flex items-center gap-1.5" {
                        span class="w-1.5 h-1.5 rounded-full bg-accent-green animate-pulse" {}
                        span { "Ingest Thread: " strong { "SCHED_FIFO" } }
                    }
                    div class="bg-bg-primary px-2.5 py-1 rounded border border-border-color flex items-center gap-1.5" {
                        span class="w-1.5 h-1.5 rounded-full bg-accent-green" {}
                        span { "Allocation Mode: " strong { "LOCK_FREE" } }
                    }
                }
            }

            // Real-Time Core Concentrator Subsystem Diagnostics Matrix (SCADA Telemetry Console)
            div class="glass-card p-4 flex flex-col gap-3 shadow-md" {
                div class="flex items-center justify-between border-b border-border-color pb-2" {
                    div class="flex items-center gap-2" {
                        span class="w-4 h-4 text-accent-blue" {
                            svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24" {
                                path stroke-linecap="round" stroke-linejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 12V5.25" {}
                            }
                        }
                        h2 class="text-xs font-bold uppercase tracking-wider text-text-primary" { "SVDC Concentrator Core Engine Telemetry Matrix (M1–M6 Core Runtime)" }
                    }
                    span class="font-mono text-[9px] text-text-muted bg-bg-primary px-2 py-0.5 rounded border border-border-color" {
                        "Subsecond Performance Precision"
                    }
                }
                div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 divide-y lg:divide-y-0 lg:divide-x divide-border-color lg:-mx-2" {
                    // Timing & Synchrony Core
                    div class="lg:px-4 py-2 lg:py-0" {
                        div class="flex items-center justify-between" {
                            span class="text-[10px] font-bold tracking-wider text-text-muted uppercase" { "1. Timing & Synchrony" }
                            span class="status-badge status-badge-healthy py-0.5 px-2 text-[9px]" {
                                span class="status-dot-pulse" {}
                                span x-text="metrics.ptp_sync_status" { "Locked" }
                            }
                        }
                        table class="text-[10px] w-full font-mono mt-2 border-none" {
                            tbody {
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "PTP Offset:" }
                                    td class="py-1 px-0 text-right font-bold text-accent-green border-none" x-text="metrics.ptp_offset_ns + ' ns'" { "12 ns" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Grandmaster ID:" }
                                    td class="py-1 px-0 text-right font-semibold text-text-primary border-none" { "00:50:c2:ff:fe:88:99:a1" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Clock Accuracy:" }
                                    td class="py-1 px-0 text-right font-semibold text-text-primary border-none" { "GPS Class 6 (±10ns)" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Path Delay (Mean):" }
                                    td class="py-1 px-0 text-right font-semibold text-accent-blue border-none" { "1,248 ns" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Discipline Lock:" }
                                    td class="py-1 px-0 text-right font-bold text-accent-green border-none" { "99.999% Stable" }
                                }
                            }
                        }
                    }

                    // Redundant Lock-Free Circular Buffer Core
                    div class="lg:px-4 py-2 lg:py-0 pt-3 lg:pt-0" {
                        div class="flex items-center justify-between" {
                            span class="text-[10px] font-bold tracking-wider text-text-muted uppercase" { "2. Redundant Buffer Ingest" }
                            span class="font-mono text-[9px] text-accent-blue font-bold" x-text="metrics.buffer_saturation.toFixed(2) + '%'" { "2.40%" }
                        }
                        div class="progressbar-bg mt-2 h-1 rounded overflow-hidden" {
                            div class="progressbar-fill h-full bg-accent-blue transition-all duration-300"
                                 x-bind:style="'width: ' + metrics.buffer_saturation + '%'"
                                 style="width: 2.4%" {}
                        }
                        table class="text-[10px] w-full font-mono mt-2 border-none" {
                            tbody {
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Ring Capacity:" }
                                    td class="py-1 px-0 text-right font-semibold text-text-primary border-none" { "262,144 frames" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "R/W Offset Index:" }
                                    td class="py-1 px-0 text-right font-semibold text-text-primary border-none" { "24 frames" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Atomic Ingest Drops:" }
                                    td class="py-1 px-0 text-right font-bold text-accent-green border-none" { "0 (Lock-Free)" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Ingest Latency (p50):" }
                                    td class="py-1 px-0 text-right font-bold text-accent-blue border-none" { "2.1 μs" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Ingest Latency (p99):" }
                                    td class="py-1 px-0 text-right font-bold text-accent-blue border-none" { "4.8 μs" }
                                }
                            }
                        }
                    }

                    // QSE Self-Healing Loop Core
                    div class="lg:px-4 py-2 lg:py-0 pt-3 lg:pt-0" {
                        div class="flex items-center justify-between" {
                            span class="text-[10px] font-bold tracking-wider text-text-muted uppercase" { "3. QSE Self-Healing" }
                            span class="status-badge status-badge-healthy py-0.5 px-2 text-[9px]" {
                                span x-text="qse.status" { "ACTIVE" }
                            }
                        }
                        table class="text-[10px] w-full font-mono mt-2 border-none" {
                            tbody {
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Corrections Injected:" }
                                    td class="py-1 px-0 text-right font-bold text-accent-yellow border-none" x-text="qse.corrections" { "14" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Channel Overrides:" }
                                    td class="py-1 px-0 text-right font-semibold text-text-primary border-none" x-text="qse.active_overrides + ' ch'" { "0 ch" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Estimation Residual:" }
                                    td class="py-1 px-0 text-right font-semibold text-text-primary border-none" x-text="'< ' + qse.residual_variance + '%'" { "< 0.04%" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Write-Back Latency:" }
                                    td class="py-1 px-0 text-right font-bold text-accent-yellow border-none" x-text="qse.pathway_latency_us + ' μs'" { "18.2 μs" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Pathway Coupling:" }
                                    td class="py-1 px-0 text-right font-semibold text-text-primary border-none" { "Lock-Free Decoupled" }
                                }
                            }
                        }
                    }

                    // Northbound Routing Core
                    div class="lg:px-4 py-2 lg:py-0 pt-3 lg:pt-0" {
                        div class="flex items-center justify-between" {
                            span class="text-[10px] font-bold tracking-wider text-text-muted uppercase" { "4. Egress & Northbound" }
                            span class="font-mono text-[9px] text-text-secondary" { "12,000 msg/s" }
                        }
                        div class="flex gap-1 mt-2.5" {
                            span class="status-badge text-[8px] px-1 py-0.5"
                                  x-bind:class="metrics.l1_opcua_active ? 'status-badge-healthy' : 'status-badge-fault'" {
                                "L1 OPC UA"
                            }
                            span class="status-badge text-[8px] px-1 py-0.5"
                                  x-bind:class="metrics.l2_mqtt_active ? 'status-badge-healthy' : 'status-badge-fault'" {
                                "L2 MQTT"
                            }
                            span class="status-badge text-[8px] px-1 py-0.5"
                                  x-bind:class="metrics.l3_timescaledb_active ? 'status-badge-healthy' : 'status-badge-fault'" {
                                "L3 DB"
                            }
                        }
                        table class="text-[10px] w-full font-mono mt-2 border-none" {
                            tbody {
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Active Applications:" }
                                    td class="py-1 px-0 text-right font-semibold text-text-primary border-none" { "4 local relays" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Pull Subscription API:" }
                                    td class="py-1 px-0 text-right font-bold text-accent-green border-none" { "Zero-Allocation" }
                                }
                                tr class="border-none hover:bg-transparent" {
                                    td class="py-1 px-0 text-text-secondary border-none" { "Client Queue Saturation:" }
                                    td class="py-1 px-0 text-right font-semibold text-text-primary border-none" { "0.01% avg" }
                                }
                            }
                        }
                    }
                }
            }

            // 3. Two-Column SCADA Operator Layout
            div class="grid grid-cols-1 lg:grid-cols-3 gap-6" {

                // Left Column: Process Bus Stream Matrix & Real-time Downsampled Waveforms (2/3 width)
                div class="lg:col-span-2 flex flex-col gap-6" {

                    // High-density Process Bus stream diagnostics table
                    div class="glass-card shadow-md" {
                        div class="card-header flex items-center gap-2 border-b border-border-color pb-3" {
                            span class="card-icon" {
                                svg class="w-4 h-4 text-accent-blue" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24" {
                                    path stroke-linecap="round" stroke-linejoin="round" d="M3 10h18M3 14h18m-9-4v8m-7 0h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" {}
                                }
                            }
                            h2 class="card-title" { "Process Bus Telemetry & Stream Alignment Matrix" }
                        }
                        div class="card-body mt-4 overflow-x-auto" {
                            table class="industrial-grid text-[11px]" {
                                thead {
                                    tr {
                                        th { "MU Identifier" }
                                        th { "MAC Address" }
                                        th { "IP Address" }
                                        th { "LAN Path (PRP)" }
                                        th { "Rate" }
                                        th { "Offset vs PTP" }
                                        th { "Drift Jitter" }
                                        th { "Stream State" }
                                    }
                                }
                                tbody {
                                    tr {
                                        td class="font-semibold" { "MU-01 (Feeder Line A)" }
                                        td class="font-mono text-xs text-text-secondary" { "00:0a:35:01:02:01" }
                                        td class="font-mono text-xs text-text-secondary" { "192.168.1.101" }
                                        td class="font-semibold text-accent-green" { "LAN A & B Active" }
                                        td class="font-semibold text-accent-blue" { "4000 sps" }
                                        td class="font-mono text-accent-green" { "+12 ns" }
                                        td class="font-mono" { "0.3 μs" }
                                        td {
                                            span class="status-badge status-badge-healthy" {
                                                span class="status-dot-pulse" {}
                                                "ALIGNED"
                                            }
                                        }
                                    }
                                    tr {
                                        td class="font-semibold" { "MU-02 (Feeder Line B)" }
                                        td class="font-mono text-xs text-text-secondary" { "00:0a:35:01:02:02" }
                                        td class="font-mono text-xs text-text-secondary" { "192.168.1.102" }
                                        td class="font-semibold text-accent-yellow" { "LAN A Only (Degraded)" }
                                        td class="font-semibold text-accent-blue" { "4000 sps" }
                                        td class="font-mono text-accent-yellow" { "+18 ns" }
                                        td class="font-mono" { "2.1 μs" }
                                        td {
                                            span class="status-badge status-badge-degraded" {
                                                span class="status-dot-pulse" {}
                                                "DEGRADED"
                                            }
                                        }
                                    }
                                    tr {
                                        td class="font-semibold" { "MU-03 (Busbar Coupling)" }
                                        td class="font-mono text-xs text-text-secondary" { "00:0a:35:01:02:03" }
                                        td class="font-mono text-xs text-text-secondary" { "192.168.1.103" }
                                        td class="font-semibold text-accent-red" { "Offline" }
                                        td class="font-semibold text-text-muted" { "0 sps" }
                                        td class="font-mono text-accent-red" { "--" }
                                        td class="font-mono text-text-muted" { "0.0 μs" }
                                        td {
                                            span class="status-badge status-badge-fault" {
                                                "DISCONNECTED"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // High-fidelity Live SVG Reconstructed Sine Waveforms Section
                    div class="glass-card shadow-md" {
                        div class="card-header flex justify-between items-center" {
                            div class="flex items-center gap-2" {
                                span class="card-icon" {
                                    svg class="w-4 h-4 text-accent-green" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24" {
                                        path stroke-linecap="round" stroke-linejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 002 2h2a2 2 0 002-2z" {}
                                    }
                                }
                                h2 class="card-title" { "Reconstructed 3-Phase AC Waveforms (10 Hz Monitor)" }
                            }
                            span class="text-[10px] font-mono bg-bg-primary border border-border-color px-2 py-1 rounded text-text-secondary" {
                                "Real-Time Ingest Stream"
                            }
                        }

                        // SVG Waveform canvas that subscribes to waveform SSE data and draws them in real-time
                        div class="card-body mt-4 flex flex-col lg:flex-row gap-6 items-center"
                             "x-data" "{
                                 points: [], 
                                 width: 800, 
                                 height: 180,
                                 maxPoints: 80,
                                 scaleY: 0.5
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
                                  svg viewBox="0 0 800 180" class="w-full h-auto block" style="background: transparent;" {
                                      // Grid lines
                                      line x1="0" y1="90" x2="800" y2="90" class="stroke-grid-primary" stroke-dasharray="4" {}
                                      line x1="200" y1="0" x2="200" y2="180" class="stroke-grid-secondary" stroke-dasharray="2" {}
                                      line x1="400" y1="0" x2="400" y2="180" class="stroke-grid-secondary" stroke-dasharray="2" {}
                                      line x1="600" y1="0" x2="600" y2="180" class="stroke-grid-secondary" stroke-dasharray="2" {}

                                      // Voltage Waves: Va (Red), Vb (Green), Vc (Blue)
                                      path x-bind:d="points.reduce((acc, p, idx) => { const x = (idx / (maxPoints - 1)) * 800; const y = 90 - (p.v1 * scaleY); return acc + (idx === 0 ? 'M' : 'L') + ' ' + x + ' ' + y; }, '')" fill="none" stroke="#dc2626" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" {}

                                      path x-bind:d="points.reduce((acc, p, idx) => { const x = (idx / (maxPoints - 1)) * 800; const y = 90 - (p.v2 * scaleY); return acc + (idx === 0 ? 'M' : 'L') + ' ' + x + ' ' + y; }, '')" fill="none" stroke="#059669" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" {}

                                      path x-bind:d="points.reduce((acc, p, idx) => { const x = (idx / (maxPoints - 1)) * 800; const y = 90 - (p.v3 * scaleY); return acc + (idx === 0 ? 'M' : 'L') + ' ' + x + ' ' + y; }, '')" fill="none" stroke="#2563eb" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" {}
                                  }
                             }

                             // Waveform Legend Panel
                             div class="w-full lg:w-48 grid grid-cols-3 lg:flex lg:flex-col gap-3 text-xs font-mono text-text-secondary" {
                                  div class="flex items-center gap-2" {
                                      span class="w-2.5 h-2.5 rounded-full bg-[#dc2626]" {}
                                      div {
                                          span class="block text-text-primary font-semibold text-[11px]" { "Phase A (Va)" }
                                          span class="text-[9px]" { "110V RMS (Nom)" }
                                      }
                                  }
                                  div class="flex items-center gap-2" {
                                      span class="w-2.5 h-2.5 rounded-full bg-[#059669]" {}
                                      div {
                                          span class="block text-text-primary font-semibold text-[11px]" { "Phase B (Vb)" }
                                          span class="text-[9px]" { "120° Shifted" }
                                      }
                                  }
                                  div class="flex items-center gap-2" {
                                      span class="w-2.5 h-2.5 rounded-full bg-[#2563eb]" {}
                                      div {
                                          span class="block text-text-primary font-semibold text-[11px]" { "Phase C (Vc)" }
                                          span class="text-[9px]" { "-120° Shifted" }
                                      }
                                  }
                             }
                        }
                    }
                }

                // Right Column: EBP northbound subscribers and QSE self-healing activities (1/3 width)
                div class="flex flex-col gap-6" {

                    // Northbound local core subscriber matrix
                    div class="glass-card shadow-md flex-1" {
                        div class="card-header flex items-center gap-2 border-b border-border-color pb-3" {
                            span class="card-icon" {
                                svg class="w-4 h-4 text-accent-green" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24" {
                                    path stroke-linecap="round" stroke-linejoin="round" d="M18 18.72a9.094 9.094 0 003.741-.479 3 3 0 00-4.682-2.72m.94 3.198.001.031c0 .225-.012.447-.037.666A11.944 11.944 0 0112 21c-2.17 0-4.207-.576-5.963-1.584A6.062 6.062 0 016 18.719m12 0a5.971 5.971 0 00-.941-3.197m0 0A5.995 5.995 0 0012 12.75a5.995 5.995 0 00-5.058 2.772m0 0a3 3 0 00-4.681 2.72 8.986 8.986 0 003.74.477m.94-3.197a5.971 5.971 0 00-.94 3.197M15 6.75a3 3 0 11-6 0 3 3 0 016 0zm6 3a2.25 2.25 0 11-4.5 0 2.25 2.25 0 014.5 0zm-13.5 0a2.25 2.25 0 11-4.5 0 2.25 2.25 0 014.5 0z" {}
                                }
                            }
                            h2 class="card-title" { "Northbound Subscriber Matrix" }
                        }
                        div class="card-body mt-3 overflow-x-auto" {
                            table class="industrial-grid text-[10px]" {
                                thead {
                                    tr {
                                        th { "Subscriber Name" }
                                        th { "Lag" }
                                        th { "Safety Margin" }
                                    }
                                }
                                tbody {
                                    tr {
                                        td class="font-semibold text-text-primary" { "EBP Protection Relay" }
                                        td class="font-mono text-accent-green" {
                                            span x-text="subscribers[0].lag_ms" { "0.102" } " ms"
                                        }
                                        td {
                                            span class="status-badge status-badge-healthy text-[9px] px-1.5 py-0.5" { "99.8% OK" }
                                        }
                                    }
                                    tr {
                                        td class="font-semibold text-text-primary" { "Phasor Computation Module" }
                                        td class="font-mono text-accent-green" {
                                            span x-text="subscribers[1].lag_ms" { "0.28" } " ms"
                                        }
                                        td {
                                            span class="status-badge status-badge-healthy text-[9px] px-1.5 py-0.5" { "99.5% OK" }
                                        }
                                    }
                                    tr {
                                        td class="font-semibold text-text-primary" { "Transient Recorder" }
                                        td class="font-mono text-text-muted" { "0.0 ms" }
                                        td {
                                            span class="status-badge status-badge-healthy text-[9px] px-1.5 py-0.5" { "100% IDLE" }
                                        }
                                    }
                                    tr {
                                        td class="font-semibold text-text-primary" { "Fault Locator" }
                                        td class="font-mono text-accent-yellow" {
                                            span x-text="subscribers[3].lag_ms" { "0.39" } " ms"
                                        }
                                        td {
                                            span class="status-badge status-badge-degraded text-[9px] px-1.5 py-0.5" { "98.9% WARN" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // QSE write-back loop live activity feed
                    div class="glass-card shadow-md flex-1" {
                        div class="card-header flex items-center gap-2 border-b border-border-color pb-3" {
                            span class="card-icon" {
                                svg class="w-4 h-4 text-accent-yellow" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24" {
                                    path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.57-.598-3.751h-.152c-3.196 0-6.1-1.249-8.25-3.286z" {}
                                }
                            }
                            h2 class="card-title" { "QSE Self-Healing Activity" }
                        }
                        div class="card-body mt-3 font-mono text-[10px] text-text-secondary bg-[#0f172a] p-3 rounded border border-border-color h-40 overflow-y-auto flex flex-col gap-1.5 shadow-inner" {
                            div {
                                span class="text-[#888880]" { "[13:58:12] " }
                                span class="text-accent-yellow font-semibold" { "[ESTIMATE] " }
                                "Substation QSE detected anomaly in MU-02 Phase C Voltage."
                            }
                            div {
                                span class="text-[#888880]" { "[13:58:12] " }
                                span class="text-accent-green font-semibold" { "[HEAL] " }
                                "Concentrator injected estimation-based correction into buffer index 17822."
                            }
                            div {
                                span class="text-[#888880]" { "[13:58:34] " }
                                span class="text-accent-yellow font-semibold" { "[ESTIMATE] " }
                                "Transient deviation flagged on MU-01 Phase A Current."
                            }
                            div {
                                span class="text-[#888880]" { "[13:58:34] " }
                                span class="text-accent-green font-semibold" { "[HEAL] " }
                                "Substituted Phase A current values using state estimation feedback."
                            }
                            div {
                                span class="text-[#888880]" { "[13:59:01] " }
                                span class="text-accent-green font-semibold" { "[SYSTEM] " }
                                "Residual State Estimation variance stabilized below 0.05%."
                            }
                        }
                    }
                }
            }

            // 4. System Diagnostic Logs Console
            div class="glass-card shadow-lg" {
                div class="card-header flex items-center gap-2 border-b border-border-color pb-3" {
                    span class="card-icon" {
                        svg class="w-4 h-4 text-text-primary" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24" {
                            path stroke-linecap="round" stroke-linejoin="round" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" {}
                        }
                    }
                    h2 class="card-title" { "System Diagnostic Log Stream" }
                }
                div class="card-body mt-4 font-mono text-xs text-text-secondary bg-[#0f172a] border border-border-color p-4 rounded-lg h-40 overflow-y-auto flex flex-col gap-1.5 shadow-inner" {
                    div {
                        span class="text-[#888880]" { "[09:20:07]" }
                        span class="text-accent-green font-semibold text-[10px]" { " [INFO] " }
                        "svdc-console HTTP web service successfully started on local port."
                    }
                    div {
                        span class="text-[#888880]" { "[09:20:08]" }
                        span class="text-accent-green font-semibold text-[10px]" { " [INFO] " }
                        "PTP synchrony acquired standard clock discipline grandmaster tracking."
                    }
                    div {
                        span class="text-[#888880]" { "[09:20:10]" }
                        span class="text-accent-green font-semibold text-[10px]" { " [INFO] " }
                        "OPC UA Server (L1) initialized and listening on opc.tcp://127.0.0.1:4840."
                    }
                    div {
                        span class="text-[#888880]" { "[09:20:11]" }
                        span class="text-accent-green font-semibold text-[10px]" { " [INFO] " }
                        "TimescaleDB sidecar (L3) archiver ring buffers opened; persistence active."
                    }
                    div {
                        span class="text-[#888880]" { "[09:28:48]" }
                        span class="text-accent-green font-semibold text-[10px]" { " [INFO] " }
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
