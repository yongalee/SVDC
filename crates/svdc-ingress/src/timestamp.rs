//! WBS-2.3 — ingress timestamp.
//!
//! The aligner (M2) bins samples into time slots by *ingress* timestamp,
//! not by the publisher-side `smpCnt` — the latter is only the
//! publisher's idea of time. In production the ingress timestamp comes
//! from the kernel via `SO_TIMESTAMPING`, ideally driven by a
//! PTP-disciplined NIC (Phase 5: linuxptp / IEC 61850-9-3). For Phase 0
//! we use `SystemTime::now()` so the scaffold is self-contained.
//!
//! Resolution is nanoseconds; the storage unit is `u64` Unix-ns so
//! comparisons and ring layouts stay simple. PTP frames will produce a
//! synchronised version of the same nanosecond grid.

use std::time::{SystemTime, UNIX_EPOCH};

/// Best-effort kernel ingress timestamp. Phase 0 fills this from the
/// host monotonic clock; Phase 5 will source it from `SO_TIMESTAMPING`
/// hardware-tx timestamps via the raw socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IngressTimestamp {
    unix_ns: u64,
}

impl IngressTimestamp {
    /// Construct from raw Unix nanoseconds. Useful for tests.
    pub const fn from_unix_ns(unix_ns: u64) -> Self {
        Self { unix_ns }
    }

    /// Capture "now" from the host clock. Phase 5 will replace the
    /// call-site of this helper with the `SO_TIMESTAMPING` value
    /// returned by `recvmsg`.
    pub fn now() -> Self {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        Self { unix_ns: ns }
    }

    /// Raw Unix nanoseconds.
    pub const fn unix_ns(&self) -> u64 {
        self.unix_ns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_unix_ns_round_trips() {
        let ts = IngressTimestamp::from_unix_ns(12_345_678_901);
        assert_eq!(ts.unix_ns(), 12_345_678_901);
    }

    #[test]
    fn now_is_after_unix_epoch_2020() {
        // Sanity: now() should be >= Jan 1 2020 in Unix ns. Catches
        // clocks broken to 1970 or wall-time misconfiguration.
        let jan2020_ns: u64 = 1_577_836_800_000_000_000;
        assert!(IngressTimestamp::now().unix_ns() > jan2020_ns);
    }

    #[test]
    fn ordering_is_chronological() {
        let a = IngressTimestamp::from_unix_ns(100);
        let b = IngressTimestamp::from_unix_ns(200);
        assert!(a < b);
    }
}
