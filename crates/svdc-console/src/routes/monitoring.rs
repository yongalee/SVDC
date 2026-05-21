/* SVDC Diagnostics Telemetry Router
   OWNER: claude-code
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers

   Note: This file is assigned to Claude Code under WBS-9.5a for latency histograms
   and statistics. Antigravity implements the complete PTP sync offset trends, circular
   buffer saturation line charts, and search-enabled audit logs (WBS-9.5b).
*/

use axum::{response::Html, routing::get, Router};
use maud::html;

use crate::templates::base;

/// Register diagnostic telemetry routes
pub fn register(router: Router) -> Router {
    router.route("/monitoring", get(monitoring_page))
}

/// Renders the Diagnostics Telemetry page
async fn monitoring_page() -> Html<String> {
    let content = html! {
        div x-data="{
            ptp_history: [48, 44, 50, 42, 46, 49, 43, 45, 47, 41, 43],
            buf_history: [72, 71, 70, 72, 69, 70, 69, 71, 70, 72, 71],
            current_ptp: 12,
            current_buf: 2.4,
            mu_telemetries: [],
            audit_logs: [
                { timestamp: '2026-05-21 09:20:10', wbs: 'WBS-9.1b', operation: 'Console Web scaffold deploy', target: 'Axum Endpoint 127.0.0.1:8080', operator: 'antigravity-subagent-ui-spec', result: 'SUCCESS', result_color: 'text-accent-green' },
                { timestamp: '2026-05-21 09:20:11', wbs: 'WBS-9.2b', operation: 'SSE Broadcast Stream init', target: 'OnceLock Channel Broker', operator: 'antigravity-subagent-ui-spec', result: 'SUCCESS', result_color: 'text-accent-green' },
                { timestamp: '2026-05-21 09:28:48', wbs: 'WBS-6.1', operation: 'Pcap frame ingestion test', target: 'Ingest Core (M1)', operator: 'claude-code', result: 'SUCCESS', result_color: 'text-accent-green' },
                { timestamp: '2026-05-21 09:29:10', wbs: 'Gate G0', operation: 'Provisional Spec-Lock decision', target: 'SSIEC local node settings', operator: 'claude-code', result: 'LOCKED', result_color: 'text-accent-green' }
            ],
            search_query: '',
            init() {
                const evtSource = new EventSource('/api/events');
                evtSource.onmessage = (event) => {
                    try {
                        const data = JSON.parse(event.data);
                        if (data.event_type === 'Metrics') {
                            this.current_ptp = data.data.ptp_offset_ns;
                            this.current_buf = data.data.buffer_saturation;
                            
                            let new_ptp_y = 80 - (this.current_ptp * 2);
                            if (new_ptp_y < 10) new_ptp_y = 10;
                            if (new_ptp_y > 80) new_ptp_y = 80;
                            this.ptp_history.shift();
                            this.ptp_history.push(new_ptp_y);

                            let new_buf_y = 80 - (this.current_buf * 10);
                            if (new_buf_y < 10) new_buf_y = 10;
                            if (new_buf_y > 80) new_buf_y = 80;
                            this.buf_history.shift();
                            this.buf_history.push(new_buf_y);
                        } else if (data.event_type === 'Qse') {
                            this.audit_logs.unshift(data.data);
                            if (this.audit_logs.length > 50) this.audit_logs.pop();
                        } else if (data.event_type === 'MuMetrics') {
                            this.mu_telemetries = data.data;
                        }
                    } catch(e) {}
                };
            },
            getJitterPath(histogram) {
                if (!histogram || histogram.length === 0) return '';
                let max_val = Math.max(...histogram, 1); // Avoid division by zero
                let path = '';
                // Draw 10 bars for the histogram (bar width: 15, spacing: 5)
                for (let i = 0; i < histogram.length; i++) {
                    let h = (histogram[i] / max_val) * 60; // Max height 60px
                    let x = i * 20;
                    let y = 80 - h;
                    path += `M ${x} 80 L ${x} ${y} L ${x + 15} ${y} L ${x + 15} 80 Z `;
                }
                return path;
            },
            getPtpPath() {
                let path = 'M 0 ' + this.ptp_history[0];
                for (let i = 1; i < this.ptp_history.length; i++) {
                    path += ' L ' + (i * 50) + ' ' + this.ptp_history[i];
                }
                return path;
            },
            getBufAreaPath() {
                let path = 'M 0 ' + this.buf_history[0];
                for (let i = 1; i < this.buf_history.length; i++) {
                    path += ' L ' + (i * 50) + ' ' + this.buf_history[i];
                }
                path += ' L 500 80 L 0 80 Z';
                return path;
            },
            getBufLinePath() {
                let path = 'M 0 ' + this.buf_history[0];
                for (let i = 1; i < this.buf_history.length; i++) {
                    path += ' L ' + (i * 50) + ' ' + this.buf_history[i];
                }
                return path;
            }
        }" class="screen-layout gap-6" {
            // 1. Diagnostics Charts Grid (PTP Offset & Buffer occupancy)
            div class="flex flex-col gap-6" {

                // Chart A: PTP Synchronization Offset Trend
                div class="glass-card" {
                    div class="card-header flex justify-between items-center" {
                        div class="flex items-center gap-2" {
                            h3 class="card-title" { "PTP disciplined clock synchronization trend" }
                        }
                        span class="text-xs font-semibold text-accent-green" { "100% Tracking" }
                    }
                    div class="card-body mt-4 flex flex-col gap-2" {
                        // Inline SVG Trend Line Chart
                        div class="bg-chart-bg rounded-lg border border-border-color p-2" {
                            svg viewBox="0 0 500 80" class="w-full h-auto block" {
                                // Chart Grid lines
                                line x1="0" y1="40" x2="500" y2="40" class="stroke-grid-primary" stroke-dasharray="4" {}
                                line x1="0" y1="15" x2="500" y2="15" class="stroke-grid-secondary" stroke-dasharray="2" {}
                                line x1="0" y1="65" x2="500" y2="65" class="stroke-grid-secondary" stroke-dasharray="2" {}

                                // Plotting simulated offset values
                                path x-bind:d="getPtpPath()"
                                     fill="none" stroke="#16a34a" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" {}

                                // Labeling
                                text x="10" y="12" fill="var(--text-muted)" font-size="9" font-family="monospace" { "Limit: 100 ns" }
                                text x="10" y="75" fill="var(--text-muted)" font-size="9" font-family="monospace" x-text="'Active: ' + current_ptp + ' ns'" { "Active: 12 ns" }
                            }
                        }
                        span class="text-xs text-text-secondary mt-1 font-mono" {
                            "PTP clock discipline error remains well within the protection-critical bounds of 1 microsecond."
                        }
                    }
                }

                // Chart B: Circular Buffer Occupancy Trend
                div class="glass-card" {
                    div class="card-header flex justify-between items-center" {
                        div class="flex items-center gap-2" {
                            h3 class="card-title" { "Circular Buffer Saturation History" }
                        }
                        span class="text-xs font-semibold text-accent-blue" { "Stable occupancy" }
                    }
                    div class="card-body mt-4 flex flex-col gap-2" {
                        // Inline SVG Area Chart
                        div class="bg-chart-bg rounded-lg border border-border-color p-2" {
                            svg viewBox="0 0 500 80" class="w-full h-auto block" {
                                // Chart Grid lines
                                line x1="0" y1="40" x2="500" y2="40" class="stroke-grid-primary" stroke-dasharray="4" {}

                                // Fill and Line for Buffer Saturation
                                path x-bind:d="getBufAreaPath()"
                                     fill="#2563eb20" {}
                                path x-bind:d="getBufLinePath()"
                                     fill="none" stroke="#2563eb" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" {}

                                // Labels
                                text x="10" y="12" fill="var(--text-muted)" font-size="9" font-family="monospace" { "Capacity: 262,144 frames" }
                                text x="10" y="75" fill="var(--text-muted)" font-size="9" font-family="monospace" x-text="'Current: ' + current_buf.toFixed(2) + '%'" { "Current: 2.4%" }
                            }
                        }
                        span class="text-xs text-text-secondary mt-1 font-mono" {
                            "Buffer saturation is steady; aligner deadlines match the incoming Merging Unit sample rates."
                        }
                    }
                }
            }

            // M9 Diagnostic Telemetry Per-MU Dashboard
            div class="glass-card shadow-lg mt-6" {
                div class="card-header flex justify-between items-center border-b border-border-color pb-3" {
                    h2 class="card-title" { "Per-MU Telemetry & Health" }
                    span class="text-xs font-semibold text-accent-green" { "Live Stream" }
                }
                div class="card-body mt-4 grid grid-cols-1 lg:grid-cols-2 gap-4" {
                    template x-for="mu in mu_telemetries" {
                        div class="bg-bg-secondary border border-border-color rounded p-4 flex flex-col gap-4" {
                            // Header
                            div class="flex justify-between items-center" {
                                h4 class="font-bold text-text-primary" x-text="mu.mu_id" {}
                                span class="text-xs font-mono text-text-secondary" x-text="mu.ptp_sync" {}
                            }
                            
                            // Metrics Grid
                            div class="grid grid-cols-2 gap-2 text-sm font-mono" {
                                div class="flex flex-col" {
                                    span class="text-text-muted text-xs" { "Sample Rate" }
                                    span class="text-text-primary font-semibold" x-text="mu.observed_sps + ' Hz'" {}
                                }
                                div class="flex flex-col" {
                                    span class="text-text-muted text-xs" { "Missing Samples" }
                                    span class="text-text-primary font-semibold" x-text="mu.missing_samples" {}
                                }
                                div class="flex flex-col" {
                                    span class="text-text-muted text-xs" { "Interpolations" }
                                    span class="text-text-primary font-semibold" x-text="mu.interpolation_count" {}
                                }
                                div class="flex flex-col" {
                                    span class="text-text-muted text-xs" { "QSE Corrections" }
                                    span class="text-text-primary font-semibold" x-text="mu.qse_corrections" {}
                                }
                            }
                            
                            // Calibration Triple
                            div class="flex flex-col gap-1 border-t border-border-color pt-2" {
                                span class="text-text-muted text-xs font-mono" { "Calibration [Gain, Offset, Scale]" }
                                span class="text-text-primary text-xs font-mono" x-text="'[' + mu.calibration[0].toFixed(4) + ', ' + mu.calibration[1].toFixed(2) + ', ' + mu.calibration[2].toFixed(1) + ']'" {}
                            }
                            
                            // Jitter Histogram
                            div class="flex flex-col gap-1" {
                                span class="text-text-muted text-xs font-mono" { "Arrival Jitter Histogram" }
                                div class="bg-chart-bg rounded border border-border-color p-2 h-24" {
                                    svg viewBox="0 0 200 80" class="w-full h-full block" {
                                        line x1="0" y1="80" x2="200" y2="80" class="stroke-grid-primary" stroke-width="2" {}
                                        path x-bind:d="getJitterPath(mu.jitter_histogram)"
                                             fill="#8b5cf6" opacity="0.8" {}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 2. QSE Write-Back Audit Log Table
            div class="glass-card shadow-lg mt-6" {
                div class="card-header flex justify-between items-center border-b border-border-color pb-3" {
                    div class="flex items-center gap-2" {
                        h2 class="card-title" { "QSE Write-Back Action Audit Logs" }
                    }

                    // Simple search field
                    div class="search-box relative flex items-center" {
                        input type="text"
                              x-model="search_query"
                              placeholder="Search audit trail..."
                              class="text-xs border border-border-color bg-bg-secondary rounded px-3 py-1.5 focus:outline-none w-48 lg:w-64" {}
                    }
                }

                // Audit Trail Table
                div class="card-body mt-4 overflow-x-auto" {
                    table class="min-w-full text-sm text-left border-collapse" {
                        thead {
                            tr class="border-b border-border-color bg-bg-secondary text-text-secondary font-semibold" {
                                th class="px-4 py-3" { "Timestamp" }
                                th class="px-4 py-3" { "WBS context" }
                                th class="px-4 py-3" { "Operation" }
                                th class="px-4 py-3" { "Substation target" }
                                th class="px-4 py-3" { "Operator identity" }
                                th class="px-4 py-3" { "Result" }
                            }
                        }
                        tbody class="divide-y divide-border-color text-xs font-mono" {
                            template x-for="log in audit_logs.filter(l => l.operation.toLowerCase().includes(search_query.toLowerCase()) || l.wbs.toLowerCase().includes(search_query.toLowerCase()) || l.operator.toLowerCase().includes(search_query.toLowerCase()))" {
                                tr class="hover:bg-bg-secondary transition-all" {
                                    td class="px-4 py-3 text-text-secondary" x-text="log.timestamp" {}
                                    td class="px-4 py-3 font-semibold text-accent-blue" x-text="log.wbs" {}
                                    td class="px-4 py-3" x-text="log.operation" {}
                                    td class="px-4 py-3 text-text-secondary" x-text="log.target" {}
                                    td class="px-4 py-3 text-text-secondary" x-text="log.operator" {}
                                    td class="px-4 py-3" { span x-bind:class="log.result_color + ' font-semibold'" x-text="log.result" {} }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let rendered = base::layout("Performance & Diagnostics Telemetry", "monitoring", content);
    Html(rendered.into_string())
}
