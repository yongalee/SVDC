//! `TickRecord.ts_utc_ns` ↔ OPC UA `DateTime` per ADR-0017 §3.
//!
//! OPC UA encodes `DateTime` as a signed 64-bit count of 100-ns
//! intervals since 1601-01-01 00:00:00 UTC. Our `ts_utc_ns` is an
//! unsigned 64-bit count of nanoseconds since 1970-01-01 00:00:00
//! UTC (Unix epoch). The conversion is therefore:
//!
//! ```text
//! opcua_ticks = (unix_ns + UNIX_TO_OPCUA_EPOCH_NS) / 100
//! ```
//!
//! No timezone math is needed (both epochs are UTC). The constant
//! offset is 369 years × the seconds-per-year average, but we
//! hard-code the exact value (11_644_473_600 s × 1e9) so the
//! conversion is bit-exact and the function stays pure.

/// Offset from the Unix epoch (1970-01-01 UTC) to the OPC UA epoch
/// (1601-01-01 UTC) in nanoseconds. Equal to 11_644_473_600 s
/// (369 years, with 89 leap days in that span) × 1_000_000_000 ns
/// per second.
pub const UNIX_TO_OPCUA_EPOCH_NS: u64 = 11_644_473_600_000_000_000;

/// Nanoseconds per OPC UA `DateTime` tick.
pub const NS_PER_OPCUA_TICK: u64 = 100;

/// Convert a `TickRecord.ts_utc_ns` (nanoseconds since Unix epoch)
/// to an OPC UA `DateTime` tick count (100-ns intervals since
/// 1601-01-01 UTC). The result is signed because the OPC UA wire
/// type is `Int64`, but for any plausible `ts_utc_ns` value (which
/// is post-Unix-epoch by construction) the result is positive.
///
/// Saturates at `u64::MAX` if `unix_ns + UNIX_TO_OPCUA_EPOCH_NS`
/// would overflow `u64` (year ~2554; the SVDC will not be running
/// then). The truncation to `i64` is safe because the OPC UA
/// `DateTime` range comfortably covers the next millennium.
pub fn utc_ns_to_opcua_ticks(unix_ns: u64) -> i64 {
    let total_ns = unix_ns.saturating_add(UNIX_TO_OPCUA_EPOCH_NS);
    let ticks = total_ns / NS_PER_OPCUA_TICK;
    ticks as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference value: 1970-01-01 00:00:00 UTC in OPC UA ticks.
    /// 11_644_473_600 s × 10_000_000 ticks/s = 116_444_736_000_000_000.
    const UNIX_EPOCH_AS_OPCUA_TICKS: i64 = 116_444_736_000_000_000;

    #[test]
    fn unix_epoch_zero_maps_to_opcua_epoch_offset() {
        assert_eq!(utc_ns_to_opcua_ticks(0), UNIX_EPOCH_AS_OPCUA_TICKS);
    }

    #[test]
    fn one_hundred_ns_advances_by_one_tick() {
        assert_eq!(utc_ns_to_opcua_ticks(100), UNIX_EPOCH_AS_OPCUA_TICKS + 1);
    }

    #[test]
    fn one_second_after_unix_epoch_advances_by_ten_million_ticks() {
        assert_eq!(
            utc_ns_to_opcua_ticks(1_000_000_000),
            UNIX_EPOCH_AS_OPCUA_TICKS + 10_000_000
        );
    }

    #[test]
    fn typical_2026_timestamp_round_trips() {
        // 2026-05-22 00:00:00 UTC = 1_779_321_600 s after Unix epoch.
        let unix_ns: u64 = 1_779_321_600_000_000_000;
        let ticks = utc_ns_to_opcua_ticks(unix_ns);
        // Reverse: ticks * 100 ns/tick - epoch offset ns = original.
        let recovered_ns = (ticks as u64) * NS_PER_OPCUA_TICK - UNIX_TO_OPCUA_EPOCH_NS;
        assert_eq!(recovered_ns, unix_ns);
    }

    #[test]
    fn saturates_on_overflow_rather_than_wrapping() {
        // The largest representable Unix nanoseconds without
        // overflowing the u64 epoch sum.
        let near_max = u64::MAX - UNIX_TO_OPCUA_EPOCH_NS - 100;
        // Anything beyond this saturates; verify the saturating
        // path does not silently produce a small number.
        let way_too_large = u64::MAX;
        let saturated = utc_ns_to_opcua_ticks(way_too_large);
        let bounded = utc_ns_to_opcua_ticks(near_max);
        assert!(
            saturated >= bounded,
            "saturated value must not wrap below in-range value"
        );
    }

    #[test]
    fn epoch_constant_matches_published_value() {
        // 11_644_473_600 s × 1e9 ns/s.
        const EXPECTED: u64 = 11_644_473_600 * 1_000_000_000;
        assert_eq!(UNIX_TO_OPCUA_EPOCH_NS, EXPECTED);
    }
}
