//! `GET /north` and `GET /north/:layer` — northbound application layers.
//!
//! Honest table view of the four ADR-0016 layers. Only L0 is wired
//! today (PR H); L1/L2/L3 ship as "Planned (Phase 4)" cards. The
//! page does not invent traffic — every row pulls from
//! [`crate::dataplane::global`] or admits to being a stub.
//!
//! L0 is enabled by starting `svdc-bin --enable-l0-demo`. The flag
//! is read once at boot and cannot be toggled from the UI, so the
//! prior `/api/north/:layer/{enable,disable}` endpoints have been
//! removed. The `AuditEvent::NorthboundStateChange` variant is
//! kept (audit logs may already contain records of it) but is no
//! longer emitted.
//!
//! OWNER: claude-code. NFR-10: English-only.

use axum::extract::Path;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use maud::{html, Markup, PreEscaped};
use serde::{Deserialize, Serialize};

use crate::dataplane::DataPipeline;
use crate::templates::base::{layout, Section};

/// Northbound layer identifier per SDD §8.2 and ADR-0016.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layer {
    /// In-process subscriber (Rust API + C ABI). Wired in PR H.
    L0,
    /// OPC UA server for SCADA / HMI integration. Phase 4.
    L1,
    /// MQTT publisher for cloud / analytics fan-out. Phase 4.
    L2,
    /// TimescaleDB sidecar historian. Phase 4.
    L3,
}

impl Layer {
    fn from_str(s: &str) -> Option<Layer> {
        match s {
            "L0" | "l0" => Some(Layer::L0),
            "L1" | "l1" => Some(Layer::L1),
            "L2" | "l2" => Some(Layer::L2),
            "L3" | "l3" => Some(Layer::L3),
            _ => None,
        }
    }

    fn all() -> &'static [Layer] {
        &[Layer::L0, Layer::L1, Layer::L2, Layer::L3]
    }

    /// Code (used in URLs and JSON).
    pub fn code(self) -> &'static str {
        match self {
            Layer::L0 => "L0",
            Layer::L1 => "L1",
            Layer::L2 => "L2",
            Layer::L3 => "L3",
        }
    }

    /// Human-readable adapter name.
    pub fn name(self) -> &'static str {
        match self {
            Layer::L0 => "In-process subscriber",
            Layer::L1 => "OPC UA server",
            Layer::L2 => "MQTT publisher",
            Layer::L3 => "TimescaleDB historian",
        }
    }

    /// Transport / protocol summary, shown beneath the adapter name.
    pub fn protocol(self) -> &'static str {
        match self {
            Layer::L0 => "Rust API + C ABI (in-process)",
            Layer::L1 => "OPC UA over TCP (IEC 62541)",
            Layer::L2 => "MQTT over TCP (OASIS 5.0)",
            Layer::L3 => "PostgreSQL wire protocol",
        }
    }

    /// One-line purpose, shown on the detail page.
    pub fn purpose(self) -> &'static str {
        match self {
            Layer::L0 => {
                "Zero-network-hop tick subscription for EBP relays, QSE, and \
                 phasor computation modules running on the same node."
            }
            Layer::L1 => {
                "IEC 61850 ↔ OPC UA bridge per OPC 10040 for SCADA, HMI, and \
                 engineering workstations."
            }
            Layer::L2 => {
                "Subsampled telemetry fan-out for cloud analytics and \
                 substation-fleet management."
            }
            Layer::L3 => {
                "Time-series historian + replay backing post-event analysis \
                 and audit queries."
            }
        }
    }

    /// Endpoint string. For L0 this is the runtime command, not a
    /// network address — the subscriber is in-process by SDD §8.2.
    pub fn endpoint(self) -> &'static str {
        match self {
            Layer::L0 => "svdc-bin --enable-l0-demo",
            Layer::L1 => "opc.tcp://<host>:4840/svdc/server (planned)",
            Layer::L2 => "mqtt://<broker>:1883/svdc/... (planned)",
            Layer::L3 => "postgres://<host>:5432/svdc_historian (planned)",
        }
    }

    /// Implementation maturity. Read by both the index and detail
    /// pages so the badge text is in one place.
    pub fn maturity(self) -> Maturity {
        match self {
            Layer::L0 => Maturity::Wired,
            Layer::L1 | Layer::L2 | Layer::L3 => Maturity::Planned { phase: 4 },
        }
    }

    /// ADR that documents this adapter's design, if one exists.
    pub fn adr(self) -> Option<&'static str> {
        match self {
            Layer::L0 => Some("ADR-0010 (subscriber API), ADR-0016 (simulators)"),
            Layer::L1 => Some("ADR-0017 (planned)"),
            Layer::L2 => None,
            Layer::L3 => None,
        }
    }

    /// Runbook section anchor in `docs/northbound-simulators.md`.
    pub fn runbook_anchor(self) -> &'static str {
        match self {
            Layer::L0 => "#l0--in-process-consumer-wired-in-pr-h",
            Layer::L1 => "#l1--opc-ua-scada-client",
            Layer::L2 => "#l2--mqtt-cloud-subscriber",
            Layer::L3 => "#l3--historian-tsdb-query",
        }
    }
}

/// Implementation maturity for a [`Layer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Maturity {
    /// Adapter is shipped and observable from this UI.
    Wired,
    /// Adapter is on the roadmap; the detail page is a stub
    /// describing what will land.
    Planned {
        /// Project phase the adapter is slated for.
        phase: u8,
    },
}

/// Live counters for the L0 demo, read from the shared
/// [`DataPipeline`] each render.
#[derive(Debug, Clone, Copy)]
struct L0Status {
    /// Whether the subscriber task is currently running.
    active: bool,
    /// Highest tick_id consumed since start.
    last_tick_id: u64,
    /// Total ticks consumed since start.
    total_ticks: u64,
}

impl L0Status {
    fn from_pipeline(p: &DataPipeline) -> Self {
        Self {
            active: p.l0_demo_active(),
            last_tick_id: p.l0_demo_last_tick_id(),
            total_ticks: p.l0_demo_total_ticks(),
        }
    }
}

/// Build the northbound sub-router. No state — every render reads
/// from [`crate::dataplane::global`].
pub fn router() -> Router {
    Router::new()
        .route("/north", get(north_index))
        .route("/north/:layer", get(north_layer))
}

async fn north_index() -> impl IntoResponse {
    let pipeline = crate::dataplane::global();
    let l0 = L0Status::from_pipeline(pipeline.as_ref());

    layout(
        Section::Northbound,
        "Northbound application layers",
        html! {
            section.config-section {
                div.config-section-head {
                    h2 { "Northbound application layers" }
                    p.muted {
                        "Four adapters fan SV-aligned telemetry out of the \
                         SVDC core (SDD §8.2). Today only L0 is wired; the \
                         others ship as Phase 4 stubs. Click a row for the \
                         per-layer detail page."
                    }
                }
                table.layer-table {
                    thead {
                        tr {
                            th.col-code   { "Layer" }
                            th.col-name   { "Adapter" }
                            th            { "Endpoint" }
                            th.col-status { "Status" }
                            th.col-detail { "Live" }
                        }
                    }
                    tbody {
                        @for layer in Layer::all().iter().copied() {
                            @let href = format!("/north/{}", layer.code());
                            tr.layer-row data-layer=(layer.code()) data-href=(href) {
                                td.mono.col-code {
                                    span.layer-tag { (layer.code()) }
                                }
                                td {
                                    a.layer-link href=(href) { (layer.name()) }
                                    div.muted.small { (layer.protocol()) }
                                }
                                td.mono.endpoint-cell {
                                    code { (layer.endpoint()) }
                                }
                                td.col-status {
                                    (status_badge(layer, l0))
                                }
                                td.col-detail.mono.small {
                                    (live_summary(layer, l0))
                                }
                            }
                        }
                    }
                }
            }
            section.placeholder {
                p.muted {
                    "L0 counters are live — they reflect the running \
                     subscriber task spawned by "
                    code { "--enable-l0-demo" }
                    ". L1, L2, and L3 do not yet have adapter modules; their \
                     detail pages summarise what will land in Phase 4."
                }
            }
            script type="module" { (PreEscaped(ROW_CLICK_JS)) }
        },
    )
}

/// Render the maturity / runtime-state badge for a layer.
fn status_badge(layer: Layer, l0: L0Status) -> Markup {
    match layer.maturity() {
        Maturity::Wired => {
            if l0.active {
                html! { span.state-badge.state-on { "Wired · running" } }
            } else {
                html! { span.state-badge.state-off { "Wired · not started" } }
            }
        }
        Maturity::Planned { phase } => html! {
            span.state-badge.state-planned {
                "Planned (Phase " (phase) ")"
            }
        },
    }
}

/// One-cell "live" summary on the index row. L0 shows the cursor;
/// planned layers show an em-dash so the table stays aligned.
fn live_summary(layer: Layer, l0: L0Status) -> Markup {
    match layer {
        Layer::L0 if l0.active => html! {
            "tick_id=" (l0.last_tick_id) " · "
            (l0.total_ticks) " drained"
        },
        Layer::L0 => html! { span.muted { "idle" } },
        _ => html! { span.muted { "—" } },
    }
}

async fn north_layer(Path(layer_code): Path<String>) -> impl IntoResponse {
    let pipeline = crate::dataplane::global();
    let l0 = L0Status::from_pipeline(pipeline.as_ref());

    match Layer::from_str(&layer_code) {
        Some(layer) => layout(
            Section::Northbound,
            &format!("{} — {}", layer.code(), layer.name()),
            layer_detail_body(layer, l0),
        ),
        None => layout(
            Section::Northbound,
            "Unknown layer",
            html! {
                section.placeholder {
                    h2 { "Unknown layer" }
                    p.muted { "Expected one of L0, L1, L2, L3." }
                    a.btn-secondary href="/north" { "← All layers" }
                }
            },
        ),
    }
}

fn layer_detail_body(layer: Layer, l0: L0Status) -> Markup {
    let detail = match layer.maturity() {
        Maturity::Wired => wired_l0_body(layer, l0),
        Maturity::Planned { phase } => planned_body(layer, phase),
    };
    html! {
        section.config-section {
            div.config-section-head {
                div.layer-detail-head {
                    div {
                        h2 { (layer.code()) " · " (layer.name()) }
                        p.muted { (layer.purpose()) }
                    }
                    div.layer-detail-actions {
                        a.btn-secondary href="/north" { "← All layers" }
                    }
                }
            }
            (detail)
        }
    }
}

/// Detail body for L0 — pulls real counters from the dataplane.
fn wired_l0_body(layer: Layer, l0: L0Status) -> Markup {
    html! {
        table.layer-table {
            tbody {
                tr {
                    th { "Status" }
                    td { (status_badge(layer, l0)) }
                }
                tr {
                    th { "Protocol" }
                    td { (layer.protocol()) }
                }
                tr {
                    th { "Activation" }
                    td.mono { code { (layer.endpoint()) } }
                }
                tr {
                    th { "Last tick_id consumed" }
                    td.mono {
                        @if l0.active {
                            (l0.last_tick_id)
                        } @else {
                            span.muted { "—" }
                        }
                    }
                }
                tr {
                    th { "Total ticks drained" }
                    td.mono { (l0.total_ticks) }
                }
                tr {
                    th { "ADR" }
                    td.mono.small { (layer.adr().unwrap_or("—")) }
                }
            }
        }
        @if !l0.active {
            section.placeholder {
                h3 { "How to enable L0" }
                p.muted {
                    "The L0 reference subscriber is enabled at daemon boot. \
                     Start svdc-bin with the demo flag to observe live ticks \
                     on this page and on stdout:"
                }
                pre.codeblock { code {
                    "cargo run --release -p svdc-bin -- \\\n"
                    "    --ingress-udp 239.0.0.1:9100 \\\n"
                    "    --enable-l0-demo"
                } }
                p.muted {
                    "Pair with a southbound simulator on the same multicast \
                     group (see "
                    a href="/north" { "docs/northbound-simulators.md" }
                    "). Counters above refresh on page reload."
                }
            }
        }
    }
}

/// Detail body for L1/L2/L3 — a single "Planned (Phase N)" card.
/// No mock metrics, no toggle, no save buttons. Production adapter
/// modules will replace this when they land.
fn planned_body(layer: Layer, phase: u8) -> Markup {
    html! {
        section.placeholder {
            div.placeholder-head {
                span.state-badge.state-planned {
                    "Planned (Phase " (phase) ")"
                }
            }
            h3 { (layer.name()) }
            p { (layer.purpose()) }
            table.layer-table.layer-table-compact {
                tbody {
                    tr {
                        th { "Protocol" }
                        td { (layer.protocol()) }
                    }
                    tr {
                        th { "Target endpoint" }
                        td.mono { code { (layer.endpoint()) } }
                    }
                    tr {
                        th { "ADR" }
                        td.mono.small {
                            (layer.adr().unwrap_or("not yet authored"))
                        }
                    }
                    tr {
                        th { "Runbook section" }
                        td.mono.small {
                            "docs/northbound-simulators.md"
                            (layer.runbook_anchor())
                        }
                    }
                }
            }
            p.muted.small {
                "No adapter module is wired today. This page is a stub; the \
                 control surface (endpoint binding, credentials, throttling) \
                 will land alongside the adapter implementation in its own \
                 crate."
            }
        }
    }
}

const ROW_CLICK_JS: &str = r#"
document.querySelectorAll('tr.layer-row[data-href]').forEach((row) => {
  row.addEventListener('click', (e) => {
    if (e.target.closest('a, button, input, label')) return;
    const href = row.getAttribute('data-href');
    if (href) window.location.href = href;
  });
  row.setAttribute('role', 'link');
  row.setAttribute('tabindex', '0');
  row.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      const href = row.getAttribute('data-href');
      if (href) window.location.href = href;
    }
  });
});
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_constructs() {
        let _ = router();
    }

    #[test]
    fn layer_from_str_round_trip() {
        for l in Layer::all() {
            assert_eq!(Layer::from_str(l.code()), Some(*l));
        }
        assert_eq!(Layer::from_str("L9"), None);
    }

    #[test]
    fn each_layer_has_distinct_endpoint() {
        let mut eps: Vec<&'static str> = Layer::all().iter().map(|l| l.endpoint()).collect();
        eps.sort();
        eps.dedup();
        assert_eq!(eps.len(), 4, "endpoints must be distinct");
    }

    #[test]
    fn only_l0_is_wired() {
        assert_eq!(Layer::L0.maturity(), Maturity::Wired);
        for l in [Layer::L1, Layer::L2, Layer::L3] {
            assert!(matches!(l.maturity(), Maturity::Planned { phase: 4 }));
        }
    }

    #[test]
    fn status_badge_reflects_l0_runtime_state() {
        let running = L0Status {
            active: true,
            last_tick_id: 123,
            total_ticks: 456,
        };
        let idle = L0Status {
            active: false,
            last_tick_id: 0,
            total_ticks: 0,
        };
        let on = status_badge(Layer::L0, running).into_string();
        assert!(on.contains("Wired"));
        assert!(on.contains("running"));
        let off = status_badge(Layer::L0, idle).into_string();
        assert!(off.contains("Wired"));
        assert!(off.contains("not started"));
        let planned = status_badge(Layer::L1, idle).into_string();
        assert!(planned.contains("Planned"));
        assert!(planned.contains("Phase 4"));
    }

    #[test]
    fn live_summary_shows_tick_id_when_l0_active() {
        let running = L0Status {
            active: true,
            last_tick_id: 999,
            total_ticks: 4800,
        };
        let s = live_summary(Layer::L0, running).into_string();
        assert!(s.contains("999"));
        assert!(s.contains("4800"));
        assert!(s.contains("drained"));
    }

    #[test]
    fn live_summary_for_planned_layers_is_em_dash() {
        let idle = L0Status {
            active: false,
            last_tick_id: 0,
            total_ticks: 0,
        };
        for l in [Layer::L1, Layer::L2, Layer::L3] {
            let s = live_summary(l, idle).into_string();
            assert!(s.contains("—"), "layer {} missing em-dash", l.code());
        }
    }

    #[test]
    fn wired_l0_body_shows_activation_command() {
        let idle = L0Status {
            active: false,
            last_tick_id: 0,
            total_ticks: 0,
        };
        let body = wired_l0_body(Layer::L0, idle).into_string();
        assert!(body.contains("--enable-l0-demo"));
        assert!(body.contains("How to enable"));
    }

    #[test]
    fn wired_l0_body_hides_activation_help_when_running() {
        let running = L0Status {
            active: true,
            last_tick_id: 1,
            total_ticks: 1,
        };
        let body = wired_l0_body(Layer::L0, running).into_string();
        assert!(!body.contains("How to enable"));
    }

    #[test]
    fn planned_body_does_not_pretend_to_be_active() {
        let body = planned_body(Layer::L1, 4).into_string();
        assert!(body.contains("Planned"));
        assert!(body.contains("Phase 4"));
        assert!(!body.contains("running"));
        assert!(!body.contains("Disable"));
        assert!(!body.contains("Enable"));
    }
}
