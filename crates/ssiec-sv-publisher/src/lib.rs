//! IEC 61850-9-2 LE Sampled Value encoder, plus a companion decoder for
//! round-trip tests, a PCAP writer for Wireshark inspection, and a hex
//! dumper for stdout.
//!
//! Phase 0 scope per ADR-0003:
//! - One SV ASDU per frame, 8 channels (Ia Ib Ic In Va Vb Vc Vn).
//! - Hand-rolled BER over a `&mut [u8]` cursor — no heap allocation.
//! - Frame layout: Ethernet II + 9-2 LE header + savPdu (APPLICATION 0).
//!
//! See `docs/decisions/0003-sv-encoder-design.md` for rationale.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod calibration_loader;
pub mod pcap_writer;
pub mod vendor;
pub mod vendor_loader;
pub mod waveform;

pub use pcap_writer::PcapWriter;
pub use vendor::{VendorProfile, VlanTag};
pub use waveform::WaveformConfig;

/// 802.1Q TPID prepended to a VLAN tag.
pub const TPID_8021Q: u16 = 0x8100;

use std::io::{self, Write};

/// Ethertype assigned to IEC 61850-9-2 Sampled Values.
pub const ETHERTYPE_SV: u16 = 0x88BA;

/// Default APPID used by the Phase 0 frame. Real deployments derive this
/// from the SCD.
pub const DEFAULT_APPID: u16 = 0x4000;

/// Number of channels per 9-2 LE Phsmeas9 dataset.
pub const NUM_CHANNELS: usize = 8;

/// Bytes per channel in the sample payload (i32 value + u32 quality).
pub const BYTES_PER_CHANNEL: usize = 8;

/// Total sample payload bytes per ASDU (8 channels × 8 bytes).
pub const SAMPLE_PAYLOAD_BYTES: usize = NUM_CHANNELS * BYTES_PER_CHANNEL;

/// Channel labels in the order the standard expects.
pub const CHANNEL_LABELS: [&str; NUM_CHANNELS] = ["Ia", "Ib", "Ic", "In", "Va", "Vb", "Vc", "Vn"];

/// One channel sample: scaled value plus 32-bit quality flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSample {
    /// Instantaneous value in scaled units (per the 9-2 LE guideline:
    /// currents in 0.001 A LSB, voltages in 0.01 V LSB).
    pub value: i32,
    /// IEC 61850-7-3 quality bits packed into a u32; 0 means "good".
    pub quality: u32,
}

impl ChannelSample {
    /// Create a good-quality sample.
    pub const fn good(value: i32) -> Self {
        Self { value, quality: 0 }
    }
}

/// Sample data carried in one ASDU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleData {
    /// Eight channels in the order [Ia Ib Ic In Va Vb Vc Vn].
    pub channels: [ChannelSample; NUM_CHANNELS],
}

impl SampleData {
    /// Nominal 60 Hz / 230 V / 5 A snapshot at 0° phase. Useful as a
    /// known-good test vector and as the Phase 0 hardcoded payload.
    pub const NOMINAL_3PH: SampleData = SampleData {
        channels: [
            ChannelSample::good(5000),   // Ia: cos(0)*5A
            ChannelSample::good(-2500),  // Ib: cos(-120°)*5A
            ChannelSample::good(-2500),  // Ic: cos(120°)*5A
            ChannelSample::good(0),      // In
            ChannelSample::good(23000),  // Va: cos(0)*230V
            ChannelSample::good(-11500), // Vb
            ChannelSample::good(-11500), // Vc
            ChannelSample::good(0),      // Vn
        ],
    };

    /// Pack the 8 channels into the 64-byte SV payload (big-endian).
    pub fn pack(&self, out: &mut [u8; SAMPLE_PAYLOAD_BYTES]) {
        for (i, ch) in self.channels.iter().enumerate() {
            let base = i * BYTES_PER_CHANNEL;
            out[base..base + 4].copy_from_slice(&ch.value.to_be_bytes());
            out[base + 4..base + 8].copy_from_slice(&ch.quality.to_be_bytes());
        }
    }

    /// Unpack 64 bytes of SV payload into 8 channels.
    pub fn unpack(bytes: &[u8; SAMPLE_PAYLOAD_BYTES]) -> Self {
        let mut channels = [ChannelSample {
            value: 0,
            quality: 0,
        }; NUM_CHANNELS];
        for (i, ch) in channels.iter_mut().enumerate() {
            let base = i * BYTES_PER_CHANNEL;
            ch.value = i32::from_be_bytes(bytes[base..base + 4].try_into().unwrap());
            ch.quality = u32::from_be_bytes(bytes[base + 4..base + 8].try_into().unwrap());
        }
        SampleData { channels }
    }
}

/// One ASDU's worth of header fields, plus the sample payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsduFields<'a> {
    /// SV identifier (e.g. "SVDC_DEMO_01"). ASCII only.
    pub sv_id: &'a str,
    /// Monotonic sample counter (0..=65535, wraps per cycle).
    pub smp_cnt: u16,
    /// SCD configuration revision.
    pub conf_rev: u32,
    /// 0 = none, 1 = local, 2 = global (per IEC 61850-9-2 ed.2).
    pub smp_synch: u8,
    /// Sample rate in Hz (e.g. 4800 = 80 SPC × 60 Hz).
    pub smp_rate: u16,
    /// The 8 channel samples.
    pub samples: SampleData,
}

/// Frame-level parameters: Ethernet MACs, APPID, optional VLAN tag.
#[derive(Debug, Clone, Copy)]
pub struct FrameParams {
    /// Destination MAC. SV multicast range is 01:0C:CD:04:00:00..01:FF.
    pub dst_mac: [u8; 6],
    /// Source MAC.
    pub src_mac: [u8; 6],
    /// 9-2 LE APPID (typically 0x4000).
    pub appid: u16,
    /// Optional 802.1Q VLAN tag. When `Some`, the encoder emits
    /// `[TPID 0x8100][TCI]` between the source MAC and the EtherType.
    /// Real substation MUs almost always tag with PCP = 4.
    pub vlan: Option<vendor::VlanTag>,
}

impl FrameParams {
    /// Phase 0 demo defaults.
    pub const DEMO: FrameParams = FrameParams {
        dst_mac: [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01],
        src_mac: [0x02, 0x00, 0x00, 0x00, 0x00, 0x01],
        appid: DEFAULT_APPID,
        vlan: None,
    };

    /// Build frame parameters that look like the given vendor's
    /// merging unit. `unit_suffix` is the last three octets of the
    /// source MAC (normally a unit serial-number low-bits value).
    pub fn from_vendor(profile: &VendorProfile, unit_suffix: [u8; 3]) -> Self {
        Self {
            dst_mac: profile.multicast_mac,
            src_mac: profile.source_mac(unit_suffix),
            appid: profile.default_appid,
            vlan: profile.vlan,
        }
    }
}

/// Errors the encoder can produce. All map to "output buffer too small"
/// in Phase 0; richer variants land in Phase 1.
#[derive(Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The provided output buffer cannot hold the encoded frame.
    BufferTooSmall {
        /// Bytes the encoder needed.
        needed: usize,
        /// Bytes the caller offered.
        available: usize,
    },
    /// An ASN.1 length exceeded 65535, which the Phase 0 long-form
    /// length encoding does not support.
    LengthExceeds16Bit(usize),
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EncodeError::BufferTooSmall { needed, available } => write!(
                f,
                "output buffer too small: need {needed} bytes, have {available}"
            ),
            EncodeError::LengthExceeds16Bit(n) => {
                write!(f, "ASN.1 length {n} exceeds 16-bit long form")
            }
        }
    }
}

impl std::error::Error for EncodeError {}

/// Write-cursor over a fixed-size buffer. No allocation.
struct Cursor<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        if self.remaining() < bytes.len() {
            return Err(EncodeError::BufferTooSmall {
                needed: self.pos + bytes.len(),
                available: self.buf.len(),
            });
        }
        self.buf[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }

    fn write_u8(&mut self, byte: u8) -> Result<(), EncodeError> {
        self.write(&[byte])
    }

    fn write_u16_be(&mut self, value: u16) -> Result<(), EncodeError> {
        self.write(&value.to_be_bytes())
    }

    /// Reserve the BER long-form length placeholder `0x82 LL LL` and
    /// return the position of the first length byte so it can be patched
    /// after the inner element is written.
    fn reserve_length16(&mut self) -> Result<usize, EncodeError> {
        self.write_u8(0x82)?;
        let len_pos = self.pos;
        self.write_u16_be(0)?;
        Ok(len_pos)
    }

    /// Patch a previously reserved length to the bytes written since
    /// `len_pos + 2` (i.e., since just after the placeholder).
    fn patch_length16(&mut self, len_pos: usize) -> Result<(), EncodeError> {
        let inner_len = self.pos - (len_pos + 2);
        if inner_len > u16::MAX as usize {
            return Err(EncodeError::LengthExceeds16Bit(inner_len));
        }
        let bytes = (inner_len as u16).to_be_bytes();
        self.buf[len_pos] = bytes[0];
        self.buf[len_pos + 1] = bytes[1];
        Ok(())
    }
}

/// Encode `[tag] LL value` as a primitive BER TLV with short or long-form
/// length encoding chosen automatically.
fn write_primitive_tlv(cur: &mut Cursor<'_>, tag: u8, value: &[u8]) -> Result<(), EncodeError> {
    cur.write_u8(tag)?;
    if value.len() < 128 {
        cur.write_u8(value.len() as u8)?;
    } else if value.len() <= u16::MAX as usize {
        cur.write_u8(0x82)?;
        cur.write_u16_be(value.len() as u16)?;
    } else {
        return Err(EncodeError::LengthExceeds16Bit(value.len()));
    }
    cur.write(value)?;
    Ok(())
}

/// BER INTEGER minimal encoding for a u16 (always positive; never has
/// a leading 0x80 bit set because u16::MAX < 0x10000).
fn encode_int_u16(value: u16) -> ([u8; 3], usize) {
    let mut buf = [0u8; 3];
    if value == 0 {
        buf[0] = 0;
        (buf, 1)
    } else if value < 0x80 {
        buf[0] = value as u8;
        (buf, 1)
    } else if value < 0x8000 {
        buf[0] = (value >> 8) as u8;
        buf[1] = (value & 0xFF) as u8;
        (buf, 2)
    } else {
        buf[0] = 0x00; // leading zero so sign bit stays positive
        buf[1] = (value >> 8) as u8;
        buf[2] = (value & 0xFF) as u8;
        (buf, 3)
    }
}

/// Encode a complete 9-2 LE SV frame (Ethernet II + 9-2 header + savPdu)
/// into `out`. Returns the number of bytes written.
pub fn encode_frame(
    params: &FrameParams,
    asdu: &AsduFields<'_>,
    out: &mut [u8],
) -> Result<usize, EncodeError> {
    let mut cur = Cursor::new(out);

    // Ethernet II header. If a VLAN tag is configured, insert the
    // 802.1Q [TPID][TCI] pair between the source MAC and the
    // SV EtherType — every real merging unit tags its SV traffic
    // with PCP = 4 per the 9-2 LE Implementation Guideline.
    cur.write(&params.dst_mac)?;
    cur.write(&params.src_mac)?;
    if let Some(tag) = params.vlan {
        cur.write_u16_be(TPID_8021Q)?;
        cur.write_u16_be(tag.tci())?;
    }
    cur.write_u16_be(ETHERTYPE_SV)?;

    // 9-2 LE header: APPID + Length (patched later) + 2 reserved words.
    cur.write_u16_be(params.appid)?;
    let nine2_len_pos = cur.pos;
    cur.write_u16_be(0)?; // length placeholder, patched at the end
    cur.write_u16_be(0)?; // Reserved1
    cur.write_u16_be(0)?; // Reserved2
    let nine2_payload_start = cur.pos;

    // savPdu  [APPLICATION 0] IMPLICIT SEQUENCE — tag 0x60, constructed.
    cur.write_u8(0x60)?;
    let savpdu_len_pos = cur.reserve_length16()?;

    // noASDU [0] IMPLICIT INTEGER = 1
    let (no_asdu_buf, no_asdu_n) = encode_int_u16(1);
    write_primitive_tlv(&mut cur, 0x80, &no_asdu_buf[..no_asdu_n])?;

    // asdu [2] IMPLICIT SEQUENCE OF — constructed context-specific.
    cur.write_u8(0xA2)?;
    let asdu_seq_len_pos = cur.reserve_length16()?;

    // One ASDU: SEQUENCE (universal constructed, tag 0x30).
    cur.write_u8(0x30)?;
    let asdu_len_pos = cur.reserve_length16()?;

    // svID [0] IMPLICIT VisibleString (primitive context-specific).
    write_primitive_tlv(&mut cur, 0x80, asdu.sv_id.as_bytes())?;

    // smpCnt [2] IMPLICIT OCTET STRING (size 2) — u16 BE.
    write_primitive_tlv(&mut cur, 0x82, &asdu.smp_cnt.to_be_bytes())?;

    // confRev [3] IMPLICIT OCTET STRING (size 4) — u32 BE.
    write_primitive_tlv(&mut cur, 0x83, &asdu.conf_rev.to_be_bytes())?;

    // smpSynch [5] IMPLICIT OCTET STRING (size 1).
    write_primitive_tlv(&mut cur, 0x85, &[asdu.smp_synch])?;

    // smpRate [6] IMPLICIT INTEGER.
    let (rate_buf, rate_n) = encode_int_u16(asdu.smp_rate);
    write_primitive_tlv(&mut cur, 0x86, &rate_buf[..rate_n])?;

    // sample [7] IMPLICIT OCTET STRING (64 bytes of packed channels).
    let mut payload = [0u8; SAMPLE_PAYLOAD_BYTES];
    asdu.samples.pack(&mut payload);
    write_primitive_tlv(&mut cur, 0x87, &payload)?;

    // Patch lengths inside-out.
    cur.patch_length16(asdu_len_pos)?;
    cur.patch_length16(asdu_seq_len_pos)?;
    cur.patch_length16(savpdu_len_pos)?;

    // The 9-2 LE Length field includes the 8-byte 9-2 header itself.
    let nine2_payload_len = cur.pos - nine2_payload_start;
    let nine2_total = nine2_payload_len + 8;
    if nine2_total > u16::MAX as usize {
        return Err(EncodeError::LengthExceeds16Bit(nine2_total));
    }
    let bytes = (nine2_total as u16).to_be_bytes();
    cur.buf[nine2_len_pos] = bytes[0];
    cur.buf[nine2_len_pos + 1] = bytes[1];

    Ok(cur.pos)
}

/// Worst-case frame size for the Phase 0 layout. Safe to allocate this
/// once on the stack and reuse.
///
/// Layout budget:
///   Eth header              14
///   Optional 802.1Q tag      4   (TPID 0x8100 + TCI)
///   9-2 LE header            8
///   savPdu outer tag+len     4   (0x60 0x82 LL LL)
///     noASDU                 5   (0x80 0x03 + up to 3 bytes)
///     asdu seq tag+len       4   (0xA2 0x82 LL LL)
///       ASDU tag+len         4   (0x30 0x82 LL LL)
///         svID               4 + 96  (tag+len + up to 96 ASCII chars — vendor profiles include full functional names)
///         smpCnt             4
///         confRev            6
///         smpSynch           3
///         smpRate            5
///         sample             4 + 64  (tag+len + payload)
///   Total                  < 320 bytes
pub const MAX_FRAME_BYTES: usize = 320;

// ---------------------------------------------------------------------------
//  Minimal decoder for round-trip tests.
// ---------------------------------------------------------------------------

/// Decoder errors.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Input ended before the expected element completed.
    Truncated,
    /// A tag did not match the expected sequence.
    UnexpectedTag {
        /// Tag byte that was found.
        found: u8,
        /// Tag byte that was expected.
        expected: u8,
    },
    /// The frame's EtherType is not 0x88BA.
    NotSvFrame {
        /// EtherType actually observed.
        ethertype: u16,
    },
    /// A length field encoded with a form the Phase 0 decoder does not
    /// support (longer than `0x82 LL LL`).
    UnsupportedLengthForm(u8),
    /// An OCTET STRING did not have the expected size.
    BadSize {
        /// Expected octet-string size.
        expected: usize,
        /// Octet-string size actually decoded.
        actual: usize,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::Truncated => write!(f, "input truncated"),
            DecodeError::UnexpectedTag { found, expected } => {
                write!(f, "expected tag 0x{expected:02X}, found 0x{found:02X}")
            }
            DecodeError::NotSvFrame { ethertype } => {
                write!(f, "EtherType 0x{ethertype:04X} is not SV (0x88BA)")
            }
            DecodeError::UnsupportedLengthForm(b) => {
                write!(f, "unsupported BER length form (first byte 0x{b:02X})")
            }
            DecodeError::BadSize { expected, actual } => {
                write!(f, "expected {expected}-byte octet string, got {actual}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Owned snapshot of a decoded frame. Used in tests for assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    /// Destination MAC.
    pub dst_mac: [u8; 6],
    /// Source MAC.
    pub src_mac: [u8; 6],
    /// 9-2 LE APPID.
    pub appid: u16,
    /// 9-2 LE Length field value (includes the 8-byte 9-2 header).
    pub length: u16,
    /// Number of ASDUs declared.
    pub no_asdu: u16,
    /// First (and only, in Phase 0) ASDU.
    pub asdu: DecodedAsdu,
}

/// One decoded ASDU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAsdu {
    /// SV identifier.
    pub sv_id: String,
    /// Sample counter.
    pub smp_cnt: u16,
    /// Configuration revision.
    pub conf_rev: u32,
    /// Synchronization state (0=none, 1=local, 2=global).
    pub smp_synch: u8,
    /// Sample rate in Hz.
    pub smp_rate: u16,
    /// 64-byte channel payload as 8 channels.
    pub samples: SampleData,
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, DecodeError> {
        let b = *self.buf.get(self.pos).ok_or(DecodeError::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    fn read_u16_be(&mut self) -> Result<u16, DecodeError> {
        if self.pos + 2 > self.buf.len() {
            return Err(DecodeError::Truncated);
        }
        let v = u16::from_be_bytes([self.buf[self.pos], self.buf[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.pos + n > self.buf.len() {
            return Err(DecodeError::Truncated);
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn read_ber_length(&mut self) -> Result<usize, DecodeError> {
        let first = self.read_u8()?;
        if first < 0x80 {
            Ok(first as usize)
        } else if first == 0x81 {
            Ok(self.read_u8()? as usize)
        } else if first == 0x82 {
            Ok(self.read_u16_be()? as usize)
        } else {
            Err(DecodeError::UnsupportedLengthForm(first))
        }
    }

    fn expect_tag(&mut self, expected: u8) -> Result<(), DecodeError> {
        let found = self.read_u8()?;
        if found != expected {
            return Err(DecodeError::UnexpectedTag { found, expected });
        }
        Ok(())
    }

    fn read_tlv(&mut self, expected_tag: u8) -> Result<&'a [u8], DecodeError> {
        self.expect_tag(expected_tag)?;
        let len = self.read_ber_length()?;
        self.read_bytes(len)
    }
}

fn decode_int(bytes: &[u8]) -> u32 {
    let mut v: u32 = 0;
    for &b in bytes {
        v = (v << 8) | (b as u32);
    }
    v
}

/// Decode an SV frame produced by [`encode_frame`].
///
/// Phase 0 only — supports exactly the field set the encoder produces,
/// in the order it produces them. The real SVDC ingress decoder lives
/// in `svdc-ingress` (Phase 1).
pub fn decode_frame(buf: &[u8]) -> Result<DecodedFrame, DecodeError> {
    let mut r = Reader::new(buf);

    let dst = r.read_bytes(6)?;
    let mut dst_mac = [0u8; 6];
    dst_mac.copy_from_slice(dst);
    let src = r.read_bytes(6)?;
    let mut src_mac = [0u8; 6];
    src_mac.copy_from_slice(src);
    // The next u16 is either the SV EtherType directly, or an
    // 802.1Q TPID (0x8100) that introduces a 4-byte VLAN tag.
    // Skip over the tag and re-read the EtherType when present.
    let mut ethertype = r.read_u16_be()?;
    if ethertype == TPID_8021Q {
        let _tci = r.read_u16_be()?;
        ethertype = r.read_u16_be()?;
    }
    if ethertype != ETHERTYPE_SV {
        return Err(DecodeError::NotSvFrame { ethertype });
    }

    let appid = r.read_u16_be()?;
    let length = r.read_u16_be()?;
    let _res1 = r.read_u16_be()?;
    let _res2 = r.read_u16_be()?;

    // savPdu APPLICATION 0 IMPLICIT SEQUENCE.
    r.expect_tag(0x60)?;
    let _savpdu_len = r.read_ber_length()?;

    let no_asdu_bytes = r.read_tlv(0x80)?;
    let no_asdu = decode_int(no_asdu_bytes) as u16;

    // asdu [2] IMPLICIT SEQUENCE OF.
    r.expect_tag(0xA2)?;
    let _asdu_seq_len = r.read_ber_length()?;

    // First ASDU: SEQUENCE (universal constructed).
    r.expect_tag(0x30)?;
    let _asdu_len = r.read_ber_length()?;

    let sv_id_bytes = r.read_tlv(0x80)?;
    let sv_id = String::from_utf8_lossy(sv_id_bytes).into_owned();

    let smp_cnt_bytes = r.read_tlv(0x82)?;
    if smp_cnt_bytes.len() != 2 {
        return Err(DecodeError::BadSize {
            expected: 2,
            actual: smp_cnt_bytes.len(),
        });
    }
    let smp_cnt = u16::from_be_bytes([smp_cnt_bytes[0], smp_cnt_bytes[1]]);

    let conf_rev_bytes = r.read_tlv(0x83)?;
    if conf_rev_bytes.len() != 4 {
        return Err(DecodeError::BadSize {
            expected: 4,
            actual: conf_rev_bytes.len(),
        });
    }
    let conf_rev = u32::from_be_bytes([
        conf_rev_bytes[0],
        conf_rev_bytes[1],
        conf_rev_bytes[2],
        conf_rev_bytes[3],
    ]);

    let smp_synch_bytes = r.read_tlv(0x85)?;
    if smp_synch_bytes.len() != 1 {
        return Err(DecodeError::BadSize {
            expected: 1,
            actual: smp_synch_bytes.len(),
        });
    }
    let smp_synch = smp_synch_bytes[0];

    let smp_rate_bytes = r.read_tlv(0x86)?;
    let smp_rate = decode_int(smp_rate_bytes) as u16;

    let sample_bytes = r.read_tlv(0x87)?;
    if sample_bytes.len() != SAMPLE_PAYLOAD_BYTES {
        return Err(DecodeError::BadSize {
            expected: SAMPLE_PAYLOAD_BYTES,
            actual: sample_bytes.len(),
        });
    }
    let mut sample_arr = [0u8; SAMPLE_PAYLOAD_BYTES];
    sample_arr.copy_from_slice(sample_bytes);
    let samples = SampleData::unpack(&sample_arr);

    Ok(DecodedFrame {
        dst_mac,
        src_mac,
        appid,
        length,
        no_asdu,
        asdu: DecodedAsdu {
            sv_id,
            smp_cnt,
            conf_rev,
            smp_synch,
            smp_rate,
            samples,
        },
    })
}

// ---------------------------------------------------------------------------
//  Output sinks.
// ---------------------------------------------------------------------------

/// Write a "hexdump -C"-style rendering of `bytes` to `out`.
pub fn write_hex_dump<W: Write>(out: &mut W, bytes: &[u8]) -> io::Result<()> {
    for (offset, chunk) in bytes.chunks(16).enumerate() {
        write!(out, "{:08x}  ", offset * 16)?;
        for i in 0..16 {
            if i < chunk.len() {
                write!(out, "{:02x} ", chunk[i])?;
            } else {
                write!(out, "   ")?;
            }
            if i == 7 {
                write!(out, " ")?;
            }
        }
        write!(out, " |")?;
        for &b in chunk {
            let c = if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '.'
            };
            write!(out, "{c}")?;
        }
        writeln!(out, "|")?;
    }
    Ok(())
}

/// libpcap file global header: little-endian magic + version + tz/sigfig
/// + snaplen + link type. Link type 1 = LINKTYPE_ETHERNET.
const PCAP_GLOBAL_HEADER: [u8; 24] = [
    0xD4, 0xC3, 0xB2, 0xA1, // magic (microsecond resolution)
    0x02, 0x00, 0x04, 0x00, // version major=2, minor=4
    0x00, 0x00, 0x00, 0x00, // thiszone
    0x00, 0x00, 0x00, 0x00, // sigfigs
    0xFF, 0xFF, 0x00, 0x00, // snaplen 65535
    0x01, 0x00, 0x00, 0x00, // network = LINKTYPE_ETHERNET
];

/// Write a single-record PCAP file containing one Ethernet frame.
pub fn write_pcap<W: Write>(out: &mut W, frame: &[u8]) -> io::Result<()> {
    out.write_all(&PCAP_GLOBAL_HEADER)?;
    // Record header: ts_sec, ts_usec, incl_len, orig_len. Timestamps
    // are fixed at zero; Wireshark only displays them.
    let mut rec = [0u8; 16];
    rec[0..4].copy_from_slice(&0u32.to_le_bytes()); // ts_sec
    rec[4..8].copy_from_slice(&0u32.to_le_bytes()); // ts_usec
    rec[8..12].copy_from_slice(&(frame.len() as u32).to_le_bytes()); // incl_len
    rec[12..16].copy_from_slice(&(frame.len() as u32).to_le_bytes()); // orig_len
    out.write_all(&rec)?;
    out.write_all(frame)?;
    Ok(())
}

// ---------------------------------------------------------------------------
//  Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_asdu() -> AsduFields<'static> {
        AsduFields {
            sv_id: "SVDC_DEMO_01",
            smp_cnt: 0,
            conf_rev: 1,
            smp_synch: 2,
            smp_rate: 4800,
            samples: SampleData::NOMINAL_3PH,
        }
    }

    #[test]
    fn encode_then_decode_roundtrips() {
        let mut buf = [0u8; MAX_FRAME_BYTES];
        let n = encode_frame(&FrameParams::DEMO, &demo_asdu(), &mut buf).unwrap();
        let frame = &buf[..n];

        let decoded = decode_frame(frame).unwrap();
        assert_eq!(decoded.dst_mac, FrameParams::DEMO.dst_mac);
        assert_eq!(decoded.src_mac, FrameParams::DEMO.src_mac);
        assert_eq!(decoded.appid, DEFAULT_APPID);
        assert_eq!(decoded.no_asdu, 1);
        assert_eq!(decoded.asdu.sv_id, "SVDC_DEMO_01");
        assert_eq!(decoded.asdu.smp_cnt, 0);
        assert_eq!(decoded.asdu.conf_rev, 1);
        assert_eq!(decoded.asdu.smp_synch, 2);
        assert_eq!(decoded.asdu.smp_rate, 4800);
        assert_eq!(decoded.asdu.samples, SampleData::NOMINAL_3PH);
    }

    #[test]
    fn frame_length_field_matches_actual_bytes() {
        let mut buf = [0u8; MAX_FRAME_BYTES];
        let n = encode_frame(&FrameParams::DEMO, &demo_asdu(), &mut buf).unwrap();
        let decoded = decode_frame(&buf[..n]).unwrap();
        // The Length field is the count from the 9-2 LE header start
        // (APPID) through end of savPdu, inclusive of the 8-byte header.
        let expected = n - 14; // total minus Ethernet header
        assert_eq!(decoded.length as usize, expected);
    }

    #[test]
    fn frame_starts_with_sv_multicast_mac_and_ethertype() {
        let mut buf = [0u8; MAX_FRAME_BYTES];
        let n = encode_frame(&FrameParams::DEMO, &demo_asdu(), &mut buf).unwrap();
        // First 3 bytes are 01:0C:CD per the 9-2 multicast assignment.
        assert_eq!(&buf[..3], &[0x01, 0x0C, 0xCD]);
        // EtherType at offset 12..14 is 0x88BA.
        assert_eq!(&buf[12..14], &[0x88, 0xBA]);
        // 9-2 LE APPID at offset 14..16 is 0x4000.
        assert_eq!(&buf[14..16], &[0x40, 0x00]);
        // Bounds: encoded length < MAX.
        assert!(n < MAX_FRAME_BYTES);
    }

    #[test]
    fn sample_data_pack_unpack_roundtrips() {
        let original = SampleData::NOMINAL_3PH;
        let mut buf = [0u8; SAMPLE_PAYLOAD_BYTES];
        original.pack(&mut buf);
        let recovered = SampleData::unpack(&buf);
        assert_eq!(recovered, original);
    }

    #[test]
    fn buffer_too_small_returns_error() {
        let mut tiny = [0u8; 8];
        let r = encode_frame(&FrameParams::DEMO, &demo_asdu(), &mut tiny);
        assert!(matches!(r, Err(EncodeError::BufferTooSmall { .. })));
    }

    #[test]
    fn pcap_file_has_correct_header() {
        let mut out = Vec::new();
        let mut buf = [0u8; MAX_FRAME_BYTES];
        let n = encode_frame(&FrameParams::DEMO, &demo_asdu(), &mut buf).unwrap();
        write_pcap(&mut out, &buf[..n]).unwrap();
        assert_eq!(&out[..4], &[0xD4, 0xC3, 0xB2, 0xA1]); // little-endian magic
        assert_eq!(&out[20..24], &[0x01, 0x00, 0x00, 0x00]); // LINKTYPE_ETHERNET
        assert_eq!(out.len(), 24 + 16 + n);
    }

    #[test]
    fn hex_dump_has_offsets_and_ascii_gutter() {
        let mut out = Vec::new();
        write_hex_dump(&mut out, &[0x48, 0x69, 0x21, 0x00]).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("00000000"));
        assert!(s.contains("|Hi!.|"));
    }
}
