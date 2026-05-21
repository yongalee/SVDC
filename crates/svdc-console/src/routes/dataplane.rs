//! `/dataplane` — UI verification surface for the data-plane crates
//! built in PRs #42–#50.
//!
//! Routes:
//! - `GET  /dataplane` — main page (maud)
//! - `POST /api/dataplane/start` — spawn the synthetic pipeline
//! - `POST /api/dataplane/stop` — join the pipeline task
//! - `GET  /api/dataplane/status` — HTML fragment for HTMX polling
//! - `GET  /api/dataplane/status.json` — same data as JSON
//! - `POST /api/dataplane/tamper` — inject a CRC-mismatched record
//! - `POST /api/dataplane/reset` — wipe buffer + counters + CSV
//! - `GET  /api/dataplane/historian.csv` — download the running CSV
//!
//! The page wires htmx to poll `/api/dataplane/status` every 500 ms
//! and swap the result into the `#dp-status` panel. Each control
//! button is a `<form hx-post="...">` that swaps the same panel.
//!
//! OWNER: claude-code. NFR-10: English-only.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use maud::{html, Markup, PreEscaped};

use crate::dataplane::{DataPipeline, DataPipelineSnapshot};
use crate::templates::base::{layout, Section};

/// Build the data-plane sub-router. Constructs and shares one
/// process-wide [`DataPipeline`].
pub fn router() -> Router {
    let pipe = crate::dataplane::global();
    Router::new()
        .route("/dataplane", get(page))
        .route("/api/dataplane/start", post(start))
        .route("/api/dataplane/stop", post(stop))
        .route("/api/dataplane/status", get(status_html))
        .route("/api/dataplane/status.json", get(status_json))
        .route("/api/dataplane/tamper", post(tamper))
        .route("/api/dataplane/reset", post(reset))
        .route("/api/dataplane/vendor", post(set_vendor))
        .route("/api/dataplane/historian.csv", get(historian_csv))
        .with_state(pipe)
}

async fn page(State(pipe): State<Arc<DataPipeline>>) -> Markup {
    let snap = pipe.snapshot();
    layout(Section::Dataplane, "Data plane", body(&snap))
}

async fn status_html(State(pipe): State<Arc<DataPipeline>>) -> Markup {
    status_panel(&pipe.snapshot())
}

async fn status_json(State(pipe): State<Arc<DataPipeline>>) -> Json<DataPipelineSnapshot> {
    Json(pipe.snapshot())
}

async fn start(State(pipe): State<Arc<DataPipeline>>) -> Markup {
    let _ = pipe.start();
    status_panel(&pipe.snapshot())
}

async fn stop(State(pipe): State<Arc<DataPipeline>>) -> Markup {
    let _ = pipe.stop().await;
    status_panel(&pipe.snapshot())
}

async fn tamper(State(pipe): State<Arc<DataPipeline>>) -> Markup {
    pipe.inject_tamper();
    status_panel(&pipe.snapshot())
}

async fn reset(State(pipe): State<Arc<DataPipeline>>) -> Markup {
    pipe.reset();
    status_panel(&pipe.snapshot())
}

#[derive(serde::Deserialize)]
struct VendorForm {
    vendor: String,
}

async fn set_vendor(
    State(pipe): State<Arc<DataPipeline>>,
    axum::Form(form): axum::Form<VendorForm>,
) -> Markup {
    // "none" / empty resets to the generic DATAPLANE_DEMO svID.
    let preset = if form.vendor.is_empty() || form.vendor == "none" {
        None
    } else {
        Some(form.vendor.as_str())
    };
    let _ = pipe.set_vendor(preset);
    // Re-render the status panel so the operator sees the new vendor
    // label immediately, even before the next htmx poll.
    status_panel(&pipe.snapshot())
}

async fn historian_csv(State(pipe): State<Arc<DataPipeline>>) -> Response {
    match std::fs::read(&pipe.historian_path) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=svdc-dataplane-demo.csv".to_string(),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            "historian CSV does not exist yet; start the pipeline first",
        )
            .into_response(),
    }
}

fn body(snap: &DataPipelineSnapshot) -> Markup {
    html! {
        section.config-section {
            div.config-section-head {
                h2 { "Data-plane pipeline (Phase 0 demo)" }
                p.muted {
                    "End-to-end verification surface for the eight data-plane crates "
                    "(ingress, aligner, subscribe, historian, integrity overlay, "
                    "management API). Click "
                    strong { "Start" }
                    " to spawn a background task that synthesises one IngressFrame every 50 ms, "
                    "pushes it through " code { "Aligner::process_frame" }
                    ", lands the result in a shared "
                    code { "TickBuffer" } " (capacity " (svdc_console_dataplane_const_buffer()) " records), and streams it through "
                    "the historian to a CSV. The status panel below polls every 500 ms."
                }
            }
            div.dp-controls {
                form method="post" action="/api/dataplane/start" hx-post="/api/dataplane/start" hx-target="#dp-status" hx-swap="outerHTML" {
                    button.btn type="submit" { "Start pipeline" }
                }
                form method="post" action="/api/dataplane/stop" hx-post="/api/dataplane/stop" hx-target="#dp-status" hx-swap="outerHTML" {
                    button.btn type="submit" { "Stop pipeline" }
                }
                form method="post" action="/api/dataplane/tamper" hx-post="/api/dataplane/tamper" hx-target="#dp-status" hx-swap="outerHTML" {
                    button.btn.btn-warning type="submit" title="Push a record whose stored CRC does not match its samples — proves the integrity overlay catches it" {
                        "Inject tamper"
                    }
                }
                form method="post" action="/api/dataplane/reset" hx-post="/api/dataplane/reset" hx-target="#dp-status" hx-swap="outerHTML" {
                    button.btn.btn-muted type="submit" title="Clear buffer + counters + CSV" {
                        "Reset"
                    }
                }
                a.btn.btn-link href="/api/dataplane/historian.csv" download="svdc-dataplane-demo.csv" {
                    "Download historian CSV"
                }
            }
            (vendor_selector(crate::dataplane::global().selected_vendor_name()))
            (status_panel(snap))
        }
        section.config-section {
            div.config-section-head {
                h2 { "Management API (svdc-api, ADR-0013)" }
                p.muted {
                    "The PR #50 router is mounted under " code { "/api/mgmt/*" }
                    " on this same listener so you can verify each endpoint from one tab."
                }
            }
            table.industry-table {
                thead {
                    tr {
                        th { "Method" } th { "Path" } th { "Purpose" }
                    }
                }
                tbody {
                    tr {
                        td { "GET" }
                        td { a href="/api/mgmt/health" target="_blank" { "/api/mgmt/health" } }
                        td { "Liveness + integrity verdict (JSON)" }
                    }
                    tr {
                        td { "GET" }
                        td { a href="/api/mgmt/channels" target="_blank" { "/api/mgmt/channels" } }
                        td { "Channel registry snapshot (Phase 0 empty)" }
                    }
                    tr {
                        td { "GET" }
                        td { a href="/api/mgmt/metrics" target="_blank" { "/api/mgmt/metrics" } }
                        td { "Prometheus text exposition format" }
                    }
                    tr {
                        td { "POST" }
                        td { code { "/api/mgmt/calibration/{channel_id}" } }
                        td { "Per-channel " code { "(gain, offset, unit_scale)" } " (JSON body)" }
                    }
                }
            }
            p.muted {
                "Tip: " code { "curl http://127.0.0.1:8080/api/mgmt/health" } " — when the data-plane "
                "demo is running, " code { "tick_buffer_len" } " reflects this page's buffer."
            }
        }
        (PreEscaped(POLL_JS))
    }
}

/// Vendor preset selector + paste-ready simulator command line.
/// The dropdown drives the synthetic loop's svID via
/// `set_vendor`; the command line is the operator's hint for
/// what to type in Terminal A when running the real simulator
/// against the daemon's `--ingress-udp` port.
fn vendor_selector(current: Option<&str>) -> Markup {
    let presets = ssiec_sv_publisher::vendor::ALL;
    let active = current.unwrap_or("none");
    html! {
        div.dp-vendor {
            div.dp-vendor-head {
                h3 { "Vendor preset" }
                p.muted {
                    "Switches the synthetic loop's svID + APPID + VLAN + "
                    "sample-rate to the chosen vendor convention. The "
                    "buffer rolls within seconds; the new svID appears "
                    "on " a href="/south/mus" { "Merging Units" } "."
                }
            }
            form
                method="post"
                action="/api/dataplane/vendor"
                hx-post="/api/dataplane/vendor"
                hx-target="#dp-status"
                hx-swap="outerHTML"
            {
                label for="dp-vendor-select" { "Active preset:" }
                select.dp-vendor-select id="dp-vendor-select" name="vendor" onchange="this.form.requestSubmit()" {
                    option value="none" selected[active == "none"] {
                        "(none — generic DATAPLANE_DEMO svID)"
                    }
                    @for v in presets {
                        option value=(v.name) selected[active == v.name] {
                            (v.name) " — " (v.notes)
                        }
                    }
                }
            }
            div.dp-vendor-cmd {
                strong { "Equivalent simulator command (real UDP feed):" }
                @match current {
                    Some(name) => pre {
                        "cargo run --release -p ssiec-sv-publisher -- udp 127.0.0.1:9100 \\\n    --vendor " (name) " --duration 3600"
                    },
                    None => pre {
                        "cargo run --release -p ssiec-sv-publisher -- udp 127.0.0.1:9100 \\\n    --vendor abb_relion_670 --duration 3600"
                    },
                }
                p.muted {
                    "Then start the daemon with "
                    code { "--ingress-udp 127.0.0.1:9100" }
                    ". See "
                    a href="/" { "Dashboard" }
                    " for the live-feed badge."
                }
            }
        }
    }
}

fn status_panel(snap: &DataPipelineSnapshot) -> Markup {
    let running_class = if snap.running {
        "dp-status running"
    } else {
        "dp-status idle"
    };
    let running_label = if snap.running { "running" } else { "idle" };
    let integrity_class = if snap.integrity_violations == 0 {
        "ok"
    } else {
        "degraded"
    };
    html! {
        div id="dp-status"
            class=(running_class)
            hx-get="/api/dataplane/status"
            hx-trigger="every 500ms"
            hx-swap="outerHTML"
        {
            div.dp-status-row {
                span.dp-status-label { "Pipeline" }
                span.dp-status-value { (running_label) }
            }
            div.dp-status-row {
                span.dp-status-label { "Tick buffer" }
                span.dp-status-value {
                    (snap.buffer_len) " / " (snap.buffer_capacity)
                    span.muted { " (TickRecord, ADR-0009 §6)" }
                }
            }
            div.dp-status-row {
                span.dp-status-label { "Ticks emitted" }
                span.dp-status-value { (snap.ticks_emitted) }
            }
            div.dp-status-row {
                span.dp-status-label { "Latest tick_id" }
                span.dp-status-value {
                    @match snap.latest_tick_id {
                        Some(id) => (id),
                        None => span.muted { "—" },
                    }
                }
            }
            div.dp-status-row {
                span.dp-status-label { "Latest CRC" }
                span.dp-status-value {
                    @match snap.latest_crc {
                        Some(crc) => (format!("0x{crc:08X}")),
                        None => span.muted { "—" },
                    }
                }
            }
            div.dp-status-row {
                span.dp-status-label { "Latest Ia (raw)" }
                span.dp-status-value {
                    @match snap.latest_ch0_value {
                        Some(v) => (v),
                        None => span.muted { "—" },
                    }
                }
            }
            div.dp-status-row.dp-status-integrity {
                span.dp-status-label { "Integrity" }
                span.dp-status-value.{(integrity_class)} {
                    @if snap.integrity_violations == 0 {
                        "ok (verify_all = 0)"
                    } @else {
                        (format!("degraded — {} violation(s)", snap.integrity_violations))
                        @if !snap.violation_tick_ids.is_empty() {
                            span.muted {
                                " @ tick_id ["
                                @for (i, id) in snap.violation_tick_ids.iter().enumerate() {
                                    @if i > 0 { ", " }
                                    (id)
                                }
                                "]"
                            }
                        }
                    }
                }
            }
            div.dp-status-row {
                span.dp-status-label { "Tampers triggered" }
                span.dp-status-value { (snap.tamper_count) }
            }
            div.dp-status-row {
                span.dp-status-label { "Historian CSV" }
                span.dp-status-value {
                    @if snap.historian_exists {
                        a href="/api/dataplane/historian.csv" {
                            (snap.historian_path)
                        }
                    } @else {
                        span.muted { (snap.historian_path) " (not yet created)" }
                    }
                }
            }
        }
    }
}

fn svdc_console_dataplane_const_buffer() -> usize {
    crate::dataplane::DEMO_BUFFER_CAPACITY
}

const POLL_JS: &str = r#"
<style>
.dp-controls { display: flex; gap: 12px; flex-wrap: wrap; margin: 16px 0; }
.dp-controls .btn { padding: 8px 16px; border: 1px solid #c0c4cd; background: #fff; cursor: pointer; border-radius: 4px; font-family: inherit; font-size: 14px; }
.dp-controls .btn:hover { background: #f4f6fa; }
.dp-controls .btn-warning { border-color: #b4541f; color: #b4541f; }
.dp-controls .btn-muted { color: #6b6f7a; }
.dp-controls .btn-link { text-decoration: none; color: #0b1f3a; display: inline-block; line-height: 1.6; }
.dp-status { border: 1px solid #d8dce4; border-radius: 4px; padding: 14px 18px; background: #fafbfd; font-variant-numeric: tabular-nums; }
.dp-status.running { border-color: #2f8f4d; box-shadow: 0 0 0 1px rgba(47,143,77,0.15) inset; }
.dp-status.idle    { border-color: #c8cbd2; }
.dp-status-row { display: flex; justify-content: space-between; padding: 4px 0; border-bottom: 1px dashed #e6e8ec; gap: 24px; }
.dp-status-row:last-child { border-bottom: none; }
.dp-status-label { color: #6b6f7a; }
.dp-status-value { font-family: 'IBM Plex Mono', monospace; }
.dp-status-value.ok { color: #2f8f4d; }
.dp-status-value.degraded { color: #b4541f; font-weight: 600; }
.dp-status-integrity { margin-top: 6px; padding-top: 8px; border-top: 1px solid #e0e3e9; }
.dp-vendor { border: 1px solid #d8dce4; border-radius: 4px; padding: 14px 18px; background: #fff; margin: 12px 0; }
.dp-vendor-head h3 { margin: 0 0 6px 0; font-size: 14px; }
.dp-vendor form { display: flex; align-items: center; gap: 10px; margin: 10px 0; }
.dp-vendor-select { padding: 6px 10px; border: 1px solid #c0c4cd; border-radius: 4px; font-family: inherit; font-size: 13px; min-width: 360px; }
.dp-vendor-cmd { margin-top: 12px; }
.dp-vendor-cmd pre { background: #0b1f3a; color: #f7f4ed; padding: 12px 16px; border-radius: 4px; font-family: 'IBM Plex Mono', monospace; font-size: 12px; line-height: 1.5; overflow-x: auto; }
</style>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_snapshot() -> DataPipelineSnapshot {
        // Constructed directly so tests don't race on the global
        // pipeline singleton.
        DataPipelineSnapshot {
            running: false,
            buffer_len: 0,
            buffer_capacity: crate::dataplane::DEMO_BUFFER_CAPACITY,
            ticks_emitted: 0,
            tamper_count: 0,
            integrity_violations: 0,
            violation_tick_ids: Vec::new(),
            latest_tick_id: None,
            latest_ts_ns: None,
            latest_crc: None,
            latest_ch0_value: None,
            historian_path: "/tmp/none.csv".to_string(),
            historian_exists: false,
        }
    }

    #[test]
    fn router_constructs() {
        let _ = router();
    }

    #[test]
    fn status_panel_renders_idle_state() {
        let html = status_panel(&fresh_snapshot()).into_string();
        assert!(html.contains("dp-status idle"));
        assert!(html.contains("Pipeline"));
        assert!(html.contains("ok (verify_all = 0)"));
    }

    #[test]
    fn status_panel_marks_running_with_running_class() {
        let mut snap = fresh_snapshot();
        snap.running = true;
        snap.buffer_len = 5;
        snap.ticks_emitted = 5;
        let html = status_panel(&snap).into_string();
        assert!(html.contains("dp-status running"));
        assert!(html.contains(">running<"));
        assert!(html.contains("5 / 256"));
    }

    #[test]
    fn status_panel_marks_integrity_degraded_with_violation_tick_ids() {
        let mut snap = fresh_snapshot();
        snap.integrity_violations = 1;
        snap.violation_tick_ids = vec![1_000_000_001];
        snap.tamper_count = 1;
        let html = status_panel(&snap).into_string();
        assert!(html.contains("degraded"));
        assert!(html.contains("1_000_000_001") || html.contains("1000000001"));
    }
}
