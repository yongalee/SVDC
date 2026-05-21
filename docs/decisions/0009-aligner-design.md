# ADR-0009: `svdc-aligner` design and M2→M3 boundary

- Status: Accepted
- Date: 2026-05-21
- Owner: claude-code
- Supersedes: —
- Superseded by: —
- Related: ADR-0008 (`svdc-ingress`), SDD §7.1 (TickRecord), IP §9.2 WBS-2.5..2.9

## Context

Phase 2 of the SVDC turns the stream of decoded SV frames coming off
the M1→M2 ring into a stream of PTP-aligned tick records that the
northbound layers (`svdc-api`, `svdc-opcua`, the historian) consume.
The Implementation Plan splits this into five WBS items:

| WBS    | Responsibility                                         |
| ------ | ------------------------------------------------------ |
| 2.5    | Time aligner (bin frames onto the PTP tick grid).      |
| 2.6    | Interpolator (fill gaps from publisher drops).         |
| 2.7    | Calibration application.                               |
| 2.8    | Dual circular buffer (the M2→M3 staging area).         |
| 2.9    | CB integrity + failover.                               |

This ADR captures the design choices made in the Phase 0 scaffold so
the Phase 2 work can land each module without thrashing the public
surface of the crate. It is the sequel to ADR-0008 (`svdc-ingress`)
and follows the same playbook.

## Decision

### 1. One crate, four modules, one stable type at the boundary

`svdc-aligner` is a single crate with four submodules
(`binner`, `interpolator`, `calibrator`, `buffer`). The single struct
that crosses the M2→M3 boundary is `svdc_core::TickRecord` — the
aligner consumes `svdc_ingress::IngressFrame` and emits `TickRecord`s
into [`TickBuffer`]. Both consumer types live outside the aligner so
northbound code can import them without dragging the aligner into its
dependency graph.

ADR rationale: keeping `TickRecord` in `svdc-core` rather than
`svdc-aligner` is the same pattern ADR-0008 §1 documented — boundary
structs live in the layer that defines the contract, not the layer
that fills it. The aligner is one of (eventually) several producers
into the tick buffer; the historian replay path will be another.

### 2. Identity pipeline in Phase 0; real behaviour lands per-module

Each of the four submodules is an identity in the Phase 0 scaffold:

- [`Binner`] computes the tick index `ts_utc_ns / period_ns`.
- [`Interpolator`] returns the input frame untouched.
- [`Calibrator`] returns the input frame untouched. The
  [`Calibration`] data struct is finalised so Phase 2 only has to
  fill in the map and apply it.
- [`TickBuffer`] is a `Mutex<VecDeque<TickRecord>>`.

The assembled [`Aligner::process_frame`] emits one `TickRecord` per
input frame. The Phase 0 integration test
(`tests/end_to_end_pipeline.rs`) drives 8 publisher frames through
the full M1→M2 path and asserts monotonic tick IDs + bin-period
spacing — sufficient evidence that the surface holds together.

### 3. `process_frame -> Vec<TickRecord>` (not `Option`)

The Phase 2 aligner can emit zero, one, or several ticks per input
frame:

- Zero when a window is still open and waiting for more samples.
- One in the common steady-state case.
- Several when a frame arrives after a publisher pause spanning
  multiple bins (the aligner closes the intervening empty windows).

Returning `Vec<TickRecord>` is the future-friendly shape; callers
already loop over the result in the Phase 0 scaffold so the Phase 2
change is invisible to them.

### 4. `TickBuffer` drops the oldest on overflow

Under sustained back-pressure the buffer must choose between:

- **Drop oldest** — consumers can fall behind safely; their fresh
  reads still get the *newest* tick. Stale ticks are the cost.
- **Drop newest (reject push)** — would stall the data plane in a
  way that's invisible to operators and would block the aligner's
  thread.

We choose drop-oldest and surface it via the
[`PushOutcome::DroppedOldest`] return so the Phase 1 telemetry path
has a clean place to count drops. This mirrors the `IngressRing` →
return-the-rejected-frame contract in ADR-0008 §4: both rings
expose drop-pressure to telemetry rather than hide it.

### 5. Calibration triple duplicated, not shared

The aligner's `Calibration` struct and `svdc_console::operational::Calibration`
have identical field shapes but the aligner does **not** depend on
`svdc-console` (which would pull in `axum`, `maud`, etc. — heavy
deps the data plane has no business carrying). Phase 2 wires the
console's operational map into the aligner explicitly at the
daemon startup boundary; the two `Calibration` structs convert
trivially. ADR-0007's read-only/operator-tunable split is preserved.

### 6. Dual circular buffer (WBS-2.8 + 2.9) is one struct in Phase 0

Phase 0 ships a single `Mutex<VecDeque>`. The "dual" in dual circular
buffer covers two concerns:

- **Integrity** (WBS-2.9): periodic hash checkpoints over the buffer
  so a corrupt producer is caught at the boundary.
- **Failover**: a hot-spare buffer takes over without consumer-
  visible discontinuity if the primary is being checked or swapped.

Both will sit *behind* the [`TickBuffer`] public surface as an
internal upgrade in Phase 2. Consumers do not have to change.

## Consequences

- The five WBS-2.x items can land independently; each replaces its
  own module behind the surface. Antigravity owns the hot-path work
  (real binner with grace-window logic, lock-free buffer); Claude
  owns design surface + integrity overlay.
- TickRecord stays a placeholder in `svdc-core` until Phase 2 work
  expands it per SDD §7.1. Northbound layers that import `TickRecord`
  today consume only `tick_id` and `ts_utc_ns`; they will get a
  full `Sample` array later, gated by a Phase 2 PR that lands the
  fleshed-out shape.
- The aligner has no async runtime and no I/O. It is a pure stream
  transformer; the daemon is responsible for plumbing.
- The Phase 0 integration test (`end_to_end_pipeline.rs`) is the
  first time the publisher → ingress → aligner path runs end-to-end
  in CI. It functions as a regression guard for boundary type
  changes across three crates.

## Out of scope

- TickRecord field expansion per SDD §7.1 (separate Phase 2 PR).
- Northbound layer wiring (`svdc-api`, `svdc-opcua`, MQTT, historian).
- Channel-registry-aware aligner (the aligner today operates on a
  single MU; multi-MU bookkeeping per the SCD lands in Phase 2).
