//! `GET /api/config/calibration` — read all non-default calibrations.
//! `GET /api/config/calibration/:mu_id` — per-MU map.
//! `POST /api/config/calibration/:mu_id/:idx` — set one channel.
//! `DELETE /api/config/calibration/:mu_id/:idx` — reset to identity.
//!
//! Calibration triples live in the SVDC-local `OperationalState`,
//! not in the SCD. Per IEC 61850-6 the SCD is a system-engineering
//! artefact and never edited by an operating node.
//!
//! OWNER: claude-code (WBS-9.6a extension).

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

use crate::operational::{self, Calibration, ChannelKey, SharedOperational};
use crate::scd::registry::{self as registry_mod, SharedRegistry};

/// Build the calibration sub-router with the global operational state.
pub fn router() -> Router {
    Router::new()
        .route("/api/config/calibration", get(api_list_all))
        .route("/api/config/calibration/:mu_id", get(api_list_for_mu))
        .route(
            "/api/config/calibration/:mu_id/:idx",
            post(api_set).delete(api_reset),
        )
        .with_state(AppState {
            operational: operational::global(),
            registry: registry_mod::global(),
        })
}

#[derive(Clone)]
struct AppState {
    operational: SharedOperational,
    registry: SharedRegistry,
}

/// Wire-format calibration entry.
#[derive(Debug, Serialize)]
pub struct CalibrationEntry {
    /// MU id this calibration applies to.
    pub mu_id: String,
    /// Channel index inside the MU's channel list.
    pub channel_idx: usize,
    /// The calibration values.
    pub calibration: Calibration,
}

async fn api_list_all(State(state): State<AppState>) -> Json<Vec<CalibrationEntry>> {
    let mut entries: Vec<CalibrationEntry> = state
        .operational
        .overrides()
        .into_iter()
        .map(|(k, v)| CalibrationEntry {
            mu_id: k.mu_id,
            channel_idx: k.channel_idx,
            calibration: v,
        })
        .collect();
    entries.sort_by(|a, b| {
        a.mu_id
            .cmp(&b.mu_id)
            .then_with(|| a.channel_idx.cmp(&b.channel_idx))
    });
    Json(entries)
}

async fn api_list_for_mu(
    State(state): State<AppState>,
    Path(mu_id): Path<String>,
) -> Json<HashMap<usize, Calibration>> {
    let mut out: HashMap<usize, Calibration> = HashMap::new();
    for (k, v) in state.operational.overrides() {
        if k.mu_id == mu_id {
            out.insert(k.channel_idx, v);
        }
    }
    Json(out)
}

/// Outcome of a calibration write.
#[derive(Debug, Serialize)]
pub struct CalibrationWriteResponse {
    /// Whether the write succeeded.
    pub ok: bool,
    /// Human-readable status.
    pub message: String,
    /// The state now in effect (or the identity if reset).
    pub calibration: Calibration,
}

async fn api_set(
    State(state): State<AppState>,
    Path((mu_id, idx)): Path<(String, usize)>,
    Json(value): Json<Calibration>,
) -> impl IntoResponse {
    if !mu_and_channel_exist(&state.registry, &mu_id, idx) {
        return (
            StatusCode::NOT_FOUND,
            Json(CalibrationWriteResponse {
                ok: false,
                message: format!("no MU '{mu_id}' channel {idx} in registry"),
                calibration: Calibration::default(),
            }),
        );
    }
    if !value.gain.is_finite() || !value.offset.is_finite() || !value.unit_scale.is_finite() {
        return (
            StatusCode::BAD_REQUEST,
            Json(CalibrationWriteResponse {
                ok: false,
                message: "gain/offset/unit_scale must be finite (no NaN, no inf)".into(),
                calibration: Calibration::default(),
            }),
        );
    }
    let key = ChannelKey {
        mu_id: mu_id.clone(),
        channel_idx: idx,
    };
    let prev = state.operational.set_calibration(key, value);
    tracing::info!(
        audit.event = "calibration_set",
        audit.mu_id = %mu_id,
        audit.channel_idx = idx,
        audit.gain = value.gain as f64,
        audit.offset = value.offset as f64,
        audit.unit_scale = value.unit_scale as f64,
        audit.previous_gain = prev.gain as f64,
        "calibration updated"
    );
    (
        StatusCode::OK,
        Json(CalibrationWriteResponse {
            ok: true,
            message: format!(
                "calibration set for MU '{mu_id}' channel {idx} (previous gain {})",
                prev.gain
            ),
            calibration: value,
        }),
    )
}

async fn api_reset(
    State(state): State<AppState>,
    Path((mu_id, idx)): Path<(String, usize)>,
) -> impl IntoResponse {
    let key = ChannelKey {
        mu_id: mu_id.clone(),
        channel_idx: idx,
    };
    let removed = state.operational.reset_calibration(&key);
    let had_override = removed.is_some();
    tracing::info!(
        audit.event = "calibration_reset",
        audit.mu_id = %mu_id,
        audit.channel_idx = idx,
        audit.had_override = had_override,
        "calibration reset to identity"
    );
    (
        StatusCode::OK,
        Json(CalibrationWriteResponse {
            ok: true,
            message: if had_override {
                format!("calibration for MU '{mu_id}' channel {idx} reset to identity")
            } else {
                format!("MU '{mu_id}' channel {idx} was already at identity")
            },
            calibration: Calibration::default(),
        }),
    )
}

fn mu_and_channel_exist(registry: &SharedRegistry, mu_id: &str, idx: usize) -> bool {
    registry
        .snapshot()
        .iter()
        .find(|m| m.id == mu_id)
        .map(|m| idx < m.channels.len())
        .unwrap_or(false)
}
