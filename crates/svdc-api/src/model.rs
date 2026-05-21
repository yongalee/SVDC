//! JSON DTOs for the management API.
//!
//! These shapes are the wire contract. Renaming, reordering, or
//! removing a field is a breaking change that external consumers
//! (Prometheus relabel rules, QSE scrapers, factory tests) will
//! notice. Add new optional fields freely; never remove old ones.

use serde::{Deserialize, Serialize};

/// Body of `GET /health`. Always 200 unless the daemon has stopped
/// accepting requests entirely; integrity / liveness issues surface
/// inside the JSON.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    /// `"ok"` when liveness checks pass; `"degraded"` when at least
    /// one data-plane invariant is failing.
    pub status: String,
    /// Daemon uptime in milliseconds since process start.
    pub uptime_ms: u128,
    /// Data-plane snapshot.
    pub data_plane: DataPlaneHealth,
}

/// Data-plane subsection of [`HealthResponse`].
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataPlaneHealth {
    /// Number of records currently held in the tick buffer.
    pub tick_buffer_len: usize,
    /// Tick buffer capacity (immutable after daemon start).
    pub tick_buffer_capacity: usize,
    /// Number of records whose CRC failed verification at the last
    /// integrity sweep (zero when healthy).
    pub integrity_violations: usize,
}

/// Body of `GET /channels`. Phase 0 returns an empty list; Phase 2
/// populates from the SCD-derived channel registry.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ChannelsResponse {
    /// Channel registry snapshot.
    pub channels: Vec<ChannelEntry>,
}

/// One row of the channel registry per SDD §7.2.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ChannelEntry {
    /// Dense channel index used into `TickRecord::samples`.
    pub channel_id: u16,
    /// Owning MU identifier.
    pub mu_id: String,
    /// One of `"A"`, `"B"`, `"C"`, `"N"`, `"G"` (ground).
    pub phase: String,
    /// One of `"voltage"`, `"current"`.
    pub quantity: String,
    /// Current calibration triple (engineering-unit conversion).
    pub calibration: CalibrationDto,
}

/// Body of `POST /calibration/{channel_id}` and the `calibration`
/// field inside `ChannelEntry`. Identical to the
/// `svdc_aligner::Calibration` struct on the data-plane side; kept
/// here as its own DTO so the wire format can diverge from the
/// in-memory struct without forcing a data-plane recompile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationDto {
    /// Multiplicative gain applied to the raw sample value.
    pub gain: f32,
    /// Additive offset (raw units) applied after gain.
    pub offset: f32,
    /// Scale factor from raw integer to engineering units.
    pub unit_scale: f32,
}

impl Default for CalibrationDto {
    fn default() -> Self {
        Self {
            gain: 1.0,
            offset: 0.0,
            unit_scale: 1.0,
        }
    }
}

/// Response body of `POST /calibration/{channel_id}` on success.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct CalibrationApplied {
    /// Channel ID the calibration was applied to.
    pub channel_id: u16,
    /// Triple that is now active for this channel.
    pub calibration: CalibrationDto,
}

/// Common JSON error envelope. 4xx and 5xx responses carry this
/// shape so consumers can rely on `err.error` being present.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ApiError {
    /// Short machine-readable error code, e.g. `"bad_calibration"`.
    pub error: String,
    /// Operator-readable explanation.
    pub message: String,
}
