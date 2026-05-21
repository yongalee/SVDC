# ADR-0011: `svdc-historian` — append-only CSV historian (WBS-3.9)

- Status: Accepted
- Date: 2026-05-21
- Owner: claude-code
- Supersedes: —
- Superseded by: —
- Related: ADR-0010 (`svdc-subscribe`), SDD §7.1 (TickRecord), IP §9.2 WBS-3.9

## Context

WBS-3.9 in the IP calls for a TimescaleDB sidecar as the long-term
historian for SVDC tick records. That deliverable is gated on
networking infrastructure (a database server, schema management, a
DSN secret store) we don't have in Phase 0. What we *do* have, as of
PR #47, is a stable in-process subscriber API
(`svdc_subscribe::Subscription`) that hands `TickRecord`s to any
node-local consumer. The cheapest possible northbound consumer that
exercises the full data plane is one that writes those records to a
CSV file.

Two reasons it's worth landing now rather than waiting for Phase 4:

1. It is the first **vertical slice** of the data plane that
   produces operator-visible output: a file on disk that a
   spreadsheet can plot. The Phase 0 demo cycle benefits.
2. The shape of "Subscription → format-specific writer" is the
   pattern every Phase 4 northbound sink will share (Parquet,
   TimescaleDB, MQTT, OPC UA). Locking it down in CSV now de-risks
   the Phase 4 work.

## Decision

### 1. CSV in Phase 0; Parquet and TimescaleDB later

`Format::Csv` is the only variant Phase 0 supports. The `Format`
enum exists so the Phase 4 owners can add `Parquet` and
`TimescaleDB` variants without changing the [`Historian`] surface.
`HistorianConfig` carries the variant so callers pick the writer
when they construct the historian — `Historian::new` dispatches.

CSV is chosen specifically because:

- **No external dependencies** — `std::io::BufWriter` + `write!`.
- **Plot-friendly** — pandas, Excel, gnuplot all consume CSV
  natively.
- **Append-safe** — POSIX append is atomic per write call; no need
  to coordinate with sidecar writers.
- **Recovery-friendly** — corruption is line-local; a partial
  final line during a crash loses one tick, not the whole file.

The cost is on-disk size (≈ 200 bytes per row at 8 channels). For
the Phase 0 demo cycle this is irrelevant; Phase 4 rotation +
Parquet eliminates it.

### 2. Header on first write; never re-written

`Historian::new` checks `config.path.exists()` *before* opening the
file in append mode. If the file did not exist, it writes the CSV
header row once. Re-opening an existing file from a later daemon
run appends rows without re-writing the header. This keeps the
file `pandas.read_csv()`-clean across restarts.

Tradeoff: a corrupted header (e.g. truncated mid-write) is not
self-healing. Phase 4's Parquet writer side-steps the issue by
embedding the schema in the file footer.

### 3. One row per tick, every channel slot in the header

Schema:

```
tick_id, ts_utc_ns, n_channels, flags_hex,
ch0_value, ch0_quality, ch0_origin,
ch1_value, ch1_quality, ch1_origin,
...,
ch63_value, ch63_quality, ch63_origin
```

The header references all `MAX_CHANNELS` (= 64) slots — even the
ones the aligner doesn't populate in Phase 0 — so analytics tools
see a stable column set across deployments with different
populated-channel counts. `flags_hex` is `0x%04X` formatted so
operators can grep for specific flags (e.g. `0x0002` =
INTERPOLATED).

This is wasteful for the single-MU Phase 0 demo (most rows have
56 channels of zeros). The cost is borne by the disk, not the data
plane. Phase 4 column compression (Parquet) or normalised storage
(TimescaleDB) makes it a non-issue.

### 4. `tick()` model: caller controls cadence

`Historian::tick()` drains the subscription via `read_since()` and
writes every fresh record. The caller decides cadence (every tick
from the daemon loop, every 50 ms from a separate thread, etc.).
Reasons:

- The aligner already runs on its own thread; the historian
  shouldn't impose a separate thread of its own.
- The cadence is a deployment knob the daemon owns, not the
  historian.

`Historian::flush()` exists explicitly because `BufWriter` does
not flush on every write; the daemon decides when durability is
worth the syscall.

### 5. No rotation in Phase 0

`RotationPolicy::None` is the only variant. Rotation interacts with
retention policy (how long do we keep yesterday's CSV?), with
operator-visible file naming, and with downstream consumers
(historian + TimescaleDB sidecar must agree on cutover). All three
need product input; Phase 0 keeps a single file and grows it.

The placeholder enum sits in the surface so Phase 4 can add
`BySize(u64)` / `Daily` without breaking callers.

## Consequences

- The Phase 0 demo loop can now produce a CSV that opens in any
  spreadsheet — the SVDC has its first operator-facing output.
- The `Subscription` → writer pattern is now exercised end-to-end.
  Phase 4's OPC UA / MQTT / TimescaleDB sinks all follow the same
  shape and can copy the historian's structure.
- The `flags_hex` column lets ops/test engineers grep raw CSV for
  INTERPOLATED, QSE_CORRECTED, DEGRADED ticks without a parser.
- `tick()` returning the count enables a `/metrics` endpoint
  (Phase 3) to publish "historian rows/sec" without extra
  bookkeeping in the historian itself.
- Disk size on a single-MU 4800-Hz stream: ≈ 200 B/row × 4800/s =
  ≈ 1 MB/s = 86 GB/day. Acceptable for short demos; rotation +
  compression are explicit Phase 4 follow-ups.

## Out of scope

- Rotation (Phase 4: `BySize`, `Daily`).
- Parquet sidecar (Phase 4).
- TimescaleDB sidecar (Phase 4, WBS-3.9 in the IP).
- Daemon wiring (`svdc-bin --historian-out`): the historian is a
  library here; the daemon wires it next.
- Channel filtering: `Historian` writes every populated slot
  regardless of the subscription's `ChannelSet` (advisory in Phase 0
  per ADR-0010 §4).
