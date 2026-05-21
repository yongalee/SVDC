//! 3-phase voltage + current waveform synthesiser for the SV publisher.
//!
//! Phase 1 work item: replace the Phase 0 hardcoded `NOMINAL_3PH`
//! constant with a configurable generator. One `WaveformConfig`
//! produces a stream of `SampleData` by `sample(smp_cnt)`; the
//! caller (PCAP writer / UDP loop) drives the sample counter.
//!
//! Pure math — no I/O, no allocation per sample. The `harmonics`
//! vectors are read-only during sample generation; allocation happens
//! once at config time.
//!
//! OWNER: claude-code (WBS-6.x extension).
//! NFR-10: English-only.

use core::f32::consts::PI;

use crate::{ChannelSample, SampleData, NUM_CHANNELS};

/// Configuration of the waveform generator.
///
/// All amplitude values are in 9-2 LE scaled-integer units:
///   - voltage: 0.01 V per LSB (i.e. 23000 = 230.0 V peak)
///   - current: 0.001 A per LSB (i.e. 5000 = 5.0 A peak)
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformConfig {
    /// Samples per second (e.g. 4800 = 80 SPC × 60 Hz).
    pub sample_rate: u32,
    /// Fundamental frequency in Hz (60.0 reference, 50.0 for EU sites).
    pub frequency: f32,
    /// Voltage amplitude (peak) in scaled units.
    pub voltage_amp: i32,
    /// Current amplitude (peak) in scaled units.
    pub current_amp: i32,
    /// Power-factor lag of current behind voltage, in radians. Zero
    /// for a resistive load; positive for inductive (most loads);
    /// negative for capacitive.
    pub current_lag_rad: f32,
    /// Additional harmonics on voltage. Each entry is
    /// `(order, relative_amplitude)`. Order 1 is the fundamental and
    /// is implicit; do not list it here. Relative amplitude is
    /// a fraction of `voltage_amp` (e.g. 0.05 = 5% THD-V at that
    /// harmonic).
    pub voltage_harmonics: Vec<(u32, f32)>,
    /// Same shape as `voltage_harmonics`, for currents.
    pub current_harmonics: Vec<(u32, f32)>,
}

impl Default for WaveformConfig {
    /// Reference deployment defaults: 4800 Hz, 60 Hz, nominal V/I,
    /// unity power factor, no harmonics.
    fn default() -> Self {
        Self {
            sample_rate: 4800,
            frequency: 60.0,
            voltage_amp: 23000, // 230 V peak
            current_amp: 5000,  // 5 A peak
            current_lag_rad: 0.0,
            voltage_harmonics: Vec::new(),
            current_harmonics: Vec::new(),
        }
    }
}

impl WaveformConfig {
    /// Generate one 8-channel sample at logical sample index
    /// `smp_cnt`. The function is pure: identical input yields
    /// identical output, no allocation.
    pub fn sample(&self, smp_cnt: u32) -> SampleData {
        let t = (smp_cnt as f64) / (self.sample_rate as f64);
        let omega = 2.0_f64 * (PI as f64) * (self.frequency as f64);
        // Phase offsets for A, B, C, neutral.
        const PHASE_A: f64 = 0.0;
        const TWO_THIRDS_PI: f64 = 2.0 * PI as f64 / 3.0;
        let phase_b = -TWO_THIRDS_PI;
        let phase_c = TWO_THIRDS_PI;

        let v_sample = |phase_offset: f64| -> i32 {
            let base = (self.voltage_amp as f64) * (omega * t + phase_offset).sin();
            let mut h: f64 = 0.0;
            for (order, rel) in &self.voltage_harmonics {
                let n = *order as f64;
                h += (self.voltage_amp as f64)
                    * (*rel as f64)
                    * (omega * n * t + n * phase_offset).sin();
            }
            (base + h).round() as i32
        };
        let i_sample = |phase_offset: f64| -> i32 {
            let lag = self.current_lag_rad as f64;
            let base = (self.current_amp as f64) * (omega * t + phase_offset - lag).sin();
            let mut h: f64 = 0.0;
            for (order, rel) in &self.current_harmonics {
                let n = *order as f64;
                h += (self.current_amp as f64)
                    * (*rel as f64)
                    * (omega * n * t + n * phase_offset - n * lag).sin();
            }
            (base + h).round() as i32
        };

        let va = v_sample(PHASE_A);
        let vb = v_sample(phase_b);
        let vc = v_sample(phase_c);
        let vn = -(va + vb + vc); // ideal Y-grounded sum-to-zero
        let ia = i_sample(PHASE_A);
        let ib = i_sample(phase_b);
        let ic = i_sample(phase_c);
        let in_ = -(ia + ib + ic);

        let mut channels = [ChannelSample::good(0); NUM_CHANNELS];
        // Channel ordering per crate::CHANNEL_LABELS: [Ia Ib Ic In Va Vb Vc Vn]
        channels[0] = ChannelSample::good(ia);
        channels[1] = ChannelSample::good(ib);
        channels[2] = ChannelSample::good(ic);
        channels[3] = ChannelSample::good(in_);
        channels[4] = ChannelSample::good(va);
        channels[5] = ChannelSample::good(vb);
        channels[6] = ChannelSample::good(vc);
        channels[7] = ChannelSample::good(vn);
        SampleData { channels }
    }

    /// Number of samples per power-frequency cycle. Convenience for
    /// callers that want to size buffers in cycle units.
    pub fn samples_per_cycle(&self) -> u32 {
        ((self.sample_rate as f32) / self.frequency).round() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: i32, b: i32, tol: i32) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn default_is_4800hz_60hz_nominal() {
        let c = WaveformConfig::default();
        assert_eq!(c.sample_rate, 4800);
        assert!((c.frequency - 60.0).abs() < f32::EPSILON);
        assert_eq!(c.voltage_amp, 23000);
        assert_eq!(c.current_amp, 5000);
        assert_eq!(c.samples_per_cycle(), 80);
    }

    #[test]
    fn fundamental_at_t0_matches_phase_offsets() {
        // At sample 0 -> t=0 -> Va = Vamp*sin(0) = 0, but with the
        // shifted phases Vb = sin(-2π/3) ≈ -0.866, Vc = sin(2π/3) ≈ 0.866.
        let c = WaveformConfig::default();
        let s = c.sample(0);
        let va = s.channels[4].value;
        let vb = s.channels[5].value;
        let vc = s.channels[6].value;
        assert!(approx(va, 0, 5), "Va near zero at t=0, got {va}");
        let expected_vb = (-(23000.0_f64) * (3.0_f64.sqrt() / 2.0)).round() as i32;
        let expected_vc = ((23000.0_f64) * (3.0_f64.sqrt() / 2.0)).round() as i32;
        assert!(
            approx(vb, expected_vb, 5),
            "Vb near {expected_vb}, got {vb}"
        );
        assert!(
            approx(vc, expected_vc, 5),
            "Vc near {expected_vc}, got {vc}"
        );
    }

    #[test]
    fn neutral_sums_to_negative_total_in_balanced_three_phase() {
        let c = WaveformConfig::default();
        for n in [0u32, 10, 20, 79] {
            let s = c.sample(n);
            let va = s.channels[4].value;
            let vb = s.channels[5].value;
            let vc = s.channels[6].value;
            let vn = s.channels[7].value;
            // sum of all four should be zero (within rounding).
            let sum = va + vb + vc + vn;
            assert!(
                sum.abs() <= 4,
                "balanced 3ph Σ should ≈ 0 at smp {n}, got {sum}"
            );
        }
    }

    #[test]
    fn quarter_cycle_voltage_peaks_at_amplitude() {
        // 4800 Hz / 60 Hz = 80 samples per cycle.
        // Quarter cycle = 20 samples. At smp=20 Va should = +amp.
        let c = WaveformConfig::default();
        let s = c.sample(20);
        assert!(approx(s.channels[4].value, 23000, 50));
    }

    #[test]
    fn current_lag_shifts_current_only() {
        // Pi/2 lag: current at smp=0 is at its negative peak of sin(-π/2).
        let c = WaveformConfig {
            current_lag_rad: std::f32::consts::FRAC_PI_2,
            ..WaveformConfig::default()
        };
        let s = c.sample(0);
        let ia = s.channels[0].value;
        let va = s.channels[4].value;
        assert!(approx(ia, -5000, 5), "Ia at -peak with pi/2 lag, got {ia}");
        assert!(approx(va, 0, 5), "Va unaffected, got {va}");
    }

    #[test]
    fn harmonics_change_waveform_shape() {
        let baseline = WaveformConfig::default().sample(10);
        let distorted = WaveformConfig {
            voltage_harmonics: vec![(3, 0.10), (5, 0.05)],
            ..WaveformConfig::default()
        }
        .sample(10);
        // Adding harmonics should make the sample meaningfully different.
        assert_ne!(baseline.channels[4].value, distorted.channels[4].value);
    }

    #[test]
    fn sample_is_pure_and_deterministic() {
        let c = WaveformConfig {
            current_lag_rad: 0.2,
            voltage_harmonics: vec![(3, 0.05)],
            ..WaveformConfig::default()
        };
        let a = c.sample(42);
        let b = c.sample(42);
        assert_eq!(a, b, "same input -> same output");
    }
}
