//! `GET /metrics` — Prometheus text exposition format.
//!
//! Hand-rendered, no `prometheus` crate dep. The Phase 0 metric set
//! is minimal; new metrics append. Naming follows the Prometheus
//! style guide: lowercase, `svdc_` prefix, `_total` suffix on
//! counters, no suffix on gauges.

use std::sync::Arc;

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;

use crate::ManagementContext;

/// Returns 200 with `text/plain; version=0.0.4` per the Prometheus
/// exposition format spec.
pub async fn handler(State(ctx): State<Arc<ManagementContext>>) -> impl IntoResponse {
    let violations = ctx.tick_buffer.verify_all().len();
    let body = format!(
        "\
# HELP svdc_uptime_ms Daemon uptime since process start, milliseconds.\n\
# TYPE svdc_uptime_ms gauge\n\
svdc_uptime_ms {}\n\
# HELP svdc_tick_buffer_len Current number of TickRecords held in the buffer.\n\
# TYPE svdc_tick_buffer_len gauge\n\
svdc_tick_buffer_len {}\n\
# HELP svdc_tick_buffer_capacity Maximum number of TickRecords the buffer can hold.\n\
# TYPE svdc_tick_buffer_capacity gauge\n\
svdc_tick_buffer_capacity {}\n\
# HELP svdc_integrity_violations Records with CRC failure at the most recent sweep.\n\
# TYPE svdc_integrity_violations gauge\n\
svdc_integrity_violations {}\n",
        ctx.uptime_ms(),
        ctx.tick_buffer.len(),
        ctx.tick_buffer.capacity(),
        violations,
    );
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}
