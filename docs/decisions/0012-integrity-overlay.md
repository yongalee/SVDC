# ADR-0012: dual-CB integrity overlay — CRC over `TickRecord` payload

- Status: Accepted
- Date: 2026-05-21
- Owner: claude-code
- Supersedes: —
- Superseded by: —
- Related: ADR-0009 (`svdc-aligner` + `TickBuffer`), SDD §7.1 (TickRecord),
  IP §9.2 WBS-2.9

## Context

ADR-0009 §6 noted that the "dual" in dual circular buffer covers two
concerns — **integrity** (catch corruption in the buffer before a
consumer reads bad data) and **failover** (a hot spare takes over
without consumer-visible discontinuity) — and that both land in
Phase 2 behind the `TickBuffer` surface.

The integrity half is cheap to land now: SDD §7.1 already specifies
a `crc: u32` field on `TickRecord` (PR #46 implemented the type but
left the field at zero). Populating that field at the moment the
aligner emits a record and exposing a `verify_all()` diagnostic
gives the rest of the data plane a way to detect corruption before
the failover path is wired in.

This ADR documents the integrity overlay design. Failover stays
deferred to Phase 2.

## Decision

### 1. CRC-32 IEEE 802.3 polynomial (`0xEDB88320`, reflected)

The same polynomial Ethernet, gzip, PNG, and ZIP use. Reasons:

- **No new dependency** — a 30-line bit-at-a-time loop in
  `svdc-core` is enough; a 256-entry lookup-table speedup is
  straightforward when benchmarks demand it.
- **External tools can re-verify** — pandas + zlib's `crc32`, the
  `crc32` CLI in coreutils, or a Wireshark filter all produce the
  same 32-bit value. The historian CSV's `crc` column is portable.
- **Established collision behaviour** — 32 bits is the right size
  for "catch corruption within a ~few-MB buffer". For
  cryptographic-strength integrity (Phase 5 if/when the
  protection-bus threat model demands it) we'd reach for SHA-256
  instead; that's a different problem.

The polynomial constant + streaming accumulator live in
`svdc_core::integrity` so any future producer (historian replay,
QSE write-back) can use the same helper without depending on the
aligner crate.

### 2. CRC covers the *populated* payload, not the metadata header

The CRC is computed over `samples[..n_channels]` only — the
metadata header (`tick_id`, `ts_utc_ns`, `n_channels`, `flags`)
is **not** covered. Reasons:

- **The job of the CRC is to catch corruption inside the sample
  payload**, the field that the dual-CB swaps under load and that
  a write-back path mutates.
- The metadata header is set once when the aligner emits the
  record and never rewritten. If the header itself were corrupted,
  the failure mode is a tick_id discontinuity, which the dual-CB's
  monotonicity check catches independently.
- Folding the header into the CRC would invalidate the CRC every
  time the dual-CB stamps a different `tick_id` on a copy, which
  is exactly what the failover path needs to do.

The hash order is `value_q` little-endian (4 B), `quality` (1 B),
`origin` (1 B), `reserved` little-endian (2 B) — the same byte
order an external tool would get by reading the SDD §7.1 layout.

### 3. Unused slots do not contribute

`compute_crc` walks `live_samples()` (`samples[..n_channels]`), so
two records with the same populated prefix but different junk in
the unused slots produce the same CRC. This matters because:

- It lets the dual-CB zero-fill or leave-garbage unused slots
  without invalidating the CRC.
- It keeps the integrity overlay tight: only the bytes a consumer
  reads are bytes the CRC covers.

A test pins this behaviour so a future refactor of the live-slot
walker doesn't quietly start covering all 64 slots.

### 4. Aligner stamps; `TickBuffer::verify_all` diagnoses

The aligner's `process_frame` calls `tick.stamp_crc()` just before
returning. Every record that enters the data plane carries a valid
CRC; consumers can trust it without re-computing on the hot path.

`TickBuffer::verify_all()` walks the whole buffer and returns one
`IntegrityViolation` per failure. It is O(n_channels × buffer.len())
— a slow-path operation suitable for:

- an operator-driven `/health` probe,
- a periodic integrity sweep (every few seconds),
- the dual-CB failover trigger when Phase 2 wires it.

It is **not** suitable for per-read verification; per-read is what
the failover path is for in Phase 2 (probability of corruption is
low; a sweep + failover beats a hot-path check).

### 5. Failover stays Phase 2

The hot-spare buffer mechanism — primary fails `verify_all`, swap
to spare, mark surrounding window `DEGRADED`, reseed primary from
the aligner — needs:

- two `TickBuffer`s running in lockstep (today there is one),
- a swap mechanism observable to subscribers without dropping their
  cursors,
- a reseed policy when the failed primary comes back.

All three are bounded but non-trivial. The Phase 2 owner picks
them up; this PR ships the detection half so the failover half
has something to trigger on.

## Consequences

- Every `TickRecord` flowing through the data plane now carries a
  valid CRC. The historian's `flags_hex` column (PR #48) gains a
  natural complement: a future CSV revision can add `crc` as the
  next column and external tools can re-verify with `python -m
  zlib`.
- `TickBuffer::verify_all()` gives the future `/health` HTTP
  endpoint (SDD §8.4) a one-line probe.
- The `IntegrityViolation` struct surfaces the discrepancy
  (`{tick_id, stored, computed}`) so operators can correlate
  failures to specific tick IDs in the historian.
- Hot-path cost added: one `compute_crc()` call per
  `process_frame`. For 8 channels × 8 bytes = 64 bytes that is ~4 µs
  on a stock laptop — well under the 4800-Hz tick budget.
  Benchmarks land in WBS-7.4 (Phase 5).

## Out of scope

- Failover / hot-spare buffer (Phase 2).
- Periodic integrity sweep scheduler (lives in `svdc-bin`'s daemon
  loop when daemon wiring lands).
- Cryptographic integrity for QSE write-back authentication
  (Phase 5; CRC catches corruption, not adversarial overwrites).
- Bench-driven table-based CRC32 (Phase 5).
