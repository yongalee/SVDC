//! `GET /health` — liveness + data-plane integrity verdict.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::model::{DataPlaneHealth, HealthResponse};
use crate::ManagementContext;

/// Returns 200 with the [`HealthResponse`]. `status` is `"degraded"`
/// when [`svdc_aligner::TickBuffer::verify_all`] returns any
/// violations, `"ok"` otherwise.
pub async fn handler(State(ctx): State<Arc<ManagementContext>>) -> Json<HealthResponse> {
    let violations = ctx.tick_buffer.verify_all();
    let buffer_len = ctx.tick_buffer.len();
    let capacity = ctx.tick_buffer.capacity();
    let status = if violations.is_empty() {
        "ok".to_string()
    } else {
        "degraded".to_string()
    };
    Json(HealthResponse {
        status,
        uptime_ms: ctx.uptime_ms(),
        data_plane: DataPlaneHealth {
            tick_buffer_len: buffer_len,
            tick_buffer_capacity: capacity,
            integrity_violations: violations.len(),
        },
    })
}
