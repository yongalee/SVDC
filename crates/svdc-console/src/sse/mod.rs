//! SSE payload and event definitions for the SVDC Console.

use serde::Serialize;

/// The background emitter driving real-time updates.
pub mod emitter;

/// Metrics for the high-density system dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct DashboardMetrics {
    /// PTP Grandmaster synchronization status text.
    pub ptp_sync_status: String,
    /// Current measured offset from master in nanoseconds.
    pub ptp_offset_ns: i64,
    /// redunant circular buffer saturation level in percent.
    pub buffer_saturation: f64,
    /// Number of active merging units currently feeding the concentrator.
    pub active_mus: usize,
    /// Total aggregated throughput rate in samples per second.
    pub sps_rate: u32,
    /// L1 OPC UA server active state.
    pub l1_opcua_active: bool,
    /// L2 MQTT broker client active state.
    pub l2_mqtt_active: bool,
    /// L3 TimescaleDB sidecar archiver active state.
    pub l3_timescaledb_active: bool,
}

/// Instantaneous waveform sample for the inline-SVG oscilloscope.
#[derive(Debug, Clone, Serialize)]
pub struct WaveformSample {
    /// Identifier of the originating Merging Unit.
    pub mu_id: String,
    /// UNIX timestamp of the sample in milliseconds.
    pub timestamp_ms: u64,
    /// Voltage channel A.
    pub v1: f32,
    /// Voltage channel B.
    pub v2: f32,
    /// Voltage channel C.
    pub v3: f32,
    /// Voltage channel Neutral.
    pub v0: f32,
    /// Current channel A.
    pub i1: f32,
    /// Current channel B.
    pub i2: f32,
    /// Current channel C.
    pub i3: f32,
    /// Current channel Neutral.
    pub i0: f32,
}

/// Quasi-dynamic State Estimator (QSE) self-healing override audit log entry.
#[derive(Debug, Clone, Serialize)]
pub struct QseLog {
    /// Timestamp of the log entry.
    pub timestamp: String,
    /// WBS item code.
    pub wbs: String,
    /// Self-healing operation action summary.
    pub operation: String,
    /// Target channel or slot ID.
    pub target: String,
    /// Originating module or operator.
    pub operator: String,
    /// Outcome or result label.
    pub result: String,
    /// CSS color code class.
    pub result_color: String,
}

/// Per-Merging Unit diagnostic telemetry metrics.
#[derive(Debug, Clone, Serialize)]
pub struct MuTelemetry {
    /// Identifier of the Merging Unit.
    pub mu_id: String,
    /// Observed sample rate in samples per second.
    pub observed_sps: u32,
    /// Number of missing samples detected.
    pub missing_samples: u32,
    /// Number of interpolations performed.
    pub interpolation_count: u32,
    /// Number of QSE self-healing corrections applied.
    pub qse_corrections: u32,
    /// Histogram bucket distribution of arrival jitters.
    pub jitter_histogram: Vec<u32>,
    /// PTP synchronization status.
    pub ptp_sync: String,
    /// Configured calibration parameters: gain, offset, unit_scale.
    pub calibration: (f32, f32, f32),
}

/// Typed payloads broadcast over the Server-Sent Events stream.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type", content = "data")]
pub enum SsePayload {
    /// Dashboard metrics.
    Metrics(DashboardMetrics),
    /// Waveform sample.
    Waveform(WaveformSample),
    /// QSE log event.
    Qse(QseLog),
    /// Collection of all active Merging Unit diagnostics.
    MuMetrics(Vec<MuTelemetry>),
}
