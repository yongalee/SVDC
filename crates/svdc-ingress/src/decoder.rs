//! WBS-2.2 — IEC 61850-9-2 LE decoder surface.
//!
//! Phase 0 reuses the round-trip BER decoder already proven by
//! `ssiec-sv-publisher` (PR #2 / WBS-6.1). The phase 1 owner may choose
//! to either keep this delegation, fork a richer ingress-side decoder
//! that tolerates vendor variants, or migrate the decoder here and
//! leave only the encoder in the publisher. ADR-0008 documents the
//! decision tree; the public type [`DecodedSample`] in this module is
//! the stable interface either choice has to honour.
//!
//! Status: scaffold. The real Phase 1 decoder must:
//!  - tolerate ≥ 1 ASDU per frame,
//!  - handle short-form (`0x80..0x81`) and long-form (`0x82`) BER lengths
//!    consistently with vendor traces collected during interop testing,
//!  - record a per-frame quality summary derived from per-channel
//!    quality flags.

use ssiec_sv_publisher::{decode_frame, decode_l2_stripped_frame, DecodeError, SampleData};

/// One ASDU's worth of decoded payload as the aligner sees it. Channel
/// units and calibration are *not* applied here — that lives in
/// `svdc-aligner` (WBS-2.7). This struct only carries the on-wire
/// integers + the metadata fields the aligner needs for binning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSample {
    /// SV identifier from the ASDU's `svID`.
    pub sv_id: String,
    /// Sample counter (`smpCnt`).
    pub smp_cnt: u16,
    /// Configuration revision (`confRev`).
    pub conf_rev: u32,
    /// Synchronisation state (`smpSynch`): 0 none, 1 local, 2 global.
    pub smp_synch: u8,
    /// Sample rate in Hz from the ASDU header (not the channel rate).
    pub smp_rate: u16,
    /// Eight-channel sample payload.
    pub samples: SampleData,
}

/// Decoder errors. Phase 0 wraps the underlying publisher decoder
/// errors; Phase 1 owners can extend with variants like
/// `WrongAppId`, `UnknownDataset`, `MultiAsduUnsupported`.
#[derive(Debug)]
pub enum DecodeFailure {
    /// Underlying BER decode failure.
    Decode(DecodeError),
}

impl std::fmt::Display for DecodeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeFailure::Decode(e) => write!(f, "SV decode failed: {e}"),
        }
    }
}

impl std::error::Error for DecodeFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DecodeFailure::Decode(e) => Some(e),
        }
    }
}

/// Phase 0 stateless decoder. Phase 1 will likely move to a stateful
/// decoder that maintains a per-`svID` parser cache + dataset map.
#[derive(Debug, Default)]
pub struct Decoder;

impl Decoder {
    /// Decode one frame into `DecodedSample`s. Phase 0 returns exactly
    /// one element per frame (publisher emits 1 ASDU/frame); the API
    /// returns a `Vec` so the multi-ASDU shape is stable from day 1.
    pub fn decode_frame(&self, frame: &[u8]) -> Result<Vec<DecodedSample>, DecodeFailure> {
        let dec = decode_frame(frame).map_err(DecodeFailure::Decode)?;
        Ok(Self::lift(dec.asdu))
    }

    /// Decode an L2-stripped payload (the buffer starts at the
    /// 9-2 LE APPID field, no Ethernet header). Use this on
    /// payloads received via [`UdpSubscriber`](crate::UdpSubscriber).
    pub fn decode_l2_stripped(&self, payload: &[u8]) -> Result<Vec<DecodedSample>, DecodeFailure> {
        let dec = decode_l2_stripped_frame(payload).map_err(DecodeFailure::Decode)?;
        Ok(Self::lift(dec.asdu))
    }

    fn lift(asdu: ssiec_sv_publisher::DecodedAsdu) -> Vec<DecodedSample> {
        vec![DecodedSample {
            sv_id: asdu.sv_id,
            smp_cnt: asdu.smp_cnt,
            conf_rev: asdu.conf_rev,
            smp_synch: asdu.smp_synch,
            smp_rate: asdu.smp_rate,
            samples: asdu.samples,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssiec_sv_publisher::{encode_frame, AsduFields, FrameParams, SampleData, MAX_FRAME_BYTES};

    #[test]
    fn decoder_round_trips_a_publisher_emitted_frame() {
        let mut buf = [0u8; MAX_FRAME_BYTES];
        let asdu = AsduFields {
            sv_id: "INGRESS_SCAFFOLD",
            smp_cnt: 7,
            conf_rev: 1,
            smp_synch: 2,
            smp_rate: 4800,
            samples: SampleData::NOMINAL_3PH,
        };
        let n = encode_frame(&FrameParams::DEMO, &asdu, &mut buf).unwrap();

        let decoded = Decoder.decode_frame(&buf[..n]).unwrap();
        assert_eq!(decoded.len(), 1);
        let s = &decoded[0];
        assert_eq!(s.sv_id, "INGRESS_SCAFFOLD");
        assert_eq!(s.smp_cnt, 7);
        assert_eq!(s.conf_rev, 1);
        assert_eq!(s.smp_synch, 2);
        assert_eq!(s.smp_rate, 4800);
        assert_eq!(s.samples, SampleData::NOMINAL_3PH);
    }

    #[test]
    fn corrupt_frame_returns_decode_failure() {
        let r = Decoder.decode_frame(&[0u8; 16]);
        assert!(matches!(r, Err(DecodeFailure::Decode(_))));
    }
}
