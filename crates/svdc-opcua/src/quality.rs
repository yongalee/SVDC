//! IEC 61850 quality byte → OPC UA `StatusCode` per ADR-0017 §3.
//!
//! Implements the subset of OPC 10040 §6.3's mapping table that the
//! SVDC actually consumes today. Bits we do not process leave the
//! returned StatusCode at `Good` unless another consumed bit raises
//! it. The bidirectional reverse (`StatusCode → q`) is not provided
//! — the SVDC is the source of truth for sample quality; downstream
//! OPC UA observers do not write back.
//!
//! All values in this module are plain integers so the table is
//! testable as data and survives a swap of the `opcua` crate's
//! `StatusCode` type (which uses a newtype wrapper around `u32`).

/// IEC 61850-7-3 quality flag bit positions (subset). The byte
/// layout matches the low octet of the standard's `Quality`
/// bitstring (15 bits widened to 16 but with the high byte
/// reserved). Values are `u8` so they match `Sample.quality` in
/// `svdc-core`.
pub mod q_bits {
    /// All-zero quality byte: "Good" with no overrides.
    pub const GOOD: u8 = 0b0000_0000;
    /// Validity field, value `Invalid` (bits 0..1 = 0b10).
    pub const VALIDITY_INVALID: u8 = 0b0000_0010;
    /// Validity field, value `Questionable` (bits 0..1 = 0b11).
    pub const VALIDITY_QUESTIONABLE: u8 = 0b0000_0011;
    /// Detail bit: overflow during the measurement window.
    pub const OVERFLOW: u8 = 0b0000_0100;
    /// Detail bit: communication failure between MU and SVDC.
    pub const FAILURE: u8 = 0b1000_0000;
    /// Mask for the validity sub-field (bits 0..1).
    pub const VALIDITY_MASK: u8 = 0b0000_0011;
}

/// OPC UA `StatusCode` numeric values per OPC 10000-4 §7.34 / OPC
/// 10040 §6.3. Listed as raw `u32` so the table renders as data;
/// PR L will wrap each value in `opcua::types::StatusCode::from`.
pub mod status_codes {
    /// `Good` — no issue.
    pub const GOOD: u32 = 0x0000_0000;
    /// `Uncertain_LastUsableValue` — last good value held while
    /// quality is degraded.
    pub const UNCERTAIN_LAST_USABLE_VALUE: u32 = 0x4090_0000;
    /// `Uncertain_InterpolatedValue` — value was synthesised by
    /// the aligner's interpolator (SDD §2 FR-4).
    pub const UNCERTAIN_INTERPOLATED_VALUE: u32 = 0x408F_0000;
    /// `Bad_NoData` — validity bit says `Invalid`; no usable
    /// sample available.
    pub const BAD_NO_DATA: u32 = 0x80AB_0000;
    /// `Bad_OutOfRange` — overflow detail bit set; sample is
    /// outside the configured measurement range.
    pub const BAD_OUT_OF_RANGE: u32 = 0x803B_0000;
    /// `Bad_NoCommunication` — communication failure detail bit
    /// set; the MU stopped delivering samples.
    pub const BAD_NO_COMMUNICATION: u32 = 0x80B1_0000;
}

/// Map an IEC 61850 quality byte to an OPC UA `StatusCode`.
///
/// Precedence (highest first):
///
/// 1. `FAILURE` bit → `Bad_NoCommunication`
/// 2. validity == `Invalid` → `Bad_NoData`
/// 3. `OVERFLOW` bit → `Bad_OutOfRange`
/// 4. validity == `Questionable` → `Uncertain_LastUsableValue`
/// 5. else → `Good`
///
/// Multi-cause failures collapse to the worst code: a sample with
/// both `FAILURE` and `OVERFLOW` set is `Bad_NoCommunication`, not
/// `Bad_OutOfRange`. SCADA alarm logic only needs the single
/// dominant status; the diagnostic detail belongs in a separate
/// `q` mirror node (per ADR-0017 §2).
pub fn iec61850_to_opcua_status(q: u8) -> u32 {
    if q & q_bits::FAILURE != 0 {
        return status_codes::BAD_NO_COMMUNICATION;
    }
    let validity = q & q_bits::VALIDITY_MASK;
    if validity == q_bits::VALIDITY_INVALID {
        return status_codes::BAD_NO_DATA;
    }
    if q & q_bits::OVERFLOW != 0 {
        return status_codes::BAD_OUT_OF_RANGE;
    }
    if validity == q_bits::VALIDITY_QUESTIONABLE {
        return status_codes::UNCERTAIN_LAST_USABLE_VALUE;
    }
    status_codes::GOOD
}

/// Apply the [`svdc_core::SampleOrigin`]-style override to a base
/// status per ADR-0017 §3. Origin codes match the raw `u8` values
/// of `SampleOrigin`:
///
/// - `1` (Live) → no change
/// - `2` (Interpolated) → upgrade `Good` to
///   `Uncertain_InterpolatedValue`; leave any `Bad_…` code untouched
/// - `3` (QseEstimated) → upgrade `Good` to
///   `Uncertain_LastUsableValue`; leave any `Bad_…` code untouched
///
/// The "leave Bad untouched" rule matches OPC 10040 §6.4: severity
/// must not decrease when applying a substatus.
pub fn apply_origin_override(base: u32, origin: u8) -> u32 {
    // The OPC UA severity field is the top two bits of the
    // StatusCode (`0x4…` = Uncertain, `0x8…` = Bad). Anything at
    // Bad-severity already outranks any Uncertain override we
    // would apply.
    let is_bad = (base & 0x8000_0000) != 0;
    if is_bad {
        return base;
    }
    match origin {
        2 => status_codes::UNCERTAIN_INTERPOLATED_VALUE,
        3 => status_codes::UNCERTAIN_LAST_USABLE_VALUE,
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_byte_maps_to_good() {
        assert_eq!(iec61850_to_opcua_status(q_bits::GOOD), status_codes::GOOD);
    }

    #[test]
    fn invalid_validity_maps_to_bad_no_data() {
        assert_eq!(
            iec61850_to_opcua_status(q_bits::VALIDITY_INVALID),
            status_codes::BAD_NO_DATA
        );
    }

    #[test]
    fn questionable_validity_maps_to_uncertain_last_usable() {
        assert_eq!(
            iec61850_to_opcua_status(q_bits::VALIDITY_QUESTIONABLE),
            status_codes::UNCERTAIN_LAST_USABLE_VALUE
        );
    }

    #[test]
    fn overflow_alone_maps_to_bad_out_of_range() {
        assert_eq!(
            iec61850_to_opcua_status(q_bits::OVERFLOW),
            status_codes::BAD_OUT_OF_RANGE
        );
    }

    #[test]
    fn failure_alone_maps_to_bad_no_communication() {
        assert_eq!(
            iec61850_to_opcua_status(q_bits::FAILURE),
            status_codes::BAD_NO_COMMUNICATION
        );
    }

    #[test]
    fn failure_takes_precedence_over_overflow() {
        let combined = q_bits::FAILURE | q_bits::OVERFLOW;
        assert_eq!(
            iec61850_to_opcua_status(combined),
            status_codes::BAD_NO_COMMUNICATION
        );
    }

    #[test]
    fn invalid_validity_takes_precedence_over_overflow() {
        let combined = q_bits::VALIDITY_INVALID | q_bits::OVERFLOW;
        assert_eq!(
            iec61850_to_opcua_status(combined),
            status_codes::BAD_NO_DATA
        );
    }

    #[test]
    fn overflow_overrides_questionable_validity() {
        let combined = q_bits::VALIDITY_QUESTIONABLE | q_bits::OVERFLOW;
        assert_eq!(
            iec61850_to_opcua_status(combined),
            status_codes::BAD_OUT_OF_RANGE
        );
    }

    #[test]
    fn origin_live_is_passthrough() {
        // SampleOrigin::Live = 1 per svdc-core.
        assert_eq!(
            apply_origin_override(status_codes::GOOD, 1),
            status_codes::GOOD
        );
    }

    #[test]
    fn origin_interpolated_upgrades_good() {
        // SampleOrigin::Interpolated = 2.
        assert_eq!(
            apply_origin_override(status_codes::GOOD, 2),
            status_codes::UNCERTAIN_INTERPOLATED_VALUE
        );
    }

    #[test]
    fn origin_qse_estimated_upgrades_good() {
        // SampleOrigin::QseEstimated = 3.
        assert_eq!(
            apply_origin_override(status_codes::GOOD, 3),
            status_codes::UNCERTAIN_LAST_USABLE_VALUE
        );
    }

    #[test]
    fn origin_override_does_not_downgrade_bad() {
        for bad in [
            status_codes::BAD_NO_DATA,
            status_codes::BAD_OUT_OF_RANGE,
            status_codes::BAD_NO_COMMUNICATION,
        ] {
            assert_eq!(apply_origin_override(bad, 2), bad);
            assert_eq!(apply_origin_override(bad, 3), bad);
        }
    }

    #[test]
    fn unknown_origin_is_passthrough() {
        // Future SampleOrigin variants (or junk on the wire) must
        // not silently mutate the status.
        assert_eq!(
            apply_origin_override(status_codes::GOOD, 99),
            status_codes::GOOD
        );
    }
}
