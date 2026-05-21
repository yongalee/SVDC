//! `GET /north` and `GET /north/:layer` — North-bound application layers.
//!
//! Industrial-grade table view of the four layers (L0/L1/L2/L3) with
//! per-row endpoint, active-receivers count, throughput, and a state
//! toggle. Each row links to the per-layer detail page.
//!
//! The layer-level enable/disable API (`POST /api/north/:layer/...`)
//! is authored here (WBS-9.4a). Per-adapter detail (the actual L0/L1
//! handler implementation) lands in Phase 4 as the adapters land in
//! their own crates; until then, mock_metrics() provides plausible
//! display values so the operator UI looks alive end-to-end.

use std::sync::{Arc, RwLock};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use maud::{html, Markup, PreEscaped};
use serde::{Deserialize, Serialize};

use crate::audit::{self, AuditEvent};
use crate::templates::base::{layout, Section};

/// Northbound layer identifier per UI Doc §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layer {
    /// In-process C ABI for EBP relays / QSE / phasor computation.
    L0,
    /// OPC UA Server (SCADA, HMI).
    L1,
    /// MQTT publisher (analytics, cloud).
    L2,
    /// TimescaleDB sidecar (historian).
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

    /// Human-readable name for UI display.
    pub fn name(self) -> &'static str {
        match self {
            Layer::L0 => "Shared Memory RingBuffer",
            Layer::L1 => "OPC UA Server",
            Layer::L2 => "MQTT Publisher",
            Layer::L3 => "TimescaleDB Historian",
        }
    }

    /// Protocol family / transport summary, shown in the table.
    pub fn protocol(self) -> &'static str {
        match self {
            Layer::L0 => "Shared memory / C ABI",
            Layer::L1 => "OPC UA over TCP (per OPC 10040)",
            Layer::L2 => "MQTT over TCP",
            Layer::L3 => "PostgreSQL wire protocol",
        }
    }

    /// One-line purpose, shown on the detail page.
    pub fn purpose(self) -> &'static str {
        match self {
            Layer::L0 => "Sub-ms feed to EBP relays, QSE, and phasor computation.",
            Layer::L1 => "IEC 61850 ↔ OPC UA per OPC 10040 for SCADA / HMI.",
            Layer::L2 => "Cloud / analytics fan-out at configurable cadence.",
            Layer::L3 => "Historian + replay for audit and post-event analysis.",
        }
    }

    /// Default endpoint string. Phase 4 replaces this with a value
    /// read from the SVDC config; until then it is the reference
    /// deployment default.
    pub fn endpoint(self) -> &'static str {
        match self {
            Layer::L0 => "/dev/shm/svdc_l0_ring",
            Layer::L1 => "opc.tcp://127.0.0.1:4840/svdc/server",
            Layer::L2 => "mqtt://broker.local:1883/svdc",
            Layer::L3 => "postgres://svdc@127.0.0.1:5432/svdc_historian",
        }
    }
}

/// Mock per-layer runtime metrics shown until Phase 4 wires the real
/// adapter counters in. Deterministic so the table looks consistent
/// across page reloads.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct LayerMetrics {
    /// Number of subscribers currently attached to this layer.
    pub active_receivers: u32,
    /// Frames per second flowing through this adapter.
    pub throughput_fps: u32,
}

impl LayerMetrics {
    /// Mock value for `layer`. Reasonable for the reference 4800 Hz
    /// SV stream so the operator UI matches what they would see on a
    /// real deployment.
    pub fn mock(layer: Layer) -> Self {
        match layer {
            Layer::L0 => Self {
                active_receivers: 3,
                throughput_fps: 4800,
            },
            Layer::L1 => Self {
                active_receivers: 2,
                throughput_fps: 4800,
            },
            Layer::L2 => Self {
                active_receivers: 1,
                throughput_fps: 60,
            },
            Layer::L3 => Self {
                active_receivers: 1,
                throughput_fps: 4800,
            },
        }
    }
}

/// In-memory state of the four northbound layers.
///
/// Replaced by real adapter integration in Phase 4. Until then,
/// enable/disable flips this flag, audit-logs via `tracing`, and
/// returns the new state.
#[derive(Debug)]
pub struct NorthboundState {
    l0: RwLock<bool>,
    l1: RwLock<bool>,
    l2: RwLock<bool>,
    l3: RwLock<bool>,
}

impl NorthboundState {
    /// Initial state: all layers disabled until explicitly enabled by
    /// the operator. Phase 4 may flip the defaults after deployment
    /// experience.
    pub fn new() -> Self {
        Self {
            l0: RwLock::new(false),
            l1: RwLock::new(false),
            l2: RwLock::new(false),
            l3: RwLock::new(false),
        }
    }

    /// Read the enabled flag for `layer`.
    pub fn is_enabled(&self, layer: Layer) -> bool {
        let cell = match layer {
            Layer::L0 => &self.l0,
            Layer::L1 => &self.l1,
            Layer::L2 => &self.l2,
            Layer::L3 => &self.l3,
        };
        cell.read().map(|g| *g).unwrap_or(false)
    }

    /// Set `enabled` for `layer`. Returns the previous value.
    pub fn set(&self, layer: Layer, enabled: bool) -> bool {
        let cell = match layer {
            Layer::L0 => &self.l0,
            Layer::L1 => &self.l1,
            Layer::L2 => &self.l2,
            Layer::L3 => &self.l3,
        };
        let mut guard = cell.write().expect("northbound state lock poisoned");
        let prev = *guard;
        *guard = enabled;
        prev
    }
}

impl Default for NorthboundState {
    fn default() -> Self {
        Self::new()
    }
}

/// State alias used by the axum handlers.
pub type AppState = Arc<NorthboundState>;

/// Build the North-bound sub-router with shared state.
pub fn router() -> Router {
    let state: AppState = Arc::new(NorthboundState::new());
    Router::new()
        .route("/north", get(north_index))
        .route("/north/:layer", get(north_layer))
        .route("/api/north/:layer/enable", post(api_enable))
        .route("/api/north/:layer/disable", post(api_disable))
        .with_state(state)
}

async fn north_index(State(state): State<AppState>) -> Markup {
    let rows: Vec<(Layer, bool, LayerMetrics)> = Layer::all()
        .iter()
        .copied()
        .map(|l| (l, state.is_enabled(l), LayerMetrics::mock(l)))
        .collect();

    layout(
        Section::Northbound,
        "North-bound application layers",
        html! {
            section.config-section {
                div.config-section-head {
                    h2 { "North-bound application layers" }
                    p.muted {
                        "Four adapters fan data out from the SVDC core. Each row "
                        "shows the endpoint the adapter publishes on, the number "
                        "of consumers currently attached, and the rate at which "
                        "the adapter is forwarding frames. Click a row to drill "
                        "into the layer's detail page."
                    }
                }
                table.layer-table {
                    thead {
                        tr {
                            th.col-code   { "Layer" }
                            th.col-name   { "Adapter" }
                            th            { "Endpoint" }
                            th.col-rx     { "Receivers" }
                            th.col-fps    { "Throughput" }
                            th.col-state  { "State" }
                            th.col-actions { "Actions" }
                        }
                    }
                    tbody {
                        @for (layer, enabled, metrics) in &rows {
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
                                td.mono.col-rx { (metrics.active_receivers) }
                                td.mono.col-fps {
                                    (metrics.throughput_fps) " fps"
                                }
                                td.col-state {
                                    @if *enabled {
                                        span.state-badge.state-on { "Enabled" }
                                    } @else {
                                        span.state-badge.state-off { "Disabled" }
                                    }
                                }
                                td.col-actions {
                                    @if *enabled {
                                        button.btn-secondary
                                            type="button"
                                            data-layer=(layer.code())
                                            data-action="disable" { "Disable" }
                                    } @else {
                                        button.btn-primary
                                            type="button"
                                            data-layer=(layer.code())
                                            data-action="enable" { "Enable" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            section.placeholder {
                p.muted {
                    "Receivers and throughput are mock values until Phase 4 wires "
                    "the real adapter counters in. Enable/disable is wired today "
                    "via POST /api/north/:layer/{enable,disable}."
                }
            }
            script type="module" { (PreEscaped(LAYER_TOGGLE_JS)) }
            script type="module" { (PreEscaped(ROW_CLICK_JS)) }
        },
    )
}

async fn north_layer(State(state): State<AppState>, Path(layer_code): Path<String>) -> Markup {
    match Layer::from_str(&layer_code) {
        Some(layer) => {
            let enabled = state.is_enabled(layer);
            let metrics = LayerMetrics::mock(layer);
            layout(
                Section::Northbound,
                &format!("{} — {}", layer.code(), layer.name()),
                layer_detail_body(layer, enabled, metrics),
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

fn layer_detail_body(layer: Layer, enabled: bool, metrics: LayerMetrics) -> Markup {
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
                        @if enabled {
                            button.btn-secondary
                                type="button"
                                data-layer=(layer.code())
                                data-action="disable" { "Disable layer" }
                        } @else {
                            button.btn-primary
                                type="button"
                                data-layer=(layer.code())
                                data-action="enable" { "Enable layer" }
                        }
                    }
                }
            }
            table.layer-table {
                tbody {
                    tr {
                        th { "State" }
                        td {
                            @if enabled {
                                span.state-badge.state-on { "Enabled" }
                            } @else {
                                span.state-badge.state-off { "Disabled" }
                            }
                        }
                    }
                    tr {
                        th { "Protocol" }
                        td { (layer.protocol()) }
                    }
                    tr {
                        th { "Endpoint" }
                        td.mono { code { (layer.endpoint()) } }
                    }
                    tr {
                        th { "Active receivers" }
                        td.mono { (metrics.active_receivers) }
                    }
                    tr {
                        th { "Throughput" }
                        td.mono { (metrics.throughput_fps) " fps" }
                    }
                }
            }
        }
        section.placeholder {
            p.muted {
                "Receivers / throughput are mock values until Phase 4 wires real "
                "adapter counters. Per-adapter configuration (port, broker URL, "
                "DB credentials) lands under WBS-9.4b alongside the real adapter "
                "module that owns those settings."
            }
        }
        script type="module" { (PreEscaped(LAYER_TOGGLE_JS)) }
    }
}

/// API response body: the new (post-action) state of a layer.
#[derive(Debug, Serialize)]
pub struct LayerStateResponse {
    /// Layer code (`L0`..`L3`).
    pub layer: &'static str,
    /// Layer enabled state after the action completed.
    pub enabled: bool,
    /// Whether the request changed state (false = already in target state).
    pub changed: bool,
}

async fn api_enable(State(state): State<AppState>, Path(code): Path<String>) -> impl IntoResponse {
    api_toggle(state, &code, true)
}

async fn api_disable(State(state): State<AppState>, Path(code): Path<String>) -> impl IntoResponse {
    api_toggle(state, &code, false)
}

fn api_toggle(state: AppState, code: &str, target: bool) -> (StatusCode, Json<LayerStateResponse>) {
    let Some(layer) = Layer::from_str(code) else {
        return (
            StatusCode::NOT_FOUND,
            Json(LayerStateResponse {
                layer: "?",
                enabled: false,
                changed: false,
            }),
        );
    };
    let prev = state.set(layer, target);
    let changed = prev != target;
    if changed {
        audit::record(AuditEvent::NorthboundStateChange {
            layer: layer.code().to_string(),
            enabled: target,
        });
    }
    (
        StatusCode::OK,
        Json(LayerStateResponse {
            layer: layer.code(),
            enabled: target,
            changed,
        }),
    )
}

const LAYER_TOGGLE_JS: &str = r#"
document.querySelectorAll('button[data-layer][data-action]').forEach((btn) => {
  btn.addEventListener('click', async (e) => {
    e.stopPropagation();
    const layer = btn.getAttribute('data-layer');
    const action = btn.getAttribute('data-action');
    btn.disabled = true;
    try {
      const resp = await fetch('/api/north/' + layer + '/' + action, { method: 'POST' });
      if (resp.ok) {
        window.location.reload();
      } else {
        btn.disabled = false;
      }
    } catch (_) {
      btn.disabled = false;
    }
  });
});
"#;

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
    fn state_set_returns_previous_and_persists() {
        let s = NorthboundState::new();
        assert!(!s.is_enabled(Layer::L1));
        let prev = s.set(Layer::L1, true);
        assert!(!prev);
        assert!(s.is_enabled(Layer::L1));
        let prev2 = s.set(Layer::L1, true);
        assert!(prev2);
        assert!(s.is_enabled(Layer::L1));
    }

    #[test]
    fn api_toggle_reports_changed_correctly() {
        let s: AppState = Arc::new(NorthboundState::new());
        let (code, body) = api_toggle(s.clone(), "L2", true);
        assert_eq!(code, StatusCode::OK);
        assert!(body.0.enabled);
        assert!(body.0.changed);

        let (_, body2) = api_toggle(s.clone(), "L2", true);
        assert!(body2.0.enabled);
        assert!(!body2.0.changed, "second enable must report changed=false");

        let (_, body3) = api_toggle(s, "L2", false);
        assert!(!body3.0.enabled);
        assert!(body3.0.changed);
    }

    #[test]
    fn api_toggle_404_on_unknown_layer() {
        let s: AppState = Arc::new(NorthboundState::new());
        let (code, _) = api_toggle(s, "L9", true);
        assert_eq!(code, StatusCode::NOT_FOUND);
    }

    #[test]
    fn layer_detail_body_renders_all_facts() {
        let m = LayerMetrics::mock(Layer::L0);
        let s = layer_detail_body(Layer::L0, true, m).into_string();
        assert!(s.contains("L0"));
        assert!(s.contains("Shared Memory RingBuffer"));
        assert!(s.contains("/dev/shm/svdc_l0_ring"));
        assert!(s.contains("4800 fps"));
        assert!(s.contains("Enabled"));
    }

    #[test]
    fn row_click_js_excludes_inner_interactives() {
        assert!(ROW_CLICK_JS.contains("a, button, input, label"));
        assert!(ROW_CLICK_JS.contains("tabindex"));
    }
}
