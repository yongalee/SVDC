//! ssiec-sv-publisher
//!
//! A standalone, conformance-grade IEC 61850-9-2 / 9-2 LE Sampled Value
//! publisher reference simulator. Synthesizes SV streams from configurable
//! waveforms and publishes them with strict standard compliance.
//!
//! Goals (per IP §2 WBS-6.1):
//!   * Strict 9-2 LE compliance: ASN.1 BER encoding, correct ASDU structure,
//!     proper smpCnt and smpSynch management.
//!   * Configurable sample rate (80 or 256 SPC) per 60 Hz cycle.
//!   * Configurable channels, amplitudes, frequencies, phases, harmonics.
//!   * Ships with IEC 61850-10 conformance test vectors (known input →
//!     known SV byte sequence) for third-party self-verification.
//!   * Suitable for verifying SVDC against the standard, and for verifying
//!     commercial MUs (SEL, GE Multilin, ABB SAM600, Toshiba CRH, etc.)
//!     against each other via the SVDC.
//!
//! Status: Phase 0 skeleton. Real implementation in Phase 1.

fn main() {
    println!("ssiec-sv-publisher: Phase 0 skeleton.");
    println!("Phase 1 will implement single-packet emission for round-trip verification.");
}
