//! SVDC-local operational state.
//!
//! Distinct from the SCD-derived ChannelRegistry (`scd::registry`).
//! The SCD is an immutable input authored by the SCT and produced by
//! the system engineer per IEC 61850-6 — the SVDC must never write
//! back to it. Calibration triples, subscription enable/disable, and
//! similar operational tuning live here instead, persisted in the
//! SVDC's own config file (`/etc/svdc/operational.toml` in production;
//! in-memory for v0.1).
//!
//! OWNER: claude-code (WBS-9.6a extension for calibration).
//! NFR-10: English-only.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

/// Per-channel calibration triple.
///
/// Applied to the raw SV sample as:
/// `corrected = (raw * gain + offset) * unit_scale`.
/// Default (1.0, 0.0, 1.0) is the identity transform.
///
/// `gain` and `offset` correct sensor non-idealities (gain error,
/// DC bias) and live as f32 in scaled-integer space. `unit_scale`
/// brings the result into engineering units for display (e.g. 0.01
/// for voltage to convert the 9-2 LE i32 representation to volts).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    /// Multiplicative gain correction.
    pub gain: f32,
    /// Additive offset, in raw scaled-integer units.
    pub offset: f32,
    /// Display-unit scale (e.g. 0.01 for voltage → V).
    pub unit_scale: f32,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            gain: 1.0,
            offset: 0.0,
            unit_scale: 1.0,
        }
    }
}

impl Calibration {
    /// Apply the calibration to a raw i32 SV sample.
    pub fn apply(&self, raw: i32) -> f32 {
        (raw as f32 * self.gain + self.offset) * self.unit_scale
    }

    /// Whether this calibration is the identity transform.
    pub fn is_identity(&self) -> bool {
        self.gain == 1.0 && self.offset == 0.0 && self.unit_scale == 1.0
    }
}

/// Key for the per-channel calibration map.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelKey {
    /// MU id this channel belongs to (matches `MergingUnit::id`).
    pub mu_id: String,
    /// 0-based index into the MU's `channels` list.
    pub channel_idx: usize,
}

/// SVDC-local operational state. Read-write across routes.
#[derive(Debug, Default)]
pub struct OperationalState {
    calibrations: RwLock<HashMap<ChannelKey, Calibration>>,
}

impl OperationalState {
    /// Construct an empty operational state.
    pub fn new() -> Self {
        Self {
            calibrations: RwLock::new(HashMap::new()),
        }
    }

    /// Look up the calibration for one channel. Returns the identity
    /// transform if no override has been applied.
    pub fn calibration(&self, mu_id: &str, channel_idx: usize) -> Calibration {
        let key = ChannelKey {
            mu_id: mu_id.to_string(),
            channel_idx,
        };
        self.calibrations
            .read()
            .ok()
            .and_then(|g| g.get(&key).copied())
            .unwrap_or_default()
    }

    /// Set or update the calibration for one channel. Returns the
    /// previous calibration (default if none was set).
    pub fn set_calibration(&self, key: ChannelKey, value: Calibration) -> Calibration {
        let mut g = self
            .calibrations
            .write()
            .expect("operational state lock poisoned");
        g.insert(key.clone(), value).unwrap_or_default()
    }

    /// Reset a channel back to identity (remove the override).
    pub fn reset_calibration(&self, key: &ChannelKey) -> Option<Calibration> {
        let mut g = self
            .calibrations
            .write()
            .expect("operational state lock poisoned");
        g.remove(key)
    }

    /// Snapshot all non-default calibrations. Useful for the audit log
    /// and for the eventual TOML serializer.
    pub fn overrides(&self) -> HashMap<ChannelKey, Calibration> {
        self.calibrations
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

/// Shared handle used by axum handlers.
pub type SharedOperational = Arc<OperationalState>;

/// Process-wide singleton.
pub fn global() -> SharedOperational {
    static INSTANCE: OnceLock<SharedOperational> = OnceLock::new();
    INSTANCE
        .get_or_init(|| Arc::new(OperationalState::new()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_calibration_is_identity_transform() {
        let c = Calibration::default();
        assert!(c.is_identity());
        assert_eq!(c.apply(1000), 1000.0);
        assert_eq!(c.apply(-2500), -2500.0);
    }

    #[test]
    fn apply_corrects_sensor_gain_and_offset() {
        let c = Calibration {
            gain: 1.05,
            offset: -50.0,
            unit_scale: 0.01,
        };
        // raw 5000 -> (5000*1.05 - 50) * 0.01 = 52.0
        let r = c.apply(5000);
        assert!((r - 52.0).abs() < 1e-3, "got {r}");
    }

    #[test]
    fn operational_state_returns_identity_when_unset() {
        let s = OperationalState::new();
        let c = s.calibration("MU-X", 0);
        assert!(c.is_identity());
    }

    #[test]
    fn set_and_reset_calibration_roundtrips() {
        let s = OperationalState::new();
        let key = ChannelKey {
            mu_id: "MU-X".into(),
            channel_idx: 2,
        };
        let cal = Calibration {
            gain: 1.1,
            offset: 5.0,
            unit_scale: 0.01,
        };
        let prev = s.set_calibration(key.clone(), cal);
        assert!(prev.is_identity());
        let got = s.calibration("MU-X", 2);
        assert_eq!(got, cal);

        let removed = s.reset_calibration(&key);
        assert_eq!(removed, Some(cal));
        assert!(s.calibration("MU-X", 2).is_identity());
    }

    #[test]
    fn overrides_snapshot_excludes_unset_channels() {
        let s = OperationalState::new();
        s.set_calibration(
            ChannelKey {
                mu_id: "MU-A".into(),
                channel_idx: 0,
            },
            Calibration {
                gain: 2.0,
                offset: 0.0,
                unit_scale: 1.0,
            },
        );
        let snap = s.overrides();
        assert_eq!(snap.len(), 1);
        assert!(snap.contains_key(&ChannelKey {
            mu_id: "MU-A".into(),
            channel_idx: 0,
        }));
    }
}
