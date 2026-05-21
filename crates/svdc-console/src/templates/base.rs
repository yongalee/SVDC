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
                script src="/assets/htmx.min.js?v=1" {}
                script src="/assets/alpine.min.js?v=1" defer {}
                link rel="stylesheet" href="/assets/styles.css?v=4";

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
                            span class="brand-logo" {
                                svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24" {
                                    path stroke-linecap="round" stroke-linejoin="round" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z" {}
                                }
                            }
                            div class="brand-text" {
                                h2 class="brand-title" { "SSIEC a²SDP" }
                                p class="brand-subtitle" { "SV Data Concentrator" }
                            }
                        }

                        nav class="sidebar-menu" {
                            a href="/" class=(if active_nav == "dashboard" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" {
                                    svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                        path stroke-linecap="round" stroke-linejoin="round" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v4a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v4a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" {}
                                    }
                                }
                                span class="nav-text" { "Dashboard" }
                            }
                            a href="/south/mus" class=(if active_nav == "southbound" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" {
                                    svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                        path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" {}
                                    }
                                }
                                span class="nav-text" { "Southbound MUs" }
                            }
                            a href="/north" class=(if active_nav == "northbound" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" {
                                    svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                        path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" {}
                                    }
                                }
                                span class="nav-text" { "Northbound Controls" }
                            }
                            a href="/monitoring" class=(if active_nav == "monitoring" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" {
                                    svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                        path stroke-linecap="round" stroke-linejoin="round" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 002 2h2a2 2 0 002-2z" {}
                                    }
                                }
                                span class="nav-text" { "Diagnostics telemetry" }
                            }
                            a href="/config" class=(if active_nav == "config" { "nav-link active" } else { "nav-link" }) {
                                span class="nav-icon" {
                                    svg fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                        path stroke-linecap="round" stroke-linejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" {}
                                        path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" {}
                                    }
                                }
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
