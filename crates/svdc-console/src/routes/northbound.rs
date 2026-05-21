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
            Layer::L1 => "svdc-bin --enable-opcua 127.0.0.1:4840",
            Layer::L2 => "mqtt://<broker>:1883/svdc/... (planned)",
            Layer::L3 => "postgres://<host>:5432/svdc_historian (planned)",
        }
    }

    /// Implementation maturity. Read by both the index and detail
    /// pages so the badge text is in one place.
    pub fn maturity(self) -> Maturity {
        match self {
            Layer::L0 | Layer::L1 => Maturity::Wired,
            Layer::L2 | Layer::L3 => Maturity::Planned { phase: 4 },
        }
    }

    /// ADR that documents this adapter's design, if one exists.
    pub fn adr(self) -> Option<&'static str> {
        match self {
            Layer::L0 => Some("ADR-0010 (subscriber API), ADR-0016 (simulators)"),
            Layer::L1 => Some("ADR-0017 (server library + address space)"),
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

/// Live counters for a wired layer, read from the shared
/// [`DataPipeline`] each render. Two flavours today: L0 (the
/// in-process subscriber demo) and L1 (the OPC UA server, stubbed
/// in PR L and wired for real in PR L+).
#[derive(Debug, Clone, Copy)]
struct LiveStatus {
    /// Whether the corresponding daemon task is currently running.
    active: bool,
    /// Highest tick_id observed by the layer. Zero before the
    /// first activity or when the task is not running.
    last_tick_id: u64,
    /// Total work units the layer has completed since boot. For
    /// L0 this is ticks drained; for L1 it is OPC UA publishes.
    total_count: u64,
    /// True when the layer reports `active = true` but
    /// `total_count = 0` — i.e. enabled but not yet emitting.
    /// PR L's L1 stub sits here permanently; PR L+'s real server
    /// will flip to `is_stub = false` after the first publish.
    is_stub: bool,
}

impl LiveStatus {
    fn l0_from_pipeline(p: &DataPipeline) -> Self {
        // L0 has no stub mode — when it's active, it's actually
        // draining. Counter delay before first read is a normal
        // "no traffic yet" condition, not a half-implementation.
        Self {
            active: p.l0_demo_active(),
            last_tick_id: p.l0_demo_last_tick_id(),
            total_count: p.l0_demo_total_ticks(),
            is_stub: false,
        }
    }

    fn l1_from_pipeline(p: &DataPipeline) -> Self {
        let active = p.l1_opcua_active();
        let publishes = p.l1_opcua_total_publishes();
        Self {
            active,
            last_tick_id: p.l1_opcua_last_tick_id(),
            total_count: publishes,
            is_stub: active && publishes == 0,
        }
    }

    /// Layer-agnostic accessor used by `north_index` so the table
    /// row picks the right counters for each layer.
    fn for_layer(layer: Layer, p: &DataPipeline) -> Option<Self> {
        match layer {
            Layer::L0 => Some(Self::l0_from_pipeline(p)),
            Layer::L1 => Some(Self::l1_from_pipeline(p)),
            Layer::L2 | Layer::L3 => None,
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

    layout(
        Section::Northbound,
        "Northbound application layers",
        html! {
            section.config-section {
                div.config-section-head {
                    h2 { "Northbound application layers" }
                    p.muted {
                        "Four adapters fan SV-aligned telemetry out of the \
                         SVDC core (SDD §8.2). L0 (in-process subscriber) \
                         and L1 (OPC UA server) are wired; L2 and L3 ship as \
                         Phase 4 stubs. Click a row for the per-layer detail \
                         page."
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
                            @let status = LiveStatus::for_layer(layer, pipeline.as_ref());
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
                                    (status_badge(layer, status))
                                }
                                td.col-detail.mono.small {
                                    (live_summary(layer, status))
                                }
                            }
                        }
                    }
                }
            }
            section.placeholder {
                p.muted {
                    "L0 / L1 counters are live — they reflect the running \
                     daemon tasks spawned by "
                    code { "--enable-l0-demo" }
                    " / "
                    code { "--enable-opcua" }
                    ". L2 and L3 do not yet have adapter modules; their \
                     detail pages summarise what will land in Phase 4."
                }
            }
            script type="module" { (PreEscaped(ROW_CLICK_JS)) }
        },
    )
}

/// Render the maturity / runtime-state badge for a layer.
fn status_badge(layer: Layer, status: Option<LiveStatus>) -> Markup {
    match (layer.maturity(), status) {
        (Maturity::Wired, Some(s)) if s.is_stub => html! {
            span.state-badge.state-stub { "Wired · stub mode" }
        },
        (Maturity::Wired, Some(s)) if s.active => html! {
            span.state-badge.state-on { "Wired · running" }
        },
        (Maturity::Wired, _) => html! {
            span.state-badge.state-off { "Wired · not started" }
        },
        (Maturity::Planned { phase }, _) => html! {
            span.state-badge.state-planned {
                "Planned (Phase " (phase) ")"
            }
        },
    }
}

/// One-cell "live" summary on the index row. Wired layers show
/// their cursor; stub mode shows the stub label; planned layers
/// show an em-dash so the table stays aligned.
fn live_summary(layer: Layer, status: Option<LiveStatus>) -> Markup {
    match (layer, status) {
        (_, Some(s)) if s.is_stub => html! {
            span.muted { "stub · no publishes" }
        },
        (_, Some(s)) if s.active => html! {
            "tick_id=" (s.last_tick_id) " · "
            (s.total_count) " " (work_unit_label(layer))
        },
        (Layer::L0 | Layer::L1, _) => html! { span.muted { "idle" } },
        _ => html! { span.muted { "—" } },
    }
}

/// Per-layer label for the running counter. L0 drains ticks; L1
/// publishes OPC UA variable updates. Keeping the label in one
/// place makes the live cell explain itself without a legend.
fn work_unit_label(layer: Layer) -> &'static str {
    match layer {
        Layer::L0 => "drained",
        Layer::L1 => "published",
        Layer::L2 | Layer::L3 => "",
    }
}

async fn north_layer(Path(layer_code): Path<String>) -> impl IntoResponse {
    let pipeline = crate::dataplane::global();

    match Layer::from_str(&layer_code) {
        Some(layer) => {
            let status = LiveStatus::for_layer(layer, pipeline.as_ref());
            layout(
                Section::Northbound,
                &format!("{} — {}", layer.code(), layer.name()),
                layer_detail_body(layer, status),
            )
        }
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

fn layer_detail_body(layer: Layer, status: Option<LiveStatus>) -> Markup {
    let detail = match layer.maturity() {
        Maturity::Wired => wired_body(
            layer,
            status.unwrap_or(LiveStatus {
                active: false,
                last_tick_id: 0,
                total_count: 0,
                is_stub: false,
            }),
        ),
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

/// Detail body for any Wired layer — pulls real counters from the
/// dataplane. Activation hint adapts per layer.
fn wired_body(layer: Layer, status: LiveStatus) -> Markup {
    html! {
        table.layer-table {
            tbody {
                tr {
                    th { "Status" }
                    td { (status_badge(layer, Some(status))) }
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
                    th { (last_tick_label(layer)) }
                    td.mono {
                        @if status.active && !status.is_stub {
                            (status.last_tick_id)
                        } @else {
                            span.muted { "—" }
                        }
                    }
                }
                tr {
                    th { (total_count_label(layer)) }
                    td.mono { (status.total_count) }
                }
                tr {
                    th { "ADR" }
                    td.mono.small { (layer.adr().unwrap_or("—")) }
                }
            }
        }
        @if status.is_stub {
            section.placeholder {
                h3 { "Stub mode" }
                p.muted {
                    "The L1 OPC UA server task is enabled but no real OPC UA \
                     stack is running yet (PR L lands the CLI flag + UI; PR L+ \
                     lands the server itself). See "
                    code { "docs/decisions/0017-l1-opcua-server.md" }
                    " §1 follow-up for the openssl-sys / async-opcua library \
                     evaluation that gates the real server."
                }
            }
        } @else if !status.active {
            section.placeholder {
                h3 { (activation_header(layer)) }
                p.muted { (activation_hint(layer)) }
                pre.codeblock { code { (activation_cmd(layer)) } }
                p.muted {
                    "Counters above refresh on page reload."
                }
            }
        }
    }
}

fn last_tick_label(layer: Layer) -> &'static str {
    match layer {
        Layer::L0 => "Last tick_id consumed",
        Layer::L1 => "Last tick_id published",
        Layer::L2 | Layer::L3 => "Last tick_id",
    }
}

fn total_count_label(layer: Layer) -> &'static str {
    match layer {
        Layer::L0 => "Total ticks drained",
        Layer::L1 => "Total publishes",
        Layer::L2 | Layer::L3 => "Total",
    }
}

fn activation_header(layer: Layer) -> &'static str {
    match layer {
        Layer::L0 => "How to enable L0",
        Layer::L1 => "How to enable L1",
        Layer::L2 | Layer::L3 => "How to enable",
    }
}

fn activation_hint(layer: Layer) -> &'static str {
    match layer {
        Layer::L0 => {
            "The L0 reference subscriber is enabled at daemon boot. Start \
             svdc-bin with the demo flag to observe live ticks on this page \
             and on stdout:"
        }
        Layer::L1 => {
            "The L1 OPC UA server is enabled at daemon boot. Start svdc-bin \
             with --enable-opcua to claim the layer; the underlying server \
             implementation ships in PR L+ (see ADR-0017 §1):"
        }
        Layer::L2 | Layer::L3 => "Not yet implemented.",
    }
}

fn activation_cmd(layer: Layer) -> &'static str {
    match layer {
        Layer::L0 => {
            "cargo run --release -p svdc-bin -- \\\n    --ingress-udp 239.0.0.1:9100 \\\n    --enable-l0-demo"
        }
        Layer::L1 => {
            "cargo run --release -p svdc-bin -- \\\n    --ingress-udp 239.0.0.1:9100 \\\n    --enable-opcua 127.0.0.1:4840"
        }
        Layer::L2 | Layer::L3 => "(no command yet)",
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

    fn live(active: bool, last_tick_id: u64, total_count: u64, is_stub: bool) -> LiveStatus {
        LiveStatus {
            active,
            last_tick_id,
            total_count,
            is_stub,
        }
    }

    #[test]
    fn l0_and_l1_are_wired_l2_l3_are_planned() {
        for w in [Layer::L0, Layer::L1] {
            assert_eq!(w.maturity(), Maturity::Wired);
        }
        for p in [Layer::L2, Layer::L3] {
            assert!(matches!(p.maturity(), Maturity::Planned { phase: 4 }));
        }
    }

    #[test]
    fn status_badge_reflects_wired_runtime_state() {
        let running = live(true, 123, 456, false);
        let idle = live(false, 0, 0, false);
        let stub = live(true, 0, 0, true);

        let on = status_badge(Layer::L0, Some(running)).into_string();
        assert!(on.contains("Wired"));
        assert!(on.contains("running"));

        let off = status_badge(Layer::L0, Some(idle)).into_string();
        assert!(off.contains("Wired"));
        assert!(off.contains("not started"));

        let stub_html = status_badge(Layer::L1, Some(stub)).into_string();
        assert!(stub_html.contains("stub mode"));
        assert!(!stub_html.contains("running"));

        let planned = status_badge(Layer::L2, None).into_string();
        assert!(planned.contains("Planned"));
        assert!(planned.contains("Phase 4"));
    }

    #[test]
    fn live_summary_per_layer_uses_correct_work_unit() {
        let running_l0 = live(true, 999, 4800, false);
        let summary_l0 = live_summary(Layer::L0, Some(running_l0)).into_string();
        assert!(summary_l0.contains("4800"));
        assert!(summary_l0.contains("drained"));

        let running_l1 = live(true, 480, 48, false);
        let summary_l1 = live_summary(Layer::L1, Some(running_l1)).into_string();
        assert!(summary_l1.contains("48"));
        assert!(summary_l1.contains("published"));
    }

    #[test]
    fn live_summary_for_planned_layers_is_em_dash() {
        for l in [Layer::L2, Layer::L3] {
            let s = live_summary(l, None).into_string();
            assert!(s.contains("—"), "layer {} missing em-dash", l.code());
        }
    }

    #[test]
    fn live_summary_stub_mode_is_clear_about_no_publishes() {
        let stub = live(true, 0, 0, true);
        let s = live_summary(Layer::L1, Some(stub)).into_string();
        assert!(s.contains("stub"));
        assert!(s.contains("no publishes"));
    }

    #[test]
    fn wired_body_shows_l0_activation_command_when_idle() {
        let idle = live(false, 0, 0, false);
        let body = wired_body(Layer::L0, idle).into_string();
        assert!(body.contains("--enable-l0-demo"));
        assert!(body.contains("How to enable"));
    }

    #[test]
    fn wired_body_shows_l1_activation_command_when_idle() {
        let idle = live(false, 0, 0, false);
        let body = wired_body(Layer::L1, idle).into_string();
        assert!(body.contains("--enable-opcua"));
        assert!(body.contains("How to enable"));
    }

    #[test]
    fn wired_body_hides_activation_help_when_running() {
        let running = live(true, 1, 1, false);
        let body = wired_body(Layer::L0, running).into_string();
        assert!(!body.contains("How to enable"));
    }

    #[test]
    fn wired_body_shows_stub_disclosure_for_l1_in_stub_mode() {
        let stub = live(true, 0, 0, true);
        let body = wired_body(Layer::L1, stub).into_string();
        assert!(body.contains("Stub mode"));
        assert!(body.contains("PR L+"));
        assert!(body.contains("openssl-sys"));
        // Activation help is suppressed while stub disclosure is shown.
        assert!(!body.contains("How to enable"));
    }

    #[test]
    fn planned_body_does_not_pretend_to_be_active() {
        let body = planned_body(Layer::L2, 4).into_string();
        assert!(body.contains("Planned"));
        assert!(body.contains("Phase 4"));
        assert!(!body.contains("running"));
        assert!(!body.contains("Disable"));
        assert!(!body.contains("Enable"));
    }
}
