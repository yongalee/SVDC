//! WBS-2.5 — time binner.
//!
//! Maps an `IngressFrame`'s wall-clock timestamp onto the PTP-aligned
//! tick grid. The grid is defined by a fixed bin period in
//! nanoseconds; bin `i` covers `[i * period, (i+1) * period)`.
//!
//! Phase 0 is a thin wrapper over integer division. Phase 2 will own:
//!  - per-MU sub-bin alignment (correcting for publisher skew),
//!  - smpCnt continuity checks (detecting drops mid-window),
//!  - bin "close" detection (a frame that arrives after its bin's grace
//!    window has elapsed counts as a late arrival, not a fill).
//!
//! The placeholder type lives here so the Phase 2 binner can replace
//! the body without changing the [`Aligner`] surface.

use svdc_ingress::IngressFrame;

/// Bin index on the tick grid. Monotonic per-aligner, never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TickIndex(pub u64);

/// Fixed-period binner. Cheap to clone; holds no per-frame state in
/// Phase 0.
#[derive(Debug, Clone)]
pub struct Binner {
    period_ns: u64,
}

impl Binner {
    /// Construct a binner with the given bin period.
    /// Period 0 is rejected (an empty bin makes no sense).
    pub fn new(period_ns: u64) -> Self {
        assert!(period_ns > 0, "Binner period_ns must be > 0");
        Self { period_ns }
    }

    /// Map a frame's ingress timestamp to its tick index.
    pub fn bin_index(&self, frame: &IngressFrame) -> TickIndex {
        TickIndex(frame.timestamp.unix_ns() / self.period_ns)
    }

    /// Period in nanoseconds.
    pub fn period_ns(&self) -> u64 {
        self.period_ns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::dummy_frame;

    #[test]
    fn bin_index_is_floor_division() {
        let b = Binner::new(208_333);
        // 208_333 * 4800 = 999_998_400 → ts ≥ that lands in bin 4800.
        assert_eq!(b.bin_index(&dummy_frame(999_998_400)).0, 4800);
        // Just below bin 4801's threshold (208_333 * 4801 = 1_000_206_733).
        assert_eq!(b.bin_index(&dummy_frame(1_000_206_732)).0, 4800);
        // Exactly at the threshold rolls into bin 4801.
        assert_eq!(b.bin_index(&dummy_frame(1_000_206_733)).0, 4801);
    }

    #[test]
    #[should_panic(expected = "Binner period_ns must be > 0")]
    fn zero_period_panics() {
        let _ = Binner::new(0);
    }

    #[test]
    fn period_ns_accessor_returns_construction_value() {
        let b = Binner::new(123_456);
        assert_eq!(b.period_ns(), 123_456);
    }
}
