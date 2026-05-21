/* SVDC Console SSE Event Definitions
   OWNER: claude-code
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers

   Note: This file is owned by Claude Code (WBS-9.2a). This is a skeleton
   stub created by Antigravity (WBS-9.2b) to define the typed event contract
   and allow the emitter to compile.
*/

use serde::Serialize;

pub mod emitter;

#[derive(Debug, Clone, Serialize)]
pub struct DashboardMetrics {
    pub ptp_sync_status: String,
    pub ptp_offset_ns: i64,
    pub buffer_saturation: f64,
    pub active_mus: usize,
    pub sps_rate: u32,
    pub l1_opcua_active: bool,
    pub l2_mqtt_active: bool,
    pub l3_timescaledb_active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaveformSample {
    pub mu_id: String,
    pub timestamp_ms: u64,
    pub v1: f32,
    pub v2: f32,
    pub v3: f32,
    pub v0: f32,
    pub i1: f32,
    pub i2: f32,
    pub i3: f32,
    pub i0: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type", content = "data")]
pub enum SsePayload {
    Metrics(DashboardMetrics),
    Waveform(WaveformSample),
}
