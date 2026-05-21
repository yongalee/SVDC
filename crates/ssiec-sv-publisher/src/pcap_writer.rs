//! Streaming libpcap writer for multi-frame SV captures.
//!
//! The Phase 0 [`write_pcap`](crate::write_pcap) helper packs exactly one
//! frame into a single-record file. Phase 1 needs a continuous stream
//! (hundreds to millions of frames at 4800 Hz), so this writer:
//!
//! 1. writes the libpcap global header once on construction,
//! 2. accepts incremental `write_frame` calls with per-frame timestamps,
//! 3. holds no internal buffering beyond the underlying `Write`.
//!
//! Timestamps use the microsecond resolution variant (magic `0xA1B2C3D4`,
//! little-endian: `D4 C3 B2 A1`) — same as the Phase 0 helper — so files
//! produced by either path open identically in Wireshark.

use std::io::{self, Write};

/// libpcap link-layer type 1 = LINKTYPE_ETHERNET.
const LINKTYPE_ETHERNET: u32 = 1;

/// Standard libpcap snap length: full frame.
const SNAPLEN_MAX: u32 = 65535;

/// Streaming pcap writer. Writes the global header on `new`; each
/// `write_frame` appends one record (16-byte record header + frame bytes).
pub struct PcapWriter<W: Write> {
    inner: W,
    frames_written: u64,
}

impl<W: Write> PcapWriter<W> {
    /// Create a new writer and emit the global header. The link type is
    /// fixed to Ethernet (the only mode this crate emits).
    pub fn new(mut inner: W) -> io::Result<Self> {
        let mut hdr = [0u8; 24];
        // Magic 0xA1B2C3D4 in little-endian = microsecond timestamps.
        hdr[0..4].copy_from_slice(&0xA1B2_C3D4_u32.to_le_bytes());
        // Version 2.4.
        hdr[4..6].copy_from_slice(&2u16.to_le_bytes());
        hdr[6..8].copy_from_slice(&4u16.to_le_bytes());
        // thiszone (i32, 0 = UTC), sigfigs (u32, 0).
        hdr[8..12].copy_from_slice(&0i32.to_le_bytes());
        hdr[12..16].copy_from_slice(&0u32.to_le_bytes());
        hdr[16..20].copy_from_slice(&SNAPLEN_MAX.to_le_bytes());
        hdr[20..24].copy_from_slice(&LINKTYPE_ETHERNET.to_le_bytes());
        inner.write_all(&hdr)?;
        Ok(Self {
            inner,
            frames_written: 0,
        })
    }

    /// Append one captured frame at the given timestamp.
    ///
    /// `timestamp_us` is the absolute capture time in microseconds since
    /// the Unix epoch. Wireshark splits it into `ts_sec` and the residual
    /// microseconds. Pass `0` for "no real time"; pass increasing values
    /// (e.g. `i as u64 * sample_interval_us`) for replay-friendly traces.
    pub fn write_frame(&mut self, timestamp_us: u64, frame: &[u8]) -> io::Result<()> {
        let ts_sec = (timestamp_us / 1_000_000) as u32;
        let ts_usec = (timestamp_us % 1_000_000) as u32;
        let len = frame.len() as u32;
        let mut rec = [0u8; 16];
        rec[0..4].copy_from_slice(&ts_sec.to_le_bytes());
        rec[4..8].copy_from_slice(&ts_usec.to_le_bytes());
        rec[8..12].copy_from_slice(&len.to_le_bytes());
        rec[12..16].copy_from_slice(&len.to_le_bytes());
        self.inner.write_all(&rec)?;
        self.inner.write_all(frame)?;
        self.frames_written += 1;
        Ok(())
    }

    /// Number of records appended so far. The global header is not counted.
    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }

    /// Flush the underlying writer.
    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }

    /// Consume the writer and return the inner `Write`.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_header_is_24_bytes_little_endian_us() {
        let buf: Vec<u8> = Vec::new();
        let w = PcapWriter::new(buf).unwrap();
        let out = w.into_inner();
        assert_eq!(out.len(), 24);
        assert_eq!(&out[..4], &[0xD4, 0xC3, 0xB2, 0xA1]); // µs-LE magic
        assert_eq!(&out[20..24], &[0x01, 0x00, 0x00, 0x00]); // Ethernet
    }

    #[test]
    fn write_frame_appends_record_header_and_bytes() {
        let mut w = PcapWriter::new(Vec::new()).unwrap();
        let payload = b"\x01\x02\x03\x04";
        w.write_frame(1_500_000, payload).unwrap();
        let out = w.into_inner();
        assert_eq!(out.len(), 24 + 16 + payload.len());
        // ts_sec = 1, ts_usec = 500_000.
        assert_eq!(&out[24..28], &1u32.to_le_bytes());
        assert_eq!(&out[28..32], &500_000u32.to_le_bytes());
        // incl_len and orig_len = 4.
        assert_eq!(&out[32..36], &4u32.to_le_bytes());
        assert_eq!(&out[36..40], &4u32.to_le_bytes());
        // payload follows record header.
        assert_eq!(&out[40..44], payload);
    }

    #[test]
    fn multiple_frames_grow_count_and_buffer() {
        let mut w = PcapWriter::new(Vec::new()).unwrap();
        for i in 0..10u64 {
            w.write_frame(i * 1000, &[i as u8; 60]).unwrap();
        }
        assert_eq!(w.frames_written(), 10);
        let out = w.into_inner();
        assert_eq!(out.len(), 24 + 10 * (16 + 60));
    }
}
