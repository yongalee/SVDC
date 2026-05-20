# ADR-0003 — ssiec-sv-publisher encoder design

- **Status:** accepted
- **Date:** 2026-05-21
- **Deciders:** SSIEC SVDC team

## Context

WBS-6.1 (Phase 0) requires `ssiec-sv-publisher` to emit at least one valid
IEC 61850-9-2 LE Sampled Value packet for round-trip verification, ahead of
the full waveform-generation work that lands in Phase 1. The encoder must:

- Be conformance-grade enough that Wireshark's built-in IEC 61850 SV
  dissector accepts the bytes without error.
- Avoid heap allocation on the publish path (NFR-4) — the same encoder will
  later be invoked at 4800–15360 packets/s.
- Stay free of external dependencies wherever practical; the SDD §6 already
  rules that "the 9-2 LE profile is tightly constrained; a full ASN.1 stack
  is unnecessary and harms determinism" (SDD §6, line 1059).
- Produce something **visible** for Phase 0 acceptance — the user must be
  able to "see" the output, not just read CI green checks.

The packet format is fixed by IEC 61850-9-2 LE (UCA Implementation
Guideline, 2004). The relevant byte sequence is:

```
Ethernet II
  dst MAC  6 B   (multicast 01:0C:CD:04:00:00..01:FF for 9-2)
  src MAC  6 B
  EthType  2 B   = 0x88BA
9-2 LE header
  APPID    2 B   = 0x4000 (typical)
  Length   2 B   (total bytes from APPID through end of savPdu)
  Reserved 2 B   = 0x0000
  Reserved 2 B   = 0x0000
savPdu  (ASN.1 BER, application tag 0x60)
  noASDU      [0] IMPLICIT INTEGER          (count of ASDUs in this PDU)
  asdu        [2] IMPLICIT SEQUENCE OF ASDU
    ASDU       (SEQUENCE 0x30)
      svID       [0] IMPLICIT VisibleString
      smpCnt     [2] IMPLICIT INTEGER (0..65535)
      confRev    [3] IMPLICIT INTEGER
      smpSynch   [5] IMPLICIT INTEGER (0=none, 1=local, 2=global)
      smpRate    [6] IMPLICIT INTEGER (optional, samples per second)
      seqData    [7] IMPLICIT OCTET STRING (64 B = 8 channels × 8 B)
```

The eight channels per 9-2 LE Phsmeas9 dataset are: Ia, Ib, Ic, In, Va, Vb,
Vc, Vn. Each channel is `i32` (instantaneous magnitude in scaled units)
followed by `u32` (IEC 61850 quality bits), big-endian.

## Decision

### 1. Hand-rolled BER encoder, in-place writes into a fixed buffer

A small `BerWriter<'a>` borrows a `&'a mut [u8]` and writes BER TLVs by
advancing a cursor. No `Vec`, no `Box`, no allocator. Length-prefix
back-patching is done by reserving 2 bytes (for the short BER long-form
length encoding `0x82 LL LL`, valid for lengths 0..=65535 which covers the
SV PDU comfortably) and writing the length back after the inner element is
emitted.

This is small (<300 lines) and obviously zero-alloc.

### 2. No external dependencies for Phase 0

The Phase 0 skeleton uses only `std`. No `clap`, no `asn1`, no `pcap-file`.
The CLI is parsed by hand (it has three modes; a match on `std::env::args`
is sufficient). PCAP file format is written directly — the global header
and per-record header are 24 + 16 bytes of well-defined layout.

We may revisit and adopt `clap` once Phase 1 brings real CLI surface area
(per-MU config, waveform parameters, replay mode); the goal here is not
"never use deps" but "do not pull deps for trivial Phase 0 scope".

### 3. Three output sinks

The user must be able to *see* the output. We ship three sinks selectable
on the command line:

- **`hex` (default)** — pretty hex+ASCII dump to stdout of the raw frame
  bytes. Immediate visual confirmation. Good for code review.
- **`pcap <path>`** — writes a libpcap-format file containing one captured
  Ethernet frame. Wireshark's IEC 61850 SV dissector reads this directly
  and renders a tree view of every field. **This is the Phase 0 "UI"** —
  the visible deliverable.
- **`udp <addr:port>`** — UDP unicast of the frame payload (without the
  L2 Ethernet header) for environments where raw socket access is awkward
  (notably Windows without administrator privileges). Phase 1 will swap
  this for raw `AF_PACKET` multicast on Linux.

### 4. Hardcoded sample values, with caveat

The Phase 0 frame carries a hardcoded sample set (a single 60 Hz cycle
snapshot at 0° phase: nominal voltage 230 V scaled to 23000, nominal
current 5 A scaled to 500). These values are realistic enough to dissect
correctly but are not output of a waveform generator. Phase 1 replaces
them with the configurable waveform synthesizer per the task description
in `crates/ssiec-sv-publisher/src/main.rs` Phase 0 banner.

Hardcoding is acceptable for Phase 0 because:

- The spec-lock answer to Q2 (80 vs 256 SPC) is still open. Locking
  waveform-generation parameters before that answer is wasted work.
- The Phase 0 acceptance criterion is *one valid packet*, not realistic
  waveforms.

### 5. Round-trip verification in unit tests

A companion decoder lives in the same crate (`pub mod decoder`). It is
not the SVDC's production decoder (that work belongs in `svdc-ingress`
Phase 1, also Claude-authored) — it exists solely so the publisher's
unit tests can round-trip encoded bytes and assert field equality. This
catches encoder bugs that the standard's prose definition alone would
miss.

### 6. svID convention

The Phase 0 svID is `SVDC_DEMO_01` (12 ASCII bytes). Phase 1 will derive
svID from the SCD per FR-1. Hardcoding here keeps the Phase 0 frame
self-contained.

## Consequences

- Wireshark opening the Phase 0 PCAP shows a fully decoded SV frame —
  this is the visible deliverable the user can demo.
- Zero allocation on the publish path is verified by inspection; the
  `BerWriter` API takes a `&mut [u8]` and never grows it.
- The encoder is reusable by Phase 1's waveform synthesizer without
  rework: only the sample-value source changes.
- Adopting `clap` and a real ASN.1 crate is deferred. Phase 1 may revisit
  if the CLI surface grows beyond what hand-parsing tolerates.

## Alternatives considered

- **`asn1`/`rasn`/`asn1-rs` crates.** Rejected for Phase 0 — adds compile
  time, transitive deps, and abstracts a fixed wire format that the SDD
  explicitly chose to hand-roll. Worth re-evaluating only if 9-2 LE
  decoding in `svdc-ingress` ends up duplicating significant code.
- **`pcap`/`pcap-file` crates for output.** Rejected for Phase 0 — the
  PCAP file format we need (one record, no live capture) is ~60 bytes
  of header constants. A 100-line module is cheaper than a dep.
- **L2 raw socket emission on Windows.** Rejected — requires Npcap and
  privileged calls; Phase 0 stays in user-space. Linux raw `AF_PACKET`
  emission is added in Phase 1 alongside the real ingest test
  infrastructure.

## References

- IEC 61850-9-2, *Specific communication service mapping (SCSM) — Sampled
  values over ISO/IEC 8802-3*, 2nd ed., 2011.
- UCA International Users Group, *Implementation Guideline for Digital
  Interface to Instrument Transformers Using IEC 61850-9-2*, 2004
  ("9-2 LE").
- ITU-T X.690, *ASN.1 encoding rules: BER, CER, DER*, 2021.
- SDD §6 (line 1059) — design choice to hand-roll the BER decoder.
- ADR-0001 — dual-agent workflow; this ADR is Claude-authored as a
  cross-cutting design decision under §5.
- Issue [#2](https://github.com/yongalee/SVDC/issues/2) — WBS-6.1 task.
