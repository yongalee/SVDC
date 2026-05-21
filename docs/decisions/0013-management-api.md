# ADR-0013: `svdc-api` — management HTTP/JSON server (SDD §8.4)

- Status: Accepted
- Date: 2026-05-21
- Owner: claude-code
- Supersedes: —
- Superseded by: —
- Related: SDD §8.4 (Management & telemetry), ADR-0004 (UI stack),
  ADR-0005 (daemon / UI mode), ADR-0009 (TickBuffer), ADR-0012
  (integrity overlay), IP §9.2 WBS-3.5 / WBS-3.6

## Context

SDD §8.4 specifies four management endpoints — `GET /health`,
`GET /channels`, `GET /metrics`, `POST /calibration/{channel_id}` —
served on a configurable port and consumed by external monitoring
tools (Prometheus scrapers, the master-node QSE, factory test
harnesses).

This is a **different surface** from `svdc-console`. The console
serves the dashboard (HTML, htmx, SSE) and an operator-facing
`/api/*` namespace that talks to the same UI. The management API
is consumed by monitoring software, has different auth
requirements, different schema-stability requirements, and lives
on a different port. Mixing them in one crate would couple two
release cadences that should stay independent.

This ADR captures the management-API design choices so the Phase 3
work — write-through calibration, authentication, daemon wiring —
can land each piece behind a stable surface.

## Decision

### 1. Separate crate `svdc-api`, separate port

- Crate: `crates/svdc-api`. Depends on `svdc-core` (TickRecord,
  IntegrityViolation) and `svdc-aligner` (TickBuffer); does
  **not** depend on `svdc-console`. The two surfaces share no
  runtime types.
- Port: the daemon picks a separate bind for the management API.
  ADR-0005 keeps the console on loopback (127.0.0.1:8080); the
  management API defaults to the same host but on a different
  port (Phase 3 wiring decides the exact default, e.g. `:8081`).
  Cleanly maps to firewall rules: console = local, management =
  reachable from the monitoring VLAN.

### 2. `management_router(ctx) -> axum::Router` is the public surface

Phase 0 ships a single entry point: `management_router` returns a
fully wired `axum::Router<()>`. The daemon constructs an
`Arc<ManagementContext>`, calls `management_router(ctx)`, hands the
returned router to `axum::serve`. This keeps the daemon-wiring PR
small.

`ManagementContext` holds shared handles (uptime, `Arc<TickBuffer>`,
future `Arc<ChannelRegistry>`). It is non-`Clone` for the inner
fields — wrap the whole thing in `Arc` and share the Arc.

### 3. JSON DTOs are the wire contract; never remove a field

The shapes in `model.rs` (`HealthResponse`, `ChannelsResponse`,
`CalibrationDto`, `ApiError`, …) are the wire format consumers
bind against. Rules:

- **Add new optional fields freely.** Old consumers ignore them.
- **Never remove or rename a field.** That is a breaking change
  that needs a versioned URL (`/v2/health`).
- **Numeric ranges are part of the contract.** `uptime_ms` is
  `u128` because it survives PTP `time-travel` adjustments
  without underflow; do not narrow.

DTOs are intentionally distinct from `svdc_aligner::Calibration`
(which has the same field shape). The wire format and the
in-memory struct can diverge without forcing a data-plane recompile.

### 4. `GET /health` returns 200 unless the daemon is dead

The HTTP status code does **not** carry liveness information —
the `status` field inside the JSON does (`"ok"` vs `"degraded"`).
Consumers can rely on a 200 from `/health` meaning "the daemon
answered" and on parsing the body for the actual verdict.
Rationale:

- Some monitoring tools (Prometheus blackbox exporter) only key
  on status code. Putting "degraded but live" into a 503 would
  fire pager alarms when the data plane is merely lossy.
- The verdict is layered: `"ok"` / `"degraded"` is the current
  Phase 0 set; Phase 3 will add `"starting"` (during PTP lock)
  and Phase 5 `"failover"` (during a dual-CB swap).

### 5. `GET /metrics` is hand-rendered Prometheus text format

Format: `text/plain; version=0.0.4`. No `prometheus` crate dep —
the metric set is small and the rendering is mechanical. The
naming convention follows the Prometheus style guide: lowercase,
`svdc_` prefix, `_total` for counters, no suffix for gauges.

Phase 0 metric set:
- `svdc_uptime_ms` (gauge)
- `svdc_tick_buffer_len` (gauge)
- `svdc_tick_buffer_capacity` (gauge)
- `svdc_integrity_violations` (gauge — most recent sweep count)

Phase 3 will add counters: `svdc_frames_decoded_total`,
`svdc_frames_rejected_total`, `svdc_ticks_emitted_total`,
`svdc_subscribers_total`. Each is a one-line append.

### 6. `POST /calibration` validates Phase 0; writes through Phase 3

Phase 0 validates the request body (no NaN / Inf, non-zero `gain`
and `unit_scale`) and **echoes back** the would-be-applied triple
with HTTP 200. No data-plane mutation yet.

Phase 3 plugs in the operational-state write-through: the daemon
hands `ManagementContext` an `Arc<OperationalState>`, the handler
calls `state.set_calibration(channel_id, …)` and records the
change in the audit log. Validation and audit hook stay in this
crate; persistence stays in `svdc-console::operational` (per
ADR-0007).

### 7. Authentication deferred to Phase 5

Phase 0 ships no auth — local network or VPN-fronted use is
assumed. Phase 5 adds the auth shim (mTLS or token in header) per
SDD §11.2 (or wherever the security plan lands). The crate
deliberately exposes `management_router` rather than a full
"serve on socket" helper so the daemon owns the listener setup,
TLS, and middleware stack.

## Consequences

- Antigravity's UI work in `svdc-console` proceeds without
  contention. The two HTTP surfaces are independent.
- The integrity overlay from ADR-0012 finally has a public
  consumer: `/health` reports `verify_all()` violations and
  `/metrics` exposes them as `svdc_integrity_violations`.
- Daemon wiring is a one-file change in `svdc-bin` (start the
  management server alongside the console). That PR lands when
  `svdc-bin` is unlocked from concurrent edits.
- The wire contract is checked in (`model.rs`) and tested
  (`tests/routes.rs`). External consumers can grep the DTOs to
  generate their own typed clients.

## Out of scope

- Daemon wiring (separate PR).
- Authentication / TLS (Phase 5).
- Write-through calibration (Phase 3).
- Channel registry population (Phase 2 — depends on the SCD parser
  + aligner channel-registry indexing).
- Streaming endpoints (e.g. SSE for live tick records) — the
  console serves those; the management API is poll-only.
