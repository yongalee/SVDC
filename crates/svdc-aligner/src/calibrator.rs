//! WBS-2.7 — per-channel calibration application.
//!
//! Applies `(gain, offset, unit_scale)` triples to each channel sample,
//! producing the calibrated value in engineering units that downstream
//! consumers (northbound layers, historian) expect.
//!
//! Phase 0 ships an identity calibrator and the [`Calibration`] data
//! struct that the Phase 2 owner will plug into the aligner's map of
//! `(mu_id, channel_idx) → Calibration`. The svdc-console
//! `operational::Calibration` already has the same field shape; ADR-0007
//! keeps the SCD-derived view and the operator-tunable view separate.

use svdc_ingress::IngressFrame;

/// Per-channel calibration triple. Matches `svdc_console::operational::Calibration`;
/// duplicated here so the aligner does not depend on the console crate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibration {
    /// Multiplicative gain applied to the raw sample value.
    pub gain: f32,
    /// Additive offset (raw units) applied after gain.
    pub offset: f32,
    /// Scale factor that converts raw integer to engineering units
    /// (e.g. `0.01` for "0.01 V per LSB" on a voltage channel).
    pub unit_scale: f32,
}

impl Default for Calibration {
    /// Identity calibration: `gain = 1.0`, `offset = 0.0`, `unit_scale = 1.0`.
    fn default() -> Self {
        Self {
            gain: 1.0,
            offset: 0.0,
            unit_scale: 1.0,
        }
    }
}

impl Calibration {
    /// Apply `(gain * raw + offset) * unit_scale` to one raw integer
    /// sample. Returns the engineering-unit value.
    pub fn apply(&self, raw: i32) -> f32 {
        (self.gain * raw as f32 + self.offset) * self.unit_scale
    }
}

/// Phase 0 calibrator. Phase 2 will hold the
/// `(mu_id, channel_idx) → Calibration` map and apply the triple to
/// each channel of the incoming frame, emitting a `Vec<Sample>` of
/// calibrated values.
#[derive(Debug, Default)]
pub struct Calibrator;

impl Calibrator {
    /// Phase 0 identity: returns the input untouched. Phase 2 will
    /// produce a `Vec<svdc_core::Sample>` with the calibrated values
    /// plus the per-channel quality bits passed through from the
    /// ingress payload.
    pub fn apply<'a>(&self, frame: &'a IngressFrame) -> &'a IngressFrame {
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::dummy_frame;

    #[test]
    fn identity_calibration_returns_raw_as_float() {
        let c = Calibration::default();
        assert!((c.apply(23000) - 23000.0).abs() < f32::EPSILON);
        assert!((c.apply(-5000) - -5000.0).abs() < f32::EPSILON);
    }

    #[test]
    fn calibration_applies_gain_offset_and_unit_scale_in_order() {
        // raw = 5000, gain = 1.05, offset = -50, unit_scale = 0.001
        // expected = (1.05*5000 - 50) * 0.001 = (5250 - 50) * 0.001 = 5.200
        let c = Calibration {
            gain: 1.05,
            offset: -50.0,
            unit_scale: 0.001,
        };
        let v = c.apply(5000);
        assert!((v - 5.2).abs() < 1e-4, "got {v}");
    }

    #[test]
    fn phase_0_calibrator_is_identity_on_frame() {
        let cal = Calibrator;
        let f = dummy_frame(99);
        let out = cal.apply(&f);
        assert_eq!(out.timestamp.unix_ns(), 99);
    }
}
