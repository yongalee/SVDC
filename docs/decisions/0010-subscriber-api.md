# ADR-0010: `svdc-subscribe` — northbound subscriber API (M3→M4)

- Status: Accepted
- Date: 2026-05-21
- Owner: claude-code
- Supersedes: —
- Superseded by: —
- Related: SDD §8.2 (Subscriber API), ADR-0009 (`svdc-aligner` + `TickBuffer`),
  ADR-0008 (`svdc-ingress`), [SDD §7.1](#) (TickRecord)

## Context

SDD §8.2 specifies a northbound subscriber API with two transport
bindings — in-process C ABI (zero-copy, for EBP relays and the
Phasor Computation Module) and out-of-process UNIX-domain sockets
(length-prefixed framing, for consumers in other processes or
languages) — and identical semantics across both. The wire-level C
ABI sketched in the SDD is:

```c
SvdcCursor svdc_subscribe(const ChannelSet* cs);
int        svdc_read_latest(SvdcCursor, size_t k, TickRecord** out);
int        svdc_read_since (SvdcCursor, TickRecord** out, size_t* n_out);
void       svdc_release    (SvdcCursor, TickRecord**);
void       svdc_unsubscribe(SvdcCursor);
```

Phase 0 of the SVDC needs the Rust-native shape of this API locked
in before either transport binding is built: the data plane already
emits `TickRecord`s into a `TickBuffer` (ADR-0009), and every M4
consumer that lands later (CSV historian, OPC UA L1, MQTT L2,
TimescaleDB L3, plus the local relays) will bind against this API.

This ADR captures the design choices made in the Phase 0 scaffold so
the Phase 4 C ABI and UDS wrappers inherit the same cursor semantics
and don't have to re-derive them.

## Decision

### 1. One crate, three concepts: `Subscriber`, `Subscription`, `ChannelSet`

`svdc-subscribe` is a small crate that contains:

- **`ChannelSet`** — `All` or `Specific(Vec<u16>)`. Maps onto the C
  `const ChannelSet*` argument; the `u16` matches `channel_id` in
  SDD §7.2's channel registry.
- **`Subscription`** — the Rust analogue of `SvdcCursor`. Holds the
  per-subscription cursor (last-read `tick_id`), the requested
  `ChannelSet`, and a shared handle to the `TickBuffer`. Dropping
  this value is `svdc_unsubscribe`.
- **`Subscriber` trait** — factory that produces `Subscription`s.
  Pluggable so consumers can be unit-tested against a mock buffer;
  production wires [`InProcessSubscriber`].
- **`InProcessSubscriber`** — the Phase 0 implementation. Wraps
  `Arc<TickBuffer>` directly. Phase 4's C ABI and UDS bindings will
  *call into* this same type rather than re-implement the cursor
  logic.

### 2. `read_latest` vs `read_since` keep different cursor semantics

- `read_latest(k)` — snapshot of the newest `k` records, newest
  first. Does **not** advance the cursor. Matches the SDD's
  `svdc_read_latest` call shape; the use case is "get the most
  recent state without disturbing my stream pointer."
- `read_since()` — every record with `tick_id > cursor`, returned
  oldest-first, advances the cursor to the newest delivered tick.
  Matches `svdc_read_since`. The use case is gap-free streaming
  reads into a sink (historian, network).

The split is the same shape the SDD uses and it kept the test
matrix clean (snapshot tests vs. streaming tests).

### 3. Phase 0 returns `Vec<TickRecord>`, not `&[TickRecord]`

The SDD specifies `TickRecord**` (pointer to caller-owned array) for
the C ABI to enable **zero-copy** reads. The Rust equivalent is
`&[TickRecord]` borrowed from inside the buffer's lock — but
`TickBuffer` is currently a `Mutex<VecDeque>` (ADR-0009 §6) and
returning a slice from inside a mutex guard requires lifetime
acrobatics that don't survive the lock-free swap planned for
Phase 2.

Phase 0 returns owned `Vec<TickRecord>` (cloned out of the buffer).
Tradeoff: extra allocation + clone on every read; acceptable while
the data plane is functionally proving out. Zero-copy lands together
with the lock-free SPSC `TickBuffer` in Phase 4; the returned shape
becomes `&[TickRecord]` then.

### 4. `ChannelSet` filter is advisory in Phase 0

Filtering by `ChannelSet::Specific(...)` requires per-channel
masking, which in turn needs the SCD-derived channel registry
(SDD §7.2) wired through the aligner so each sample carries its
`channel_id`. That mapping lands in Phase 2.

Phase 0 stores the `ChannelSet` on the `Subscription` for inspection
and returns the full `TickRecord` regardless. `ChannelSet::contains`
is correct now so Phase 1 can flip on the filter by changing only
the read paths, not the API.

### 5. The cursor is per-subscription and starts at zero

`Subscription::cursor` is private. New subscriptions start at zero
("nothing read yet") and `read_since` advances it to the newest
delivered `tick_id`. Because `TickBuffer` retires the oldest record
on overflow (ADR-0009 §4), a subscription whose cursor falls behind
the buffer's window will silently miss records — that's the price
of the "drop oldest" policy. The C ABI in Phase 4 will expose
`subscription.cursor()` (a getter exists already) so external tools
can detect and alarm on cursor lag.

## Consequences

- Phase 4 work (C ABI in `svdc-cabi`, UDS binding in `svdc-uds`)
  imports this crate; they do not re-implement subscription
  bookkeeping.
- The cursor model is testable today via [`InProcessSubscriber`],
  which is the single source of truth for behaviour. New transport
  wrappers add coverage in proportion to how much wire-format
  work they do.
- A future "channel mask" optimisation can mask out unwanted channel
  samples by setting their `origin = Invalid` in the returned record
  — no API change required.
- The QSE write-back path (SDD §8.3, FR-6) gets its own crate when
  it lands; it intentionally does **not** share the subscriber
  surface, because write-back is rare and slow while subscriptions
  are hot.

## Out of scope

- C ABI (`svdc-cabi`, Phase 4).
- UDS transport (Phase 4).
- Per-channel masking (Phase 2, gated on channel registry).
- QSE write-back (Phase 4, separate crate).
- Subscription back-pressure / flow control (Phase 5; today the
  buffer's drop-oldest policy plus the cursor getter is enough).
