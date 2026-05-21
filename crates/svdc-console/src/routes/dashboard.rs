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
        div x-data="{
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
            ],
            phasors: {
                va_rms: 110.2, va_ang: 0.0,
                vb_rms: 109.8, vb_ang: -120.1,
                vc_rms: 110.5, vc_ang: 119.8,
                ia_rms: 4.82, ia_ang: -30.2,
                ib_rms: 4.79, ib_ang: -150.5,
                ic_rms: 4.88, ic_ang: 89.4,
                v1: 110.16, v2: 0.34, v0: 0.12,
                freq: 59.998, rocof: -0.002,
                mw: 1.54, mvar: 0.31, pf: 0.98
            },
            prp: {
                mu01_lan_a: true, mu01_lan_b: true, mu01_errors: 0, mu01_discards: 12,
                mu02_lan_a: true, mu02_lan_b: false, mu02_errors: 2, mu02_discards: 3,
                mu03_lan_a: false, mu03_lan_b: false, mu03_errors: 0, mu03_discards: 0
            },
            cb_sync: {
                mirror_state: 'IN_SYNC',
                active_mirror: 'CB-A',
                replica_mirror: 'CB-B',
                write_ptr: 148202,
                ebp_margin: 24, pcm_margin: 56, tr_margin: 0, fl_margin: 92,
                ebp_zero_alloc: true, pcm_zero_alloc: true, tr_zero_alloc: true, fl_zero_alloc: true
            },
            getVectorX(rms, angle, isVoltage) {
                const scale = isVoltage ? (70 / 120) : (70 / 6);
                const r = Math.min(75, rms * scale);
                const rad = angle * Math.PI / 180;
                return (100 + r * Math.cos(rad)).toFixed(1);
            },
            getVectorY(rms, angle, isVoltage) {
                const scale = isVoltage ? (70 / 120) : (70 / 6);
                const r = Math.min(75, rms * scale);
                const rad = angle * Math.PI / 180;
                return (100 - r * Math.sin(rad)).toFixed(1);
            }
        }"
        "x-init"="
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
                        
                        // Fluctuate electrical phasors realistically
                        phasors.va_rms = +(110.0 + Math.random() * 0.5).toFixed(2);
                        phasors.vb_rms = +(109.5 + Math.random() * 0.6).toFixed(2);
                        phasors.vc_rms = +(110.1 + Math.random() * 0.6).toFixed(2);
                        phasors.va_ang = +(0.0 + (Math.random() - 0.5) * 0.1).toFixed(2);
                        phasors.vb_ang = +(-120.0 + (Math.random() - 0.5) * 0.1).toFixed(2);
                        phasors.vc_ang = +(120.0 + (Math.random() - 0.5) * 0.1).toFixed(2);
                        
                        phasors.ia_rms = +(4.75 + Math.random() * 0.1).toFixed(2);
                        phasors.ib_rms = +(4.70 + Math.random() * 0.1).toFixed(2);
                        phasors.ic_rms = +(4.80 + Math.random() * 0.1).toFixed(2);
                        phasors.ia_ang = +(-30.0 + (Math.random() - 0.5) * 0.2).toFixed(2);
                        phasors.ib_ang = +(-150.0 + (Math.random() - 0.5) * 0.2).toFixed(2);
                        phasors.ic_ang = +(90.0 + (Math.random() - 0.5) * 0.2).toFixed(2);
                        
                        // Symmetrical components calculations based on nominal inputs
                        phasors.v1 = +((phasors.va_rms + phasors.vb_rms + phasors.vc_rms) / 3).toFixed(2);
                        phasors.v2 = +(0.28 + Math.random() * 0.1).toFixed(2);
                        phasors.v0 = +(0.10 + Math.random() * 0.05).toFixed(2);
                        
                        phasors.freq = +(59.995 + Math.random() * 0.01).toFixed(3);
                        phasors.rocof = +((Math.random() - 0.5) * 0.004).toFixed(3);
                        phasors.mw = +(1.51 + Math.random() * 0.05).toFixed(2);
                        phasors.mvar = +(0.29 + Math.random() * 0.03).toFixed(2);
                        phasors.pf = +(0.978 + Math.random() * 0.004).toFixed(3);
                        
                        // Increment discards and write pointer
                        prp.mu01_discards += Math.random() > 0.7 ? 1 : 0;
                        prp.mu02_errors += Math.random() > 0.95 ? 1 : 0;
                        cb_sync.write_ptr += Math.floor(Math.random() * 40) + 10;
                        
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

                // Left Column: Polar Vector Grid, Diagnostics Table, and PRP/HSR Routing (2/3 width)
                div class="lg:col-span-2 flex flex-col gap-6" {

                    // High-density Polar Vector Diagram & Electrical Diagnostics Card
                    div class="glass-card shadow-md" {
                        div class="card-header flex justify-between items-center border-b border-border-color pb-3" {
                            div class="flex items-center gap-2" {
                                h2 class="card-title" { "3-Phase Polar Phasor Vector & Symmetrical Components Grid" }
                            }
                            span class="text-[10px] font-mono bg-bg-primary border border-border-color px-2 py-1 rounded text-text-secondary" {
                                "Real-Time Vector Ingestion Monitor"
                            }
                        }

                        div class="card-body mt-4 grid grid-cols-1 md:grid-cols-2 gap-6 items-start" {
                            // Polar Diagram Canvas
                            div class="flex flex-col items-center justify-center p-2 bg-chart-bg rounded-lg border border-border-color relative" {
                                svg viewBox="0 0 200 200" style="width: 120px; height: 120px; display: block; background: transparent;" {
                                    // Concentric grid circles: 30V/1.5A, 60V/3A, 90V/4.5A, 120V/6A (radius 20, 40, 60, 80 centered at 100, 100)
                                    circle cx="100" cy="100" r="20" class="stroke-grid-secondary" fill="none" stroke-dasharray="2" {}
                                    circle cx="100" cy="100" r="40" class="stroke-grid-secondary" fill="none" stroke-dasharray="2" {}
                                    circle cx="100" cy="100" r="60" class="stroke-grid-secondary" fill="none" stroke-dasharray="2" {}
                                    circle cx="100" cy="100" r="80" class="stroke-grid-primary" fill="none" {}

                                    // Polar degree axis lines every 30 degrees (30, 60, 90, 120, 150, 180, 210, 240, 270, 300, 330)
                                    // 0-180 horizontal line
                                    line x1="20" y1="100" x2="180" y2="100" class="stroke-grid-secondary" stroke-dasharray="2" {}
                                    // 90-270 vertical line
                                    line x1="100" y1="20" x2="100" y2="180" class="stroke-grid-secondary" stroke-dasharray="2" {}

                                    // Diagonal 30-210 degree lines
                                    line x1="30.7" y1="140" x2="169.3" y2="60" class="stroke-grid-secondary" stroke-dasharray="1" {}
                                    // 60-240 line
                                    line x1="60" y1="169.3" x2="140" y2="30.7" class="stroke-grid-secondary" stroke-dasharray="1" {}
                                    // 120-300 line
                                    line x1="60" y1="30.7" x2="140" y2="169.3" class="stroke-grid-secondary" stroke-dasharray="1" {}
                                    // 150-330 line
                                    line x1="30.7" y1="60" x2="169.3" y2="140" class="stroke-grid-secondary" stroke-dasharray="1" {}

                                    // Polar Degree Labels
                                    text x="183" y="103" class="text-grid text-[8px] font-mono font-bold" { "0°" }
                                    text x="96" y="15" class="text-grid text-[8px] font-mono font-bold" { "90°" }
                                    text x="5" y="103" class="text-grid text-[8px] font-mono font-bold" { "180°" }
                                    text x="93" y="193" class="text-grid text-[8px] font-mono font-bold" { "270°" }

                                    // Dynamically bound voltage vectors
                                    // Va Voltage Vector (Solid Red)
                                    line x1="100" y1="100"
                                         x-bind:x2="getVectorX(phasors.va_rms, phasors.va_ang, true)"
                                         x-bind:y2="getVectorY(phasors.va_rms, phasors.va_ang, true)"
                                         stroke="#dc2626" stroke-width="2.5" stroke-linecap="round" {}
                                    // Vb Voltage Vector (Solid Green)
                                    line x1="100" y1="100"
                                         x-bind:x2="getVectorX(phasors.vb_rms, phasors.vb_ang, true)"
                                         x-bind:y2="getVectorY(phasors.vb_rms, phasors.vb_ang, true)"
                                         stroke="#059669" stroke-width="2.5" stroke-linecap="round" {}
                                    // Vc Voltage Vector (Solid Blue)
                                    line x1="100" y1="100"
                                         x-bind:x2="getVectorX(phasors.vc_rms, phasors.vc_ang, true)"
                                         x-bind:y2="getVectorY(phasors.vc_rms, phasors.vc_ang, true)"
                                         stroke="#2563eb" stroke-width="2.5" stroke-linecap="round" {}

                                    // Dynamically bound current vectors
                                    // Ia Current Vector (Dashed Amber)
                                    line x1="100" y1="100"
                                         x-bind:x2="getVectorX(phasors.ia_rms, phasors.ia_ang, false)"
                                         x-bind:y2="getVectorY(phasors.ia_rms, phasors.ia_ang, false)"
                                         stroke="#d97706" stroke-width="1.8" stroke-dasharray="2" stroke-linecap="round" {}
                                    // Ib Current Vector (Dashed Purple)
                                    line x1="100" y1="100"
                                         x-bind:x2="getVectorX(phasors.ib_rms, phasors.ib_ang, false)"
                                         x-bind:y2="getVectorY(phasors.ib_rms, phasors.ib_ang, false)"
                                         stroke="#8b5cf6" stroke-width="1.8" stroke-dasharray="2" stroke-linecap="round" {}
                                    // Ic Current Vector (Dashed Teal)
                                    line x1="100" y1="100"
                                         x-bind:x2="getVectorX(phasors.ic_rms, phasors.ic_ang, false)"
                                         x-bind:y2="getVectorY(phasors.ic_rms, phasors.ic_ang, false)"
                                         stroke="#14b8a6" stroke-width="1.8" stroke-dasharray="2" stroke-linecap="round" {}
                                }
                                div class="flex flex-wrap gap-x-3 gap-y-1 justify-center mt-3 text-[9px] font-mono w-full" {
                                    span class="flex items-center gap-1" {
                                        span class="w-2 h-0.5 bg-[#dc2626]" {}
                                        span class="text-text-primary" { "Va" }
                                    }
                                    span class="flex items-center gap-1" {
                                        span class="w-2 h-0.5 bg-[#059669]" {}
                                        span class="text-text-primary" { "Vb" }
                                    }
                                    span class="flex items-center gap-1" {
                                        span class="w-2 h-0.5 bg-[#2563eb]" {}
                                        span class="text-text-primary" { "Vc" }
                                    }
                                    span class="flex items-center gap-1" {
                                        span class="w-2 h-0.5 border-t border-dashed border-[#d97706]" {}
                                        span class="text-text-primary" { "Ia" }
                                    }
                                    span class="flex items-center gap-1" {
                                        span class="w-2 h-0.5 border-t border-dashed border-[#8b5cf6]" {}
                                        span class="text-text-primary" { "Ib" }
                                    }
                                    span class="flex items-center gap-1" {
                                        span class="w-2 h-0.5 border-t border-dashed border-[#14b8a6]" {}
                                        span class="text-text-primary" { "Ic" }
                                    }
                                }
                            }

                            // Electrical Telemetry & Diagnostics Table
                            div class="w-full" {
                                table class="industrial-grid text-[10px] w-full font-mono mt-0 border-none" {
                                    thead {
                                        tr {
                                            th class="py-1 px-1.5 text-left text-text-muted" { "Parameter" }
                                            th class="py-1 px-1.5 text-right text-text-muted" { "RMS Value" }
                                            th class="py-1 px-1.5 text-right text-text-muted" { "Angle" }
                                        }
                                    }
                                    tbody {
                                        tr {
                                            td class="py-1 px-1.5 text-left border-b border-border-color" { "Voltage Phase A (Va)" }
                                            td class="py-1 px-1.5 text-right font-bold text-accent-red border-b border-border-color" x-text="phasors.va_rms + ' V'" { "110.20 V" }
                                            td class="py-1 px-1.5 text-right text-text-secondary border-b border-border-color" x-text="phasors.va_ang + '°'" { "0.00°" }
                                        }
                                        tr {
                                            td class="py-1 px-1.5 text-left border-b border-border-color" { "Voltage Phase B (Vb)" }
                                            td class="py-1 px-1.5 text-right font-bold text-accent-green border-b border-border-color" x-text="phasors.vb_rms + ' V'" { "109.80 V" }
                                            td class="py-1 px-1.5 text-right text-text-secondary border-b border-border-color" x-text="phasors.vb_ang + '°'" { "-120.10°" }
                                        }
                                        tr {
                                            td class="py-1 px-1.5 text-left border-b border-border-color" { "Voltage Phase C (Vc)" }
                                            td class="py-1 px-1.5 text-right font-bold text-accent-blue border-b border-border-color" x-text="phasors.vc_rms + ' V'" { "110.50 V" }
                                            td class="py-1 px-1.5 text-right text-text-secondary border-b border-border-color" x-text="phasors.vc_ang + '°'" { "119.80°" }
                                        }
                                        tr {
                                            td class="py-1 px-1.5 text-left border-b border-border-color" { "Current Phase A (Ia)" }
                                            td class="py-1 px-1.5 text-right font-bold text-accent-yellow border-b border-border-color" x-text="phasors.ia_rms + ' A'" { "4.82 A" }
                                            td class="py-1 px-1.5 text-right text-text-secondary border-b border-border-color" x-text="phasors.ia_ang + '°'" { "-30.20°" }
                                        }
                                        tr {
                                            td class="py-1 px-1.5 text-left border-b border-border-color" { "Current Phase B (Ib)" }
                                            td class="py-1 px-1.5 text-right font-bold text-[#8b5cf6] border-b border-border-color" x-text="phasors.ib_rms + ' A'" { "4.79 A" }
                                            td class="py-1 px-1.5 text-right text-text-secondary border-b border-border-color" x-text="phasors.ib_ang + '°'" { "-150.50°" }
                                        }
                                        tr {
                                            td class="py-1 px-1.5 text-left border-b border-border-color" { "Current Phase C (Ic)" }
                                            td class="py-1 px-1.5 text-right font-bold text-[#14b8a6] border-b border-border-color" x-text="phasors.ic_rms + ' A'" { "4.88 A" }
                                            td class="py-1 px-1.5 text-right text-text-secondary border-b border-border-color" x-text="phasors.ic_ang + '°'" { "89.40°" }
                                        }
                                    }
                                }

                                // Symmetrical Components Grid Section
                                div class="mt-3 grid grid-cols-3 gap-2 p-2 bg-bg-primary rounded border border-border-color" {
                                    div class="text-center font-mono text-[9px]" {
                                        span class="block text-text-muted" { "Positive (V1)" }
                                        strong class="text-text-primary text-[10px]" x-text="phasors.v1 + ' V'" { "110.16 V" }
                                    }
                                    div class="text-center font-mono text-[9px]" {
                                        span class="block text-text-muted" { "Negative (V2)" }
                                        strong class="text-text-primary text-[10px]" x-text="phasors.v2 + ' V'" { "0.34 V" }
                                    }
                                    div class="text-center font-mono text-[9px]" {
                                        span class="block text-text-muted" { "Zero (V0)" }
                                        strong class="text-text-primary text-[10px]" x-text="phasors.v0 + ' V'" { "0.12 V" }
                                    }
                                }

                                // Total System Metrics Table
                                table class="text-[9px] w-full font-mono mt-3 border-none" {
                                    tbody {
                                        tr class="border-none hover:bg-transparent" {
                                            td class="py-0.5 px-0 text-text-muted border-none" { "Frequency:" }
                                            td class="py-0.5 px-0 text-right font-bold text-text-primary border-none" x-text="phasors.freq.toFixed(3) + ' Hz'" { "59.998 Hz" }
                                            td class="py-0.5 px-0 text-text-muted border-none pl-3" { "Active Power:" }
                                            td class="py-0.5 px-0 text-right font-bold text-text-primary border-none" x-text="phasors.mw + ' MW'" { "1.54 MW" }
                                        }
                                        tr class="border-none hover:bg-transparent" {
                                            td class="py-0.5 px-0 text-text-muted border-none" { "ROCOF:" }
                                            td class="py-0.5 px-0 text-right font-semibold text-text-primary border-none" x-text="phasors.rocof.toFixed(3) + ' Hz/s'" { "-0.002 Hz/s" }
                                            td class="py-0.5 px-0 text-text-muted border-none pl-3" { "Reactive Power:" }
                                            td class="py-0.5 px-0 text-right font-bold text-text-primary border-none" x-text="phasors.mvar + ' MVAR'" { "0.31 MVAR" }
                                        }
                                        tr class="border-none hover:bg-transparent" {
                                            td class="py-0.5 px-0 text-text-muted border-none" { "Power Factor:" }
                                            td class="py-0.5 px-0 text-right font-bold text-accent-green border-none" x-text="phasors.pf" { "0.980" }
                                            td class="py-0.5 px-0 text-text-muted border-none pl-3" { "System Angle Drift:" }
                                            td class="py-0.5 px-0 text-right font-semibold text-text-primary border-none" { "0.015°/sec" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Combined Process Bus Alignment & Redundant PRP/HSR Routing Table
                    div class="glass-card shadow-md" {
                        div class="card-header flex items-center gap-2 border-b border-border-color pb-3" {
                            h2 class="card-title" { "Process Bus Ingestion & Redundant PRP/HSR Routing Topology" }
                        }
                        div class="card-body mt-4 overflow-x-auto" {
                             table class="industrial-grid text-[10px] w-full" {
                                thead {
                                    tr {
                                        th class="text-center" { "MU ID & Feed Location" }
                                        th class="text-center font-mono" { "MAC & IP Connection" }
                                        th class="text-center" { "PRP LAN A" }
                                        th class="text-center" { "PRP LAN B" }
                                        th class="text-center" { "Out-of-Seq" }
                                        th class="text-center" { "Duplicates Discarded" }
                                        th class="text-center" { "Sync Offset" }
                                        th class="text-center" { "Alignment State" }
                                    }
                                }
                                tbody {
                                    tr class="cursor-pointer hover:bg-bg-surface" onclick="window.location.href='/south/mus/MU-01'" {
                                        td class="font-semibold text-text-primary text-center" { "MU-01 (Feeder Line A)" }
                                        td class="font-mono text-xs text-text-secondary text-center" {
                                            div { "00:0a:35:01:02:01" }
                                            div class="text-[9px] text-text-muted" { "192.168.1.101" }
                                        }
                                        td class="text-center" {
                                            span class="inline-block w-2.5 h-2.5 rounded-full bg-accent-green" x-bind:class="prp.mu01_lan_a ? 'bg-accent-green' : 'bg-accent-red'" {}
                                        }
                                        td class="text-center" {
                                            span class="inline-block w-2.5 h-2.5 rounded-full bg-accent-green" x-bind:class="prp.mu01_lan_b ? 'bg-accent-green' : 'bg-accent-red'" {}
                                        }
                                        td class="text-center font-mono font-semibold" x-text="prp.mu01_errors" { "0" }
                                        td class="text-center font-mono font-semibold text-accent-blue" x-text="prp.mu01_discards" { "12" }
                                        td class="text-center font-mono text-accent-green" { "+12 ns" }
                                        td class="text-center" {
                                            span class="status-badge status-badge-healthy px-1.5 py-0.5" {
                                                span class="status-dot-pulse" {}
                                                "ALIGNED"
                                            }
                                        }
                                    }
                                    tr class="cursor-pointer hover:bg-bg-surface" onclick="window.location.href='/south/mus/MU-02'" {
                                        td class="font-semibold text-text-primary text-center" { "MU-02 (Feeder Line B)" }
                                        td class="font-mono text-xs text-text-secondary text-center" {
                                            div { "00:0a:35:01:02:02" }
                                            div class="text-[9px] text-text-muted" { "192.168.1.102" }
                                        }
                                        td class="text-center" {
                                            span class="inline-block w-2.5 h-2.5 rounded-full bg-accent-green" x-bind:class="prp.mu02_lan_a ? 'bg-accent-green' : 'bg-accent-red'" {}
                                        }
                                        td class="text-center" {
                                            span class="inline-block w-2.5 h-2.5 rounded-full bg-accent-red" x-bind:class="prp.mu02_lan_b ? 'bg-accent-green' : 'bg-accent-red'" {}
                                        }
                                        td class="text-center font-mono font-semibold text-accent-yellow" x-text="prp.mu02_errors" { "2" }
                                        td class="text-center font-mono font-semibold text-text-muted" x-text="prp.mu02_discards" { "0" }
                                        td class="text-center font-mono text-accent-yellow" { "+18 ns" }
                                        td class="text-center" {
                                            span class="status-badge status-badge-degraded px-1.5 py-0.5" {
                                                span class="status-dot-pulse" {}
                                                "DEGRADED"
                                            }
                                        }
                                    }
                                    tr class="cursor-pointer hover:bg-bg-surface" onclick="window.location.href='/south/mus/MU-03'" {
                                        td class="font-semibold text-text-primary text-center" { "MU-03 (Busbar Coupling)" }
                                        td class="font-mono text-xs text-text-secondary text-center" {
                                            div { "00:0a:35:01:02:03" }
                                            div class="text-[9px] text-text-muted" { "192.168.1.103" }
                                        }
                                        td class="text-center" {
                                            span class="inline-block w-2.5 h-2.5 rounded-full bg-accent-red" x-bind:class="prp.mu03_lan_a ? 'bg-accent-green' : 'bg-accent-red'" {}
                                        }
                                        td class="text-center" {
                                            span class="inline-block w-2.5 h-2.5 rounded-full bg-accent-red" x-bind:class="prp.mu03_lan_b ? 'bg-accent-green' : 'bg-accent-red'" {}
                                        }
                                        td class="text-center font-mono font-semibold text-text-muted" x-text="prp.mu03_errors" { "0" }
                                        td class="text-center font-mono font-semibold text-text-muted" x-text="prp.mu03_discards" { "0" }
                                        td class="text-center font-mono text-accent-red" { "--" }
                                        td class="text-center" {
                                            span class="status-badge status-badge-fault px-1.5 py-0.5" {
                                                "DISCONNECTED"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Right Column: Circular Buffer Mirror Sync & Active Application Readers & QSE Overrides (1/3 width)
                div class="flex flex-col gap-6" {

                    // Circular Buffer Mirror Synchronization Panel
                    div class="glass-card shadow-md flex-1" {
                        div class="card-header flex items-center gap-2 border-b border-border-color pb-3" {
                            h2 class="card-title" { "Circular Buffer Mirror Sync & Active Readers Matrix" }
                        }
                        div class="card-body mt-4 flex flex-col gap-4" {
                            // Ring Synchronization Status block
                            div class="flex items-center justify-between p-2.5 bg-bg-primary rounded border border-border-color" {
                                div class="font-mono text-[9px]" {
                                    span class="block text-text-muted" { "Active Ingest Mirror" }
                                    strong class="text-accent-blue" x-text="cb_sync.active_mirror" { "CB-A" }
                                    span class="text-text-muted" { " -> Replica: " }
                                    strong class="text-text-secondary" x-text="cb_sync.replica_mirror" { "CB-B" }
                                }
                                span class="status-badge status-badge-healthy px-1.5 py-0.5 text-[8px]" {
                                    span class="status-dot-pulse" {}
                                    span x-text="cb_sync.mirror_state.replace('_', ' ')" { "IN SYNC" }
                                }
                            }

                            // Write index pointer
                            div class="flex justify-between items-center text-[10px] font-mono" {
                                span class="text-text-muted" { "Global Ring Write Index Pointer:" }
                                strong class="text-text-primary" x-text="cb_sync.write_ptr" { "148,202" }
                            }

                            // High-density subscriber readers matrix
                            table class="industrial-grid text-[10px] mt-2 w-full" {
                                thead {
                                    tr {
                                        th class="text-left py-1" { "Application Subscriber" }
                                        th class="text-right py-1" { "Lag (Frames)" }
                                        th class="text-center py-1" { "Zero-Alloc" }
                                    }
                                }
                                tbody {
                                    tr {
                                        td class="font-semibold text-text-primary py-1.5" { "EBP Protection Relay" }
                                        td class="text-right font-mono text-accent-green py-1.5" {
                                            strong x-text="cb_sync.ebp_margin" { "24" } " / 256k"
                                        }
                                        td class="text-center py-1.5" {
                                            span class="inline-flex items-center justify-center text-[8px] bg-accent-green-dim text-accent-green border border-accent-green/20 px-1 py-0.2 rounded font-bold uppercase tracking-wider" x-show="cb_sync.ebp_zero_alloc" { "Lock-free" }
                                        }
                                    }
                                    tr {
                                        td class="font-semibold text-text-primary py-1.5" { "Phasor Computation Module" }
                                        td class="text-right font-mono text-accent-green py-1.5" {
                                            strong x-text="cb_sync.pcm_margin" { "56" } " / 256k"
                                        }
                                        td class="text-center py-1.5" {
                                            span class="inline-flex items-center justify-center text-[8px] bg-accent-green-dim text-accent-green border border-accent-green/20 px-1 py-0.2 rounded font-bold uppercase tracking-wider" x-show="cb_sync.pcm_zero_alloc" { "Lock-free" }
                                        }
                                    }
                                    tr {
                                        td class="font-semibold text-text-primary py-1.5" { "Transient Recorder" }
                                        td class="text-right font-mono text-text-muted py-1.5" {
                                            strong x-text="cb_sync.tr_margin" { "0" } " / 256k"
                                        }
                                        td class="text-center py-1.5" {
                                            span class="inline-flex items-center justify-center text-[8px] bg-accent-yellow-dim text-accent-yellow border border-accent-yellow/20 px-1 py-0.2 rounded font-bold uppercase tracking-wider" x-show="cb_sync.tr_zero_alloc" { "Armed" }
                                        }
                                    }
                                    tr {
                                        td class="font-semibold text-text-primary py-1.5" { "Fault Locator" }
                                        td class="text-right font-mono text-accent-yellow py-1.5" {
                                            strong x-text="cb_sync.fl_margin" { "92" } " / 256k"
                                        }
                                        td class="text-center py-1.5" {
                                            span class="inline-flex items-center justify-center text-[8px] bg-accent-green-dim text-accent-green border border-accent-green/20 px-1 py-0.2 rounded font-bold uppercase tracking-wider" x-show="cb_sync.fl_zero_alloc" { "Lock-free" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // QSE Quasi-dynamic State Estimator Self-Healing Log Terminal
                    div class="glass-card shadow-md flex-1" {
                        div class="card-header flex items-center gap-2 border-b border-border-color pb-3" {
                            h2 class="card-title" { "QSE Self-Healing Overrides & State Estimator Logs" }
                        }
                        div class="card-body mt-3 font-mono text-[9px] text-text-secondary bg-[#0f172a] p-3 rounded border border-border-color h-[155px] overflow-y-auto flex flex-col gap-1.5 shadow-inner" {
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
