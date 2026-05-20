//! Core shared types for the SVDC.
//!
//! This crate holds types used across the ingest, alignment, and storage
//! crates. It has no I/O and no async runtime. See the SDD §7 for the
//! authoritative data model.
//!
//! Status: Phase 0 skeleton. Real definitions land in Phase 1 and Phase 2.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Placeholder for the per-tick aligned record defined in SDD §7.1.
///
/// To be implemented in Phase 2 (WBS-2.5 / WBS-2.8).
#[derive(Debug, Clone, Copy, Default)]
pub struct TickRecord {
    /// Monotonic per-node tick counter.
    pub tick_id: u64,
    /// PTP-disciplined UTC timestamp, nanoseconds.
    pub ts_utc_ns: u64,
    // ... fields to be filled in per SDD §7.1
}

/// Placeholder for the per-channel sample defined in SDD §7.1.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sample {
    /// Calibrated value in Q-format scaled to channel unit.
    pub value_q: i32,
    /// IEC 61850 quality bits.
    pub quality: u8,
    /// Origin: live, interpolated, QSE-estimated.
    pub origin: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_record_default_constructs() {
        let r = TickRecord::default();
        assert_eq!(r.tick_id, 0);
    }
}
