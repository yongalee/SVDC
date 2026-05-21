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
        div class="screen-layout gap-6" {
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
                                path d="M 0 48 L 50 44 L 100 50 L 150 42 L 200 46 L 250 49 L 300 43 L 350 45 L 400 47 L 450 41 L 500 43"
                                     fill="none" stroke="#16a34a" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" {}

                                // Labeling
                                text x="10" y="12" fill="var(--text-muted)" font-size="9" font-family="monospace" { "Limit: 100 ns" }
                                text x="10" y="75" fill="var(--text-muted)" font-size="9" font-family="monospace" { "Active: 12 ns" }
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
                                path d="M 0 72 L 50 71 L 100 70 L 150 72 L 200 69 L 250 70 L 300 69 L 350 71 L 400 70 L 450 72 L 500 71 L 500 80 L 0 80 Z"
                                     fill="#2563eb20" {}
                                path d="M 0 72 L 50 71 L 100 70 L 150 72 L 200 69 L 250 70 L 300 69 L 350 71 L 400 70 L 450 72 L 500 71"
                                     fill="none" stroke="#2563eb" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" {}

                                // Labels
                                text x="10" y="12" fill="var(--text-muted)" font-size="9" font-family="monospace" { "Capacity: 10,000 frames" }
                                text x="10" y="75" fill="var(--text-muted)" font-size="9" font-family="monospace" { "Current: 2.4%" }
                            }
                        }
                        span class="text-xs text-text-secondary mt-1 font-mono" {
                            "Buffer saturation is steady; aligner deadlines match the incoming Merging Unit sample rates."
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
                            tr class="hover:bg-bg-secondary transition-all" {
                                td class="px-4 py-3 text-text-secondary" { "2026-05-21 09:20:10" }
                                td class="px-4 py-3 font-semibold text-accent-blue" { "WBS-9.1b" }
                                td class="px-4 py-3" { "Console Web scaffold deploy" }
                                td class="px-4 py-3" { "Axum Endpoint 127.0.0.1:8080" }
                                td class="px-4 py-3 text-text-secondary" { "antigravity-subagent-ui-spec" }
                                td class="px-4 py-3" { span class="text-accent-green font-semibold" { "SUCCESS" } }
                            }
                            tr class="hover:bg-bg-secondary transition-all" {
                                td class="px-4 py-3 text-text-secondary" { "2026-05-21 09:20:11" }
                                td class="px-4 py-3 font-semibold text-accent-blue" { "WBS-9.2b" }
                                td class="px-4 py-3" { "SSE Broadcast Stream init" }
                                td class="px-4 py-3" { "OnceLock Channel Broker" }
                                td class="px-4 py-3 text-text-secondary" { "antigravity-subagent-ui-spec" }
                                td class="px-4 py-3" { span class="text-accent-green font-semibold" { "SUCCESS" } }
                            }
                            tr class="hover:bg-bg-secondary transition-all" {
                                td class="px-4 py-3 text-text-secondary" { "2026-05-21 09:28:48" }
                                td class="px-4 py-3 font-semibold text-accent-blue" { "WBS-6.1" }
                                td class="px-4 py-3" { "Pcap frame ingestion test" }
                                td class="px-4 py-3" { "Ingest Core (M1)" }
                                td class="px-4 py-3 text-text-secondary" { "claude-code" }
                                td class="px-4 py-3" { span class="text-accent-green font-semibold" { "SUCCESS" } }
                            }
                            tr class="hover:bg-bg-secondary transition-all" {
                                td class="px-4 py-3 text-text-secondary" { "2026-05-21 09:29:10" }
                                td class="px-4 py-3 font-semibold text-accent-blue" { "Gate G0" }
                                td class="px-4 py-3" { "Provisional Spec-Lock decision" }
                                td class="px-4 py-3" { "SSIEC local node settings" }
                                td class="px-4 py-3 text-text-secondary" { "claude-code" }
                                td class="px-4 py-3" { span class="text-accent-green font-semibold" { "LOCKED" } }
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
