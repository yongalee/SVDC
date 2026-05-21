//! `svdc-ingress` — south-bound ingress (M1) for the SVDC.
//!
//! This crate owns the path from raw IEC 61850-9-2 LE frames on the wire
//! to a stream of decoded samples that the aligner (M2) consumes. The
//! four WBS-2 items each become a submodule:
//!
//! | WBS    | module      | responsibility                                           |
//! | ------ | ----------- | -------------------------------------------------------- |
//! | 2.1    | [`subscriber`] | Raw L2 capture (`AF_PACKET` on Linux, Npcap on Windows). |
//! | 2.2    | [`decoder`]    | BER decode of one or more ASDUs per frame.               |
//! | 2.3    | [`timestamp`]  | Hardware / kernel ingress timestamp extraction (PTP-aware). |
//! | 2.4    | [`ring`]       | SPSC ring carrying decoded records from M1 to M2.        |
//!
//! Phase 0 scaffold scope — only the type surfaces and one happy-path
//! integration test (loopback subscriber → decoder → ring → drain). Real
//! capture and the SPSC ring land in Phase 1; PTP timestamping lands in
//! Phase 5. See `docs/decisions/0008-ingress-design.md` for rationale.
//!
//! OWNER: claude-code (Phase 0 scaffold). Phase 1 hot-path work is
//! assigned to Antigravity per ADR-0008.
//! NFR-10: English-only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod decoder;
pub mod ring;
pub mod subscriber;
pub mod timestamp;
pub mod udp;

pub use decoder::{DecodedSample, Decoder};
pub use ring::IngressRing;
pub use subscriber::{LoopbackSubscriber, Subscriber, SubscriberError};
pub use timestamp::IngressTimestamp;
pub use udp::UdpSubscriber;

/// One decoded SV frame as seen by the aligner: the frame's logical
/// timestamp and the decoded ASDU payload. This is the unit that flows
/// across the M1→M2 boundary in the SPSC ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressFrame {
    /// Kernel/PTP ingress timestamp. Set by [`subscriber`].
    pub timestamp: IngressTimestamp,
    /// One sample-data record per ASDU in the frame. Phase 0
    /// publishers emit one ASDU per frame; the aligner must already
    /// tolerate multi-ASDU per FR-1.
    pub samples: Vec<DecodedSample>,
}
