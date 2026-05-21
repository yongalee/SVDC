//! Integrity helpers.
//!
//! Phase 0 ships a hand-rolled CRC-32 (IEEE 802.3 polynomial,
//! reflected form 0xEDB88320) — the same polynomial Ethernet, gzip,
//! PNG, and ZIP all use, so external tools can re-verify the
//! historian's CRC column without an SVDC-specific implementation.
//!
//! Implementation is bit-at-a-time and zero-dependency. A 256-entry
//! lookup-table speedup is straightforward when benchmarks demand it
//! (Phase 2 target: < 1 µs per `TickRecord` of 64 channels).

/// CRC-32 (IEEE) polynomial in reflected form.
pub const CRC32_IEEE_POLY: u32 = 0xEDB8_8320;

/// Streaming CRC-32 accumulator. Construct, feed bytes via
/// [`Self::update`], retrieve the final 32-bit value via
/// [`Self::finalize`].
#[derive(Debug, Clone, Copy)]
pub struct Crc32 {
    state: u32,
}

impl Crc32 {
    /// Fresh accumulator. The initial value (`0xFFFFFFFF`) is the
    /// IEEE-802.3 standard.
    pub const fn new() -> Self {
        Self { state: 0xFFFF_FFFF }
    }

    /// Feed a slice into the accumulator.
    pub fn update(&mut self, bytes: &[u8]) {
        let mut crc = self.state;
        for &b in bytes {
            let mut byte = b;
            for _ in 0..8 {
                let lsb = ((crc as u8) ^ byte) & 1;
                crc >>= 1;
                byte >>= 1;
                if lsb != 0 {
                    crc ^= CRC32_IEEE_POLY;
                }
            }
        }
        self.state = crc;
    }

    /// Final inverted value, per IEEE-802.3 convention. Consumes the
    /// accumulator so callers don't accidentally re-feed bytes after
    /// reading the result.
    pub fn finalize(self) -> u32 {
        !self.state
    }
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

/// One-shot helper. Equivalent to `Crc32::new().update(bytes).finalize()`.
pub fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut c = Crc32::new();
    c.update(bytes);
    c.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Well-known test vectors: same values you'd get from gzip / Wireshark.
    #[test]
    fn empty_input_gives_zero() {
        assert_eq!(crc32_ieee(&[]), 0);
    }

    #[test]
    fn ascii_input_matches_reference() {
        // From the ISO 3309 / Ethernet CRC-32 standard.
        // "123456789" -> 0xCBF43926 is the canonical "check" value.
        assert_eq!(crc32_ieee(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn streaming_matches_one_shot() {
        let bytes: &[u8] = b"the quick brown fox jumps over the lazy dog";
        let one_shot = crc32_ieee(bytes);
        let mut c = Crc32::new();
        for chunk in bytes.chunks(7) {
            c.update(chunk);
        }
        assert_eq!(c.finalize(), one_shot);
    }

    #[test]
    fn single_bit_change_changes_crc() {
        let a = crc32_ieee(&[0u8; 16]);
        let mut buf = [0u8; 16];
        buf[7] = 1;
        let b = crc32_ieee(&buf);
        assert_ne!(a, b, "a one-bit change must produce a different CRC");
    }
}
