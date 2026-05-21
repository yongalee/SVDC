//! Core shared types for the SVDC.
//!
//! This crate holds types used across the ingest, alignment, and
//! storage crates. It has no I/O and no async runtime. See the SDD §7
//! for the authoritative data model. This module is the Rust
//! translation of SDD §7.1; the field shape, order, and units track
//! the SDD verbatim.
//!
//! Status: Phase 2 baseline. Field shape is locked here so the
//! aligner, dual circular buffer, and northbound layers can bind
//! against a stable record type.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Maximum number of channels in one [`TickRecord`].
///
/// SDD §7.1 specifies `samples[MAX_CH]` as a fixed-size array so the
/// record is laid out for cache-line alignment and predictable cost.
/// The current value covers eight 8-channel MUs (`8 × 8 = 64`). A
/// future deployment with more MUs per node can grow this; the layout
/// is `#[repr(C)]` so the C ABI for in-process subscribers
/// (SDD §8.2) stays binary-stable as long as the constant does not
/// shrink.
pub const MAX_CHANNELS: usize = 64;

/// Per-channel sample inside a [`TickRecord`]. Eight bytes, packed
/// to match the SDD §7.1 layout.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sample {
    /// Calibrated value, Q-format scaled to channel unit. For voltage
    /// channels the publisher's reference unit is 0.01 V/LSB; for
    /// current channels 0.001 A/LSB. Aligner reapplies the
    /// per-channel scale at calibration time.
    pub value_q: i32,
    /// IEC 61850 quality bits (low byte). Per-channel.
    pub quality: u8,
    /// Origin discriminator. See [`SampleOrigin`] for the enum
    /// translation; the field stays a `u8` so the struct matches the
    /// SDD layout 1:1.
    pub origin: u8,
    /// Reserved by SDD §7.1; zero in this version. Holds future
    /// flags (e.g. per-channel calibration version, per-channel
    /// override marker) without re-laying out the record.
    pub reserved: u16,
}

/// Origin discriminator for a [`Sample`]. Stored in [`Sample::origin`].
/// Values are stable u8 codes; new variants append.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleOrigin {
    /// Slot is unused (the record holds fewer than `MAX_CHANNELS`
    /// populated channels). Tools that walk samples must skip
    /// `Invalid` slots.
    Invalid = 0,
    /// Sample came from a real publisher frame.
    Live = 1,
    /// Sample was synthesised by the aligner's interpolator
    /// (WBS-2.6) because a publisher drop opened a gap.
    Interpolated = 2,
    /// Sample was overwritten by a QSE write-back (SDD §8.3, FR-6).
    QseEstimated = 3,
}

impl SampleOrigin {
    /// Raw u8 wire value.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Per-tick aligned record assembled by M2 (the aligner) and stored
/// in M5/M6 (the dual circular buffer). Layout matches SDD §7.1.
///
/// The struct is large (≈ 540 bytes for [`MAX_CHANNELS`] = 64) so it
/// is `Clone` but **not** `Copy`. Pass by reference on the hot path.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickRecord {
    /// Monotonic per-node tick counter. Never repeats and never
    /// wraps within the service life of one daemon.
    pub tick_id: u64,
    /// PTP-disciplined UTC timestamp, nanoseconds since Unix epoch.
    pub ts_utc_ns: u64,
    /// Number of channels populated in `samples[]`. The first
    /// `n_channels` entries carry live data; the rest are
    /// [`SampleOrigin::Invalid`].
    pub n_channels: u16,
    /// Bitfield from [`flags`]: `COMPLETE`, `INTERPOLATED`,
    /// `QSE_CORRECTED`, `DEGRADED`. Multiple bits may be set.
    pub flags: u16,
    /// CRC-32 over `samples[..n_channels]`. Phase 0 leaves this
    /// at zero; the integrity overlay (WBS-2.9) populates and
    /// verifies it.
    pub crc: u32,
    /// Per-channel samples indexed by `channel_id` in the registry.
    pub samples: [Sample; MAX_CHANNELS],
}

impl Default for TickRecord {
    fn default() -> Self {
        Self {
            tick_id: 0,
            ts_utc_ns: 0,
            n_channels: 0,
            flags: 0,
            crc: 0,
            samples: [Sample::default(); MAX_CHANNELS],
        }
    }
}

/// Bitfield values for [`TickRecord::flags`]. Held as `u16` constants
/// so the SDD layout stays C-compatible; treat them as bitwise OR.
pub mod flags {
    /// Every `samples[0..n_channels]` came from a live publisher
    /// frame (no interpolation, no QSE write-back). Mutually
    /// exclusive with [`INTERPOLATED`] / [`QSE_CORRECTED`] only by
    /// convention — operators may choose to clear `COMPLETE` when
    /// any non-live origin appears.
    pub const COMPLETE: u16 = 0x0001;
    /// At least one sample in this tick was synthesised by the
    /// interpolator (FR-4).
    pub const INTERPOLATED: u16 = 0x0002;
    /// At least one sample was overwritten by a QSE write-back
    /// (FR-6, SDD §8.3).
    pub const QSE_CORRECTED: u16 = 0x0004;
    /// Record is usable but operating outside spec — e.g. PTP lock
    /// lost, MU dropped, calibration stale.
    pub const DEGRADED: u16 = 0x0008;
}

impl TickRecord {
    /// Construct a tick with only metadata; `samples[]` defaults to
    /// all-`Invalid`. Useful for tests and for the aligner's
    /// "empty bin" emission.
    pub fn empty(tick_id: u64, ts_utc_ns: u64) -> Self {
        Self {
            tick_id,
            ts_utc_ns,
            n_channels: 0,
            flags: 0,
            crc: 0,
            samples: [Sample::default(); MAX_CHANNELS],
        }
    }

    /// Whether the named flag is set.
    pub fn has_flag(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    /// OR `flag` into [`Self::flags`].
    pub fn set_flag(&mut self, flag: u16) {
        self.flags |= flag;
    }

    /// Iterator over the populated channel slots (`0..n_channels`).
    pub fn live_samples(&self) -> &[Sample] {
        let n = self.n_channels as usize;
        let n = n.min(MAX_CHANNELS);
        &self.samples[..n]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_record_default_constructs() {
        let r = TickRecord::default();
        assert_eq!(r.tick_id, 0);
        assert_eq!(r.ts_utc_ns, 0);
        assert_eq!(r.n_channels, 0);
        assert_eq!(r.flags, 0);
        assert_eq!(r.crc, 0);
        assert_eq!(r.samples.len(), MAX_CHANNELS);
        // All slots start invalid.
        for s in &r.samples {
            assert_eq!(s.origin, SampleOrigin::Invalid.as_u8());
        }
    }

    #[test]
    fn flags_compose_with_bitwise_or() {
        let mut r = TickRecord::empty(1, 0);
        assert!(!r.has_flag(flags::COMPLETE));
        r.set_flag(flags::COMPLETE);
        r.set_flag(flags::DEGRADED);
        assert!(r.has_flag(flags::COMPLETE));
        assert!(r.has_flag(flags::DEGRADED));
        assert!(!r.has_flag(flags::INTERPOLATED));
        assert_eq!(r.flags, flags::COMPLETE | flags::DEGRADED);
    }

    #[test]
    fn live_samples_returns_populated_prefix_only() {
        let mut r = TickRecord::empty(0, 0);
        r.n_channels = 3;
        r.samples[0] = Sample {
            value_q: 100,
            quality: 0,
            origin: SampleOrigin::Live.as_u8(),
            reserved: 0,
        };
        r.samples[1] = Sample {
            value_q: 200,
            quality: 0,
            origin: SampleOrigin::Live.as_u8(),
            reserved: 0,
        };
        r.samples[2] = Sample {
            value_q: 300,
            quality: 0,
            origin: SampleOrigin::Live.as_u8(),
            reserved: 0,
        };
        let live = r.live_samples();
        assert_eq!(live.len(), 3);
        assert_eq!(live[0].value_q, 100);
        assert_eq!(live[2].value_q, 300);
    }

    #[test]
    fn live_samples_clamps_to_max_channels_on_overflow() {
        let mut r = TickRecord::empty(0, 0);
        r.n_channels = 9999; // garbage value
        assert_eq!(r.live_samples().len(), MAX_CHANNELS);
    }

    #[test]
    fn sample_origin_round_trips_u8() {
        assert_eq!(SampleOrigin::Invalid.as_u8(), 0);
        assert_eq!(SampleOrigin::Live.as_u8(), 1);
        assert_eq!(SampleOrigin::Interpolated.as_u8(), 2);
        assert_eq!(SampleOrigin::QseEstimated.as_u8(), 3);
    }

    #[test]
    fn sample_layout_is_eight_bytes() {
        // SDD §7.1 specifies an 8-byte Sample. The compiler can pad
        // to 8 regardless, but pinning it here catches accidental
        // field reorderings that would break the C ABI in §8.2.
        assert_eq!(core::mem::size_of::<Sample>(), 8);
    }
}
