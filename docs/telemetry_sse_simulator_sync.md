# SVDC Telemetry & Simulator Sync Document

**Target Audience:** Agents working on the SVDC Southbound Simulator
(`ssiec-sv-publisher`), the Core Data Plane, or the Operator Console
frontend.

This document is the **source of truth** for the telemetry SSE wire
format. If the backend changes the schema, this file changes in the
same PR; the frontend follows.

## Overview

The Diagnostic Telemetry UI (`monitoring.rs`) in the `svdc-console`
renders **dynamic, real-time charts and logs** instead of static
SVGs. It relies on the Server-Sent Events broadcast stream.

### Endpoints

| URL                  | Status     | Notes                                                                |
| -------------------- | ---------- | -------------------------------------------------------------------- |
| `GET /api/events`    | Canonical  | What the frontend targets.                                            |
| `GET /sse/dashboard` | Alias      | Same handler as `/api/events`; kept for back-compat with existing htmx attributes. |

Both URLs map to one broadcaster; client behaviour is identical.

### Transport mechanics

- `Content-Type: text/event-stream`
- Each event is a single `data: <json>\n\n` line
- Keepalive comment every 15 s (`text/keepalive`)
- Reconnect: browser-default (`EventSource` auto-reconnects)
- Cadence: emitter loops at 10 Hz; `Waveform` ticks every 100 ms;
  `Metrics` every 1 s; `Qse` is event-driven (mocked at low rate
  in Phase 0)

### Payload envelope

```json
{ "event_type": "<Metrics|Waveform|Qse>", "data": { ... } }
```

The discriminator is `event_type`. The `data` field carries the
variant payload. Add new variants by appending; never remove.

## SSE Payload Contract

The UI consumes events formatted as JSON payloads defined in
[`crates/svdc-console/src/sse/mod.rs`](../crates/svdc-console/src/sse/mod.rs).
The `SsePayload` enum dictates the structure:

### 1. `Metrics` event

Updates the PTP-disciplined clock offset chart and the Circular
Buffer saturation area chart on the Dashboard.

Required fields: `ptp_offset_ns` (i64), `buffer_saturation` (f64).
Other fields are optional on the frontend side (`#[serde(default)]`
on the backend struct).

```json
{
  "event_type": "Metrics",
  "data": {
    "ptp_sync_status": "Locked",
    "ptp_offset_ns": 14,
    "buffer_saturation": 100.0,
    "active_mus": 1,
    "sps_rate": 4800,
    "l1_opcua_active": true,
    "l2_mqtt_active": false,
    "l3_timescaledb_active": true,
    "integrity_violations": 0,
    "live_feed_active": true
  }
}
```

| Field                  | Type   | Source                                              |
| ---------------------- | ------ | --------------------------------------------------- |
| `ptp_sync_status`      | string | (mock until Phase 5)                                |
| `ptp_offset_ns`        | i64    | (mock until Phase 5)                                |
| `buffer_saturation`    | f64    | `tick_buffer.len() / capacity * 100`                |
| `active_mus`           | usize  | Phase 0 proxy = 1 if buffer non-empty (PR D wires real auto-reg) |
| `sps_rate`             | u32    | Ticks emitted in the last second                    |
| `l1_opcua_active`      | bool   | (mock until Phase 4 server lands)                   |
| `l2_mqtt_active`       | bool   | (mock until Phase 4)                                |
| `l3_timescaledb_active`| bool   | (mock until Phase 4)                                |
| `integrity_violations` | usize  | `tick_buffer.verify_all().len()` — added PR #56     |
| `live_feed_active`     | bool   | `DataPipeline::has_external_feed()` — added PR #56  |

### 2. `Qse` event

Adds a new audit-log row to the "QSE Write-Back Action Audit Logs"
table on `/monitoring`. Frontend treats each event as a single
row, appended newest-first.

```json
{
  "event_type": "Qse",
  "data": {
    "timestamp": "2026-05-21T20:30:45.123Z",
    "wbs": "WBS-9.6a",
    "operation": "set_calibration",
    "target": "MU-01 / ch4",
    "operator": "console:127.0.0.1",
    "result": "applied",
    "result_color": "green"
  }
}
```

| Field          | Type   | Notes                                                      |
| -------------- | ------ | ---------------------------------------------------------- |
| `timestamp`    | string | ISO-8601 (UTC). Mock-emitter writes `Utc::now()` per row.  |
| `wbs`          | string | Free-form WBS code or sub-system tag                       |
| `operation`    | string | Verb: `set_calibration`, `scd_upload`, `tamper_injected`, … |
| `target`       | string | Operator-readable target (MU id, channel, etc.)            |
| `operator`     | string | Source identity. Phase 0 stamps `console:<remote-ip>`      |
| `result`       | string | Outcome verb: `applied`, `rejected`, `degraded`, …         |
| `result_color` | string | UI hint: `green` / `amber` / `red` / `grey`                |

Phase 0 emits **mock QSE log rows** periodically (≈ one every 7 s)
so the audit table on `/monitoring` is non-empty during demos.
Real wiring — that emits one row per operator action and per QSE
write-back — lands when the QSE write-back path is built (paired
with ADR-0020, Phase 4).

### 3. `Waveform` event

Used by the dashboard's polar/phasor diagram and the MU-detail
live oscilloscope. One per 100 ms (10 Hz). The frontend samples
this into a per-MU scrolling buffer.

```json
{
  "event_type": "Waveform",
  "data": {
    "mu_id": "MU-01",
    "timestamp_ms": 1779392119788,
    "v1": 155.5, "v2": -77.7, "v3": -77.8, "v0": 0.0,
    "i1": 6.86,  "i2": -3.43, "i3": -3.43, "i0": 0.0
  }
}
```

| Field          | Type   | Units                                              |
| -------------- | ------ | -------------------------------------------------- |
| `mu_id`        | string | MU identifier (svID-derived in PR D)               |
| `timestamp_ms` | u64    | Unix milliseconds (PTP-disciplined when available) |
| `v1`/`v2`/`v3` | f32    | Per-phase voltage, volts (post-calibration)        |
| `v0`           | f32    | Neutral voltage; ≈ 0 in balanced 3-phase           |
| `i1`/`i2`/`i3` | f32    | Per-phase current, amperes (post-calibration)      |
| `i0`           | f32    | Neutral current; ≈ 0 in balanced 3-phase           |

PR E migrates this from the in-emitter mock waveform to the
actual `TickRecord::live_samples()` of the latest tick.

## Actionable for Simulator Agents

If you are modifying the simulator or the core SVDC engine to
produce realistic testing data:

- Dispatch metrics to the global `broadcast::Sender<String>` via
  `emitter::broadcast_event()` (or another `Sender::send` hook).
- To simulate QSE self-healing overrides, periodically emit
  `SsePayload::Qse(QseLog { ... })` so the audit trail on
  `/monitoring` populates.
- For per-MU waveform fidelity (PR E), feed
  `TickRecord::live_samples()[..n_channels]` into the existing
  `WaveformSample` shape after applying the per-channel
  calibration triple.

## Schema versioning

- **Add a field** to an existing variant → no version bump (the
  frontend treats unknown fields as ignorable; the backend
  struct has `#[serde(default)]` on new fields).
- **Add a new variant** (e.g. a future `Phasor`) → minor bump.
- **Remove or rename** a field → major bump; requires a versioned
  URL (e.g. `/api/events/v2`) so existing frontends survive.

Current variants: `Metrics`, `Waveform`, `Qse`. Schema **v1.0**.

## Reference Rust types

Authoritative types live in
[`crates/svdc-console/src/sse/mod.rs`](../crates/svdc-console/src/sse/mod.rs):

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "event_type", content = "data")]
pub enum SsePayload {
    Metrics(DashboardMetrics),
    Waveform(WaveformSample),
    Qse(QseLog),
}
```

When the Rust structs change, this doc changes in the same PR.

## Operator verification

Tail live events with curl (after the daemon is up):

```sh
curl -N http://127.0.0.1:8080/api/events

# Expected output:
#   data: {"event_type":"Waveform","data":{...}}
#
#   data: {"event_type":"Metrics","data":{...}}
#
#   data: {"event_type":"Qse","data":{"timestamp":"…","operation":"set_calibration",…}}
```

Both `/api/events` and `/sse/dashboard` produce identical output.

## See also

- [`docs/simulator-runbook.md`](simulator-runbook.md) — how to start the simulator that produces the data the SSE emits
- [`docs/northbound-simulators.md`](northbound-simulators.md) — northbound L0/L1/L2/L3 consumer plan
- [`docs/decisions/0015-simulator-driven-live-ui.md`](decisions/0015-simulator-driven-live-ui.md) — two-process architecture
- [`crates/svdc-console/src/sse/`](../crates/svdc-console/src/sse/) — implementation
