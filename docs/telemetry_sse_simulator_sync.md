# SVDC Telemetry & Simulator Sync Document

**Target Audience:** Agents working on the SVDC Southbound Simulator (`ssiec-sv-publisher`) or Core Data Plane.

## Overview
The Diagnostic Telemetry UI (`monitoring.rs`) in the `svdc-console` has been upgraded to render **dynamic, real-time charts and logs** instead of static SVGs. It relies on the Server-Sent Events (SSE) broadcast stream available at `GET /api/events`.

## SSE Payload Contract
The UI consumes events formatted as JSON payloads defined in `crates/svdc-console/src/sse/mod.rs`. The `SsePayload` enum dictates the structure:

1. **`Metrics` event**: Updates the PTP disciplined clock offset chart and the Circular Buffer saturation area chart.
   - Requires `ptp_offset_ns` (i64) and `buffer_saturation` (f64).
2. **`Qse` event**: Adds a new audit log row to the "QSE Write-Back Action Audit Logs" table.
   - Requires `timestamp`, `wbs`, `operation`, `target`, `operator`, `result`, and `result_color`.
3. **`Waveform` event**: Used by the dashboard's polar diagram.

## Actionable for Simulator Agents
If you are modifying the simulator or the core SVDC engine to produce realistic testing data:
- Ensure that the metrics you generate are dispatched to the global `broadcast::Sender<String>` (via `emitter::broadcast_event()` or your own channel hook).
- To simulate QSE self-healing overrides, periodically emit `SsePayload::Qse(QseLog { ... })` events so the audit trail on the monitoring dashboard populates dynamically.
