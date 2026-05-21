//! `svdc-aligner` — time alignment stage (M2) for the SVDC.
//!
//! Takes [`IngressFrame`]s from the M1→M2 ring (`svdc-ingress`), bins
//! them onto the PTP-aligned tick grid, interpolates gaps, applies
//! per-channel calibration, and stages the result as [`TickRecord`]s in
//! the dual circular buffer the northbound layers drain.
//!
//! WBS partition (one submodule per item):
//!
//! | WBS    | module          | responsibility                                    |
//! | ------ | --------------- | ------------------------------------------------- |
//! | 2.5    | [`binner`]      | Map ingress timestamps to tick indices.           |
//! | 2.6    | [`interpolator`]| Fill missing samples when a publisher drops one.  |
//! | 2.7    | [`calibrator`]  | Apply per-channel gain/offset/unit_scale triples. |
//! | 2.8–9  | [`buffer`]      | Dual circular buffer + integrity / failover.      |
//!
//! Phase 0 scaffold: the four modules each ship an identity-pipeline
//! placeholder so the assembled [`Aligner`] runs end-to-end against
//! `svdc-ingress`'s `LoopbackSubscriber`. The Phase 2 owner replaces the
//! placeholders with the real binning math, drop-detection /
//! interpolation, calibration matrix, and the dual-CB failover logic.
//! See `docs/decisions/0009-aligner-design.md`.
//!
//! OWNER: claude-code (Phase 0 scaffold + ADR-0009). Phase 2 hot-path
//! work (real binner, interpolator, dual-CB) goes to Antigravity.
//! NFR-10: English-only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod binner;
pub mod buffer;
pub mod calibrator;
pub mod interpolator;

pub use binner::{Binner, TickIndex};
pub use buffer::TickBuffer;
pub use calibrator::{Calibration, Calibrator};
pub use interpolator::Interpolator;

use svdc_core::TickRecord;
use svdc_ingress::IngressFrame;

/// Top-level aligner. Owns the four pipeline stages; callers drive it
/// by calling [`Aligner::process_frame`] for each `IngressFrame`
/// they pop off the ring.
#[derive(Debug)]
pub struct Aligner {
    binner: Binner,
    interpolator: Interpolator,
    calibrator: Calibrator,
    next_tick_id: u64,
}

impl Aligner {
    /// Construct an aligner with a fixed bin period (in nanoseconds).
    /// Typical: `1_000_000_000 / 4800 = 208_333` ns for 80 SPC at 60 Hz.
    pub fn new(bin_period_ns: u64) -> Self {
        Self {
            binner: Binner::new(bin_period_ns),
            interpolator: Interpolator,
            calibrator: Calibrator,
            next_tick_id: 0,
        }
    }

    /// Push one ingress frame through the pipeline. Returns zero, one,
    /// or many tick records: zero when the bin is still open, one
    /// for the common case, multiple if the publisher's last frame
    /// crossed several bins.
    ///
    /// Phase 0 identity behaviour: emits exactly one `TickRecord` per
    /// input frame so the assembled scaffold can be exercised
    /// end-to-end. Phase 2 replaces this with the real binning logic.
    pub fn process_frame(&mut self, frame: IngressFrame) -> Vec<TickRecord> {
        let _bin = self.binner.bin_index(&frame);
        let _interpolated = self.interpolator.fill_gaps(&frame);
        let _calibrated = self.calibrator.apply(&frame);
        let tick = TickRecord {
            tick_id: self.next_tick_id,
            ts_utc_ns: frame.timestamp.unix_ns(),
        };
        self.next_tick_id += 1;
        vec![tick]
    }

    /// Reset internal counters. Test helper.
    pub fn reset(&mut self) {
        self.next_tick_id = 0;
    }

    /// Next tick id that would be emitted. Visible for inspection /
    /// integration tests; not part of the steady-state contract.
    pub fn next_tick_id(&self) -> u64 {
        self.next_tick_id
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    //! Shared test fixtures. Reaches for the publisher's `SampleData`
    //! via the dev-dependency so the runtime graph stays clean.
    use ssiec_sv_publisher::SampleData;
    use svdc_ingress::{DecodedSample, IngressFrame, IngressTimestamp};

    pub fn dummy_frame(ts_ns: u64) -> IngressFrame {
        IngressFrame {
            timestamp: IngressTimestamp::from_unix_ns(ts_ns),
            samples: vec![DecodedSample {
                sv_id: "T".into(),
                smp_cnt: 0,
                conf_rev: 0,
                smp_synch: 2,
                smp_rate: 4800,
                samples: SampleData::NOMINAL_3PH,
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::dummy_frame;

    #[test]
    fn process_frame_emits_one_tick_per_input_in_phase_0() {
        let mut a = Aligner::new(208_333);
        let out = a.process_frame(dummy_frame(1_000_000_000));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].tick_id, 0);
        assert_eq!(out[0].ts_utc_ns, 1_000_000_000);
    }

    #[test]
    fn tick_ids_are_monotonic() {
        let mut a = Aligner::new(208_333);
        let r0 = a.process_frame(dummy_frame(1_000_000_000));
        let r1 = a.process_frame(dummy_frame(1_000_208_333));
        let r2 = a.process_frame(dummy_frame(1_000_416_666));
        assert_eq!(r0[0].tick_id, 0);
        assert_eq!(r1[0].tick_id, 1);
        assert_eq!(r2[0].tick_id, 2);
        assert_eq!(a.next_tick_id(), 3);
    }

    #[test]
    fn reset_rolls_back_tick_id() {
        let mut a = Aligner::new(208_333);
        a.process_frame(dummy_frame(1_000_000_000));
        a.process_frame(dummy_frame(1_000_208_333));
        assert_eq!(a.next_tick_id(), 2);
        a.reset();
        assert_eq!(a.next_tick_id(), 0);
    }
}
