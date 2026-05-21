//! Typed Server-Sent Event payloads for the Operator Console.
//!
//! The wire format is JSON, with a `event_type` discriminator and a
//! `data` payload. Subscribers (the Dashboard tile updater, the MU
//! waveform consumer) parse `event_type` first and dispatch.
//!
//! OWNER: claude-code (WBS-9.2a — typed contract).
//! Scaffold and field set were drafted by Antigravity under WBS-9.2b
//! to allow the emitter to compile; this revision adds the doc
//! comments required by the crate-level `missing_docs` lint.
//! NFR-10: English-only.

use serde::Serialize;

/// Background telemetry emitter: produces `SsePayload` JSON strings on
/// a `broadcast` channel that the SSE route subscribes to.
pub mod emitter;

/// Dashboard telemetry snapshot. One per second from the emitter.
#[derive(Debug, Clone, Serialize)]
pub struct DashboardMetrics {
    /// Human-readable PTP synchronization state, e.g. `"Locked"`,
    /// `"Holdover"`, `"Free-running"`. Maps to UI tile colour.
    pub ptp_sync_status: String,
    /// Most recently observed PTP offset relative to the grandmaster,
    /// in nanoseconds. Sign carries direction (lead vs lag).
    pub ptp_offset_ns: i64,
    /// Circular-buffer fill, in percent (0.0..=100.0).
    pub buffer_saturation: f64,
    /// Number of Merging Units currently registered and streaming.
    pub active_mus: usize,
    /// Aggregate samples-per-second rate observed across all MUs.
    pub sps_rate: u32,
    /// Whether the L1 OPC UA northbound layer is enabled and serving.
    pub l1_opcua_active: bool,
    /// Whether the L2 MQTT northbound layer is enabled and publishing.
    pub l2_mqtt_active: bool,
    /// Whether the L3 TimescaleDB northbound sidecar is enabled and
    /// successfully writing rows.
    pub l3_timescaledb_active: bool,
    /// Number of records in the TickBuffer whose CRC failed the most
    /// recent integrity sweep. Zero = healthy. Source: PR #49.
    #[serde(default)]
    pub integrity_violations: usize,
    /// `true` when the daemon's `--ingress-udp` listener is feeding
    /// the buffer; `false` when the in-process synthetic demo is the
    /// only producer (or no producer is active). Per ADR-0015 §3.
    #[serde(default)]
    pub live_feed_active: bool,
}

/// One 8-channel sample tuple for the MU-detail live waveform.
/// Emitted at ≤10 Hz (downsampled from 4800 Hz on the daemon side).
#[derive(Debug, Clone, Serialize)]
pub struct WaveformSample {
    /// MU identifier the sample originated from.
    pub mu_id: String,
    /// Unix-millis sample timestamp (PTP-disciplined when available).
    pub timestamp_ms: u64,
    /// Voltage phase 1.
    pub v1: f32,
    /// Voltage phase 2.
    pub v2: f32,
    /// Voltage phase 3.
    pub v3: f32,
    /// Voltage neutral.
    pub v0: f32,
    /// Current phase 1.
    pub i1: f32,
    /// Current phase 2.
    pub i2: f32,
    /// Current phase 3.
    pub i3: f32,
    /// Current neutral.
    pub i0: f32,
}

/// Discriminated union the SSE channel serializes.
///
/// The JSON shape is `{ "event_type": "Metrics", "data": { … } }` or
/// `{ "event_type": "Waveform", "data": { … } }`. Browser-side code
/// switches on `event_type` to decide which tile to update.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type", content = "data")]
pub enum SsePayload {
    /// Dashboard tile snapshot (1 Hz).
    Metrics(DashboardMetrics),
    /// MU waveform sample tuple (≤10 Hz).
    Waveform(WaveformSample),
}
