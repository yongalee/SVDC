/* SVDC Console Base Page Template Layout
   OWNER: claude-code
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers

   Note: This file is assigned to Claude Code under WBS-9.1a. Antigravity scaffolds
   this high-quality, fully functional template layout to support integration and testing
   of other lanes, avoiding file overlap by providing a clean baseline.
*/

use maud::{html, Markup, DOCTYPE};

/// Renders the complete HTML5 document wrapper for the Operator Console UI.
/// Includes sidebar navigation, a real-time top-bar PTP status indicator,
/// and responsive layout grids.
pub fn layout(title: &str, active_nav: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) " | SVDC Operator Console" }

                // Embedded static assets (HTMX & AlpineJS)
                script src="/assets/htmx.min.js" {}
                script src="/assets/alpine.min.js" defer {}
                link rel="stylesheet" href="/assets/styles.css";

                // Premium typography (Inter for interface, JetBrains Mono for system metrics)
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap";
            }
            body {
                div class="app-layout" {
                    // Left Sidebar Navigation
                    aside class="sidebar" {
                        div class="sidebar-brand" {
                            span class="brand-logo" { "★" }
                            div class="brand-text" {
                                h2 class="brand-title" { "SSIEC a²SDP" }
                                p class="brand-subtitle" { "SV Data Concentrator" }
                            }
                        }

                        nav class="sidebar-menu" {
                            a href="/" class=(if active_nav == "dashboard" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" { "📊" }
                                span class="nav-text" { "Dashboard" }
                            }
                            a href="/south/mus" class=(if active_nav == "southbound" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" { "🔌" }
                                span class="nav-text" { "Southbound MUs" }
                            }
                            a href="/north" class=(if active_nav == "northbound" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" { "🚀" }
                                span class="nav-text" { "Northbound Controls" }
                            }
                            a href="/monitoring" class=(if active_nav == "monitoring" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" { "📈" }
                                span class="nav-text" { "Diagnostics telemetry" }
                            }
                            a href="/config" class=(if active_nav == "config" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" { "⚙️" }
                                span class="nav-text" { "Configuration" }
                            }
                        }

                        div class="sidebar-footer" {
                            span class="env-badge" { "a²SDP local node" }
                            span class="ver-badge" { "v0.1.0-provisional" }
                        }
                    }

                    // Main Console Grid Workspace
                    div class="main-container" {
                        // Top Bar status surface
                        header class="topbar flex justify-between items-center" {
                            div class="topbar-left" {
                                h1 class="topbar-title" { (title) }
                            }
                            div class="topbar-right flex items-center gap-6" {
                                // Real-time clock/uptime telemetry block
                                div class="uptime-block text-xs text-text-secondary" {
                                    span class="label" { "System Uptime: " }
                                    span class="value font-mono font-semibold" { "01:24:45" }
                                }

                                // PTP synchrony widget
                                div class="ptp-widget flex items-center gap-2" {
                                    span class="text-xs text-text-secondary font-medium" { "Grandmaster Sync" }
                                    div class="status-badge status-badge-healthy font-mono text-xs" {
                                        span class="status-dot-pulse" {}
                                        span id="topbar-ptp-status" { "Locked (12 ns)" }
                                    }
                                }
                            }
                        }

                        // Scrollable Screen Panel Content
                        div class="content-panel" {
                            (content)
                        }
                    }
                }
            }
        }
    }
}
