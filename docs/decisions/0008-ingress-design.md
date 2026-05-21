# ADR-0008: `svdc-ingress` design and M1→M2 boundary

- Status: Accepted
- Date: 2026-05-21
- Owner: claude-code
- Supersedes: —
- Superseded by: —
- Related: ADR-0003 (SV encoder design), IP §9.2 WBS-2.1..2.4

## Context

Phase 1 of the SVDC opens the south-bound ingress: raw L2 capture, BER
decode of IEC 61850-9-2 LE frames, ingress timestamping, and the SPSC
ring that hands decoded records to the time aligner (M2). The
Implementation Plan splits this into four WBS items:

| WBS    | Responsibility                                           |
| ------ | -------------------------------------------------------- |
| 2.1    | Raw L2 capture (`AF_PACKET` on Linux, Npcap on Windows). |
| 2.2    | IEC 61850-9-2 LE decoder.                                |
| 2.3    | Hardware / kernel ingress timestamp extraction.          |
| 2.4    | SPSC ring carrying decoded records from M1 to M2.        |

The four items have different schedules (2.3 needs PTP hardware,
Phase 5) and different owners (2.1/2.4 are hot-path work assigned to
Antigravity in the dual-agent partition). They also have a stable
interface contract that the rest of the data plane — the aligner, the
historian, the API — will bind against once the scaffold lands.

This ADR captures the design choices made in the Phase 0 scaffold so
that downstream WBS-2.x work can swap pieces in without thrashing the
public surface of the crate.

## Decision

### 1. One crate, four modules, one stable type at the boundary

`svdc-ingress` is a single crate with four submodules
(`subscriber`, `decoder`, `timestamp`, `ring`) so that the WBS items
remain a partition of one crate, not a fan-out of four crates with
shared types in `svdc-core`. The single struct that crosses the
M1→M2 boundary is [`svdc_ingress::IngressFrame`]:

```rust
pub struct IngressFrame {
    pub timestamp: IngressTimestamp,
    pub samples: Vec<DecodedSample>,
}
```

`IngressFrame` lives at the crate root. The aligner (M2) imports
this one type and remains agnostic of which subscriber, decoder, or
ring implementation produced it. ADR rationale: keeping the boundary
struct in `svdc-ingress` rather than `svdc-core` avoids leaking
ingress-internal concerns (e.g. PTP timestamp ergonomics) into the
shared core types.

### 2. Phase 0 reuses `ssiec-sv-publisher`'s BER decoder

The decoder originally landed in `ssiec-sv-publisher` as the
round-trip dual to the encoder (PR #2, WBS-6.1). Rather than
duplicate it, `svdc-ingress::decoder::Decoder` re-uses
`ssiec_sv_publisher::decode_frame` and re-shapes the result into
the public [`DecodedSample`] struct that the aligner consumes.

Phase 1 owners may:

- keep this delegation (cheapest path; what we recommend),
- migrate the decoder into `svdc-ingress` and reduce
  `ssiec-sv-publisher` to encoder-only (cleanest dependency graph;
  required if vendor frames need a richer decoder than the round-
  trip use case), or
- maintain two decoders if interop testing reveals divergent vendor
  variants (most expensive; only if forced).

The [`DecodedSample`] struct in `svdc_ingress::decoder` is the
stable interface across all three choices.

### 3. Subscriber is a pull trait; Phase 0 ships a loopback impl

`Subscriber::next_frame()` returns one `(Vec<u8>, IngressTimestamp)`
per call. The Phase 0 [`LoopbackSubscriber`] yields a fixed queue
of test frames, which is enough to exercise the decoder + ring
end-to-end without any I/O.

Phase 1 will add:

- `Linux::AfPacketSubscriber` (bound to a NIC, `SO_TIMESTAMPING`
  ancillary data captured into `IngressTimestamp`).
- `Windows::NpcapSubscriber` (Phase 5; not on the Phase 1 critical
  path).

The `next_frame` shape was chosen over a push-style callback because
it makes back-pressure trivial (the consumer just stops calling) and
because Rust's borrow checker pushes a callback-into-the-ring design
toward `unsafe` lifetimes we want to avoid until benchmarks force the
issue. Phase 1 should benchmark both before committing.

### 4. SPSC ring is a `Mutex<VecDeque>` in Phase 0

The boundary is *single producer, single consumer* — one ingress
thread feeds one aligner thread — and FR-1 demands no allocation on
the hot path. Both properties are satisfied by a fixed-capacity
lock-free SPSC queue (`crossbeam-queue::ArrayQueue`, `rtrb`).

Phase 0 ships `Mutex<VecDeque>` instead so the rest of the scaffold
can be tested without pulling in a third-party SPSC dependency yet.
The public surface of [`IngressRing`] (`push` returning the rejected
frame on full, `pop` returning `None` on empty, fixed capacity) is
identical to what the lock-free swap will expose. The Phase 1 task
to land the lock-free version is mechanical and self-contained.

### 5. Timestamps stored as `u64` Unix nanoseconds

`IngressTimestamp::from_unix_ns(u64)` + `unix_ns() -> u64`. Reasons:

- The aligner's bin index is `unix_ns / bin_ns`, an integer
  operation; floats would invite rounding bias.
- The PTP path (`SO_TIMESTAMPING`) reports `struct timespec` in
  seconds + nanoseconds, trivially converted to Unix-ns.
- `u64` ns covers the year 2554, well past any plausible SVDC
  service life.

`Ord` is derived so the ring's invariants on monotonicity are easy
to assert.

## Consequences

- The four WBS-2.x items can land independently; each replaces its
  own module without ABI churn.
- The dual-agent partition is preserved: Claude lands the scaffold +
  this ADR; Antigravity owns the lock-free ring (WBS-2.4) and the
  hot-path AF_PACKET subscriber (WBS-2.1) in Phase 1.
- The published [`DecodedSample`] type couples `svdc-ingress` to the
  channel ordering convention in `ssiec_sv_publisher::SampleData`. If
  that convention ever changes the ripple touches both crates;
  acceptable because they sit in the same workspace and ship
  together.
- Frame-drop policy (return rejected frame back to caller) gives the
  Phase 1 telemetry path a clean place to bump the drop counter
  exposed on `/api/metrics`.

## Out of scope

- Multi-NIC capture / NIC-failover (Phase 5).
- Vendor-specific decoder variants (interop test, Phase 4).
- The aligner itself (`svdc-aligner`, WBS-2.5–2.7), historian, and
  northbound layers.
