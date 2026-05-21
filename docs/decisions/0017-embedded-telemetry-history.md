# ADR 0017: Embedded Telemetry History and Alarm Persistence

## Context

The SVDC diagnostic telemetry dashboard currently relies on SSE streams to provide live real-time updates for MU Buffer Saturation, PTP discipline error, Arrival Jitter Histogram, and QSE Audit Logs. While this satisfies initial visibility goals, these metrics vanish on page reload, making it impossible to diagnose intermittent network spikes, historical jitter events, or QSE corrections that occurred while the operator was not actively watching the dashboard.

Furthermore, FR-6 mandates preserving compromised sample originals in an audit log. The SDD envisions an L3 TimescaleDB sidecar for historical persistence (Phase 4). However, the SVDC Console UI needs lightweight, immediate access to operational history without spinning up or depending on the heavyweight L3 TimescaleDB stack, maintaining the "Self-hostable on commodity Linux" single-binary philosophy for the control plane dashboard.

## Decision

We will introduce **SQLite** (`rusqlite`) into `svdc-console` as an embedded operational database (`svdc_console.db`) to persist UI telemetry history, alarms, and QSE audit logs.

1. **Schema Design**:
   - `telemetry_history`: Time-series records of MU buffer saturation, jitter, and PTP offsets. Capped to a rolling window (e.g., 24 hours or 1 week) to prevent infinite growth.
   - `alarms`: Events triggered when metrics cross a configured threshold (e.g., Jitter > 100ns).
   - `audit_logs`: Preservation of QSE overwrite requests.

2. **Alarm Threshold System**:
   - The SSE emitter loop will check real-time telemetry metrics against predefined thresholds.
   - If a threshold is violated, an alarm record is inserted into SQLite and broadcasted via SSE to notify connected clients.

3. **UI Integration**:
   - The `/monitoring` page will fetch the recent historical window from SQLite upon initial load to pre-populate charts, ensuring continuity across browser refreshes.
   - An Alarms panel will display active and historical threshold violations.

## Consequences

**Pros**:
- Solves the ephemeral stream problem; operators can review past incidents.
- Aligns perfectly with the self-hostable, dependency-free architecture of `svdc-console`.
- Paves the way for implementing FR-6 audit log preservation directly accessible from the UI.

**Cons**:
- Increases binary size slightly due to `rusqlite` dependencies.
- Introduces disk I/O to the console's SSE emitter path, necessitating asynchronous or background thread offloading for database writes to avoid blocking the L0/L1 telemetry pipes.
