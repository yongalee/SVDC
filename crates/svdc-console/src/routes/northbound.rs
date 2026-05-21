//! `GET /north` and `GET /north/:layer` — North-bound application layers.
//!
//! Shell + enable/disable API contract is authored here under
//! WBS-9.4a (Claude). The four per-layer detail cards (L0, L1, L2, L3)
//! are filled in under WBS-9.4b (Antigravity).
//!
//! Northbound state is held in an in-process `RwLock` for v0.1. Real
//! daemon integration (calling into the L0/L1/L2/L3 adapter modules)
//! lands in Phase 4 once those modules exist; until then, enable /
//! disable just flips the in-memory flag and emits a tracing event
//! that the audit log will later consume.

use std::sync::{Arc, RwLock};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use maud::{html, Markup, PreEscaped};
use serde::{Deserialize, Serialize};

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
            Layer::L0 => "In-process (C ABI)",
            Layer::L1 => "OPC UA Server",
            Layer::L2 => "MQTT Publisher",
            Layer::L3 => "TimescaleDB Historian",
        }
    }

    /// One-line purpose, shown in summary tables.
    pub fn purpose(self) -> &'static str {
        match self {
            Layer::L0 => "Sub-ms feed to EBP relays, QSE, and phasor computation.",
            Layer::L1 => "IEC 61850 ↔ OPC UA per OPC 10040 for SCADA / HMI.",
            Layer::L2 => "Cloud / analytics fan-out at configurable cadence.",
            Layer::L3 => "Historian + replay for audit and post-event analysis.",
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
    let rows: Vec<(Layer, bool)> = Layer::all()
        .iter()
        .copied()
        .map(|l| (l, state.is_enabled(l)))
        .collect();

    layout(
        Section::Northbound,
        "North-bound application layers",
        html! {
            section.northbound-index {
                p.muted {
                    "Four northbound layers fan out from the SVDC core. "
                    "Each can be enabled or disabled independently. "
                    "Layer detail cards (status, client list, throughput) "
                    "land under WBS-9.4b."
                }
                table.layer-table {
                    thead {
                        tr {
                            th.col-code { "Layer" }
                            th.col-name { "Name" }
                            th.col-purpose { "Purpose" }
                            th.col-state { "State" }
                            th.col-actions { "Actions" }
                        }
                    }
                    tbody {
                        @for (layer, enabled) in &rows {
                            tr {
                                td.mono { (layer.code()) }
                                td {
                                    a href={ "/north/" (layer.code()) } { (layer.name()) }
                                }
                                td.muted { (layer.purpose()) }
                                td {
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
            script type="module" { (PreEscaped(LAYER_TOGGLE_JS)) }
        },
    )
}

async fn north_layer(State(state): State<AppState>, Path(layer_code): Path<String>) -> Markup {
    match Layer::from_str(&layer_code) {
        Some(layer) => {
            let enabled = state.is_enabled(layer);
            layout(
                Section::Northbound,
                &format!("{} — {}", layer.code(), layer.name()),
                html! {
                    section.layer-detail {
                        a.btn-secondary href="/north" { "← All layers" }
                        h2 { (layer.code()) " · " (layer.name()) }
                        p.muted { (layer.purpose()) }
                        div.layer-state {
                            @if enabled {
                                span.state-badge.state-on { "Enabled" }
                            } @else {
                                span.state-badge.state-off { "Disabled" }
                            }
                        }
                        section.placeholder {
                            p.muted {
                                "Per-layer detail card (client list, throughput, "
                                "endpoint configuration) lands under WBS-9.4b."
                            }
                        }
                    }
                },
            )
        }
        None => layout(
            Section::Northbound,
            "Unknown layer",
            html! {
                section.placeholder {
                    h2 { "Unknown layer" }
                    p.muted { "Expected one of L0, L1, L2, L3." }
                }
            },
        ),
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
        tracing::info!(
            audit.layer = %layer.code(),
            audit.action = if target { "enable" } else { "disable" },
            "northbound state changed"
        );
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
document.querySelectorAll('button[data-layer]').forEach((btn) => {
  btn.addEventListener('click', async () => {
    const layer = btn.getAttribute('data-layer');
    const action = btn.getAttribute('data-action');
    btn.disabled = true;
    try {
      const resp = await fetch('/api/north/' + layer + '/' + action, { method: 'POST' });
      if (resp.ok) {
        /* Trivially refresh by reloading the index page; richer
           HTMX-driven row-swap lands under WBS-9.4b. */
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
}
