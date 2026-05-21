//! WBS-2.1 — IEC 61850-9-2 SV Subscriber surface.
//!
//! On Linux the production subscriber is an `AF_PACKET` raw socket bound
//! to a NIC, optionally with hardware timestamping (`SO_TIMESTAMPING`).
//! On Windows the equivalent is a Npcap raw-capture handle. Phase 0
//! exposes only the *trait* both will implement and a [`LoopbackSubscriber`]
//! that yields prebuilt frames in order — sufficient for integration
//! tests that exercise the decoder + ring without any I/O.
//!
//! Real implementations land in Phase 1 (Linux) and Phase 5 (Windows).
//! See `docs/decisions/0008-ingress-design.md` §3 for the design.

use std::collections::VecDeque;

/// Errors a subscriber can report. Phase 1 will extend this with
/// `Io`, `Truncated`, `Closed`, and `BadInterface` variants.
#[derive(Debug)]
pub enum SubscriberError {
    /// No more frames to yield (loopback only). Real subscribers
    /// block on the socket and never return `Closed` unless the NIC
    /// is shut down or the daemon is exiting.
    Closed,
}

impl std::fmt::Display for SubscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriberError::Closed => write!(f, "subscriber: closed"),
        }
    }
}

impl std::error::Error for SubscriberError {}

/// Pull-style frame source. One call yields one L2 frame (including
/// Ethernet header) or an error. Phase 1 will revisit whether a push
/// style (callback into the ring) is cheaper on the hot path.
pub trait Subscriber {
    /// Yield the next L2 frame. Returns the slice of bytes plus a
    /// best-effort ingress timestamp. Caller owns the returned `Vec`.
    fn next_frame(&mut self) -> Result<(Vec<u8>, super::IngressTimestamp), SubscriberError>;
}

/// Test-only [`Subscriber`] that yields a fixed sequence of frames.
/// Each frame is paired with an explicit timestamp so unit tests can
/// reproduce the exact timing the aligner will see.
#[derive(Debug, Default)]
pub struct LoopbackSubscriber {
    queue: VecDeque<(Vec<u8>, super::IngressTimestamp)>,
}

impl LoopbackSubscriber {
    /// Empty subscriber. Push frames with [`Self::push_frame`] before draining.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one (frame, timestamp) pair to the queue.
    pub fn push_frame(&mut self, frame: Vec<u8>, ts: super::IngressTimestamp) {
        self.queue.push_back((frame, ts));
    }

    /// Number of frames still queued.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }
}

impl Subscriber for LoopbackSubscriber {
    fn next_frame(&mut self) -> Result<(Vec<u8>, super::IngressTimestamp), SubscriberError> {
        self.queue.pop_front().ok_or(SubscriberError::Closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IngressTimestamp;

    #[test]
    fn loopback_yields_frames_in_order_then_closes() {
        let mut sub = LoopbackSubscriber::new();
        sub.push_frame(vec![0xAA, 0xBB], IngressTimestamp::from_unix_ns(100));
        sub.push_frame(vec![0xCC, 0xDD], IngressTimestamp::from_unix_ns(200));
        assert_eq!(sub.pending(), 2);

        let (frame, ts) = sub.next_frame().unwrap();
        assert_eq!(frame, vec![0xAA, 0xBB]);
        assert_eq!(ts.unix_ns(), 100);

        let (frame, ts) = sub.next_frame().unwrap();
        assert_eq!(frame, vec![0xCC, 0xDD]);
        assert_eq!(ts.unix_ns(), 200);

        assert!(matches!(sub.next_frame(), Err(SubscriberError::Closed)));
    }
}
