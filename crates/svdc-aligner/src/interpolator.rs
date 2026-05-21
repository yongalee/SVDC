//! WBS-2.6 — interpolator.
//!
//! When the publisher drops a sample (a smpCnt gap inside the bin's
//! grace window) the aligner must fill the gap so downstream consumers
//! see a regular tick stream. Phase 2 will implement linear
//! interpolation between the last good sample and the next good
//! sample on the same channel, marking the synthesised value with the
//! `Sample::origin = interpolated` flag defined in `svdc-core`.
//!
//! Phase 0 is identity: every input passes through untouched.

use svdc_ingress::IngressFrame;

/// Stateful interpolator. Phase 2 will track per-channel last-good
/// samples here so a gap can be filled without consulting the
/// circular buffer.
#[derive(Debug, Default)]
pub struct Interpolator;

impl Interpolator {
    /// Phase 0 identity: returns the input frame unchanged. Phase 2
    /// will return a (possibly larger) `Vec<IngressFrame>` when one
    /// input fills multiple bins.
    pub fn fill_gaps<'a>(&mut self, frame: &'a IngressFrame) -> &'a IngressFrame {
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::dummy_frame;

    #[test]
    fn phase_0_interpolator_is_identity() {
        let mut interp = Interpolator;
        let f = dummy_frame(42);
        let out = interp.fill_gaps(&f);
        assert_eq!(out.timestamp.unix_ns(), 42);
        assert_eq!(out.samples.len(), 1);
    }
}
