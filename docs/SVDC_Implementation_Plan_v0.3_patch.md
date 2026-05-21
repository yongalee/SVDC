# SVDC Implementation Plan — v0.3 patch

> This patch supplements `SVDC_Implementation_Plan_v0.2.html`. The HTML
> document itself is re-rendered as v0.3 after the user / professor
> review pass; until then, this Markdown patch is the canonical source
> for the WBS-9 addition.

## What changed in v0.3

A new WBS category — **WBS-9 Operator Console** — is added per
`docs/SVDC_UI_Design_Document_v0.1.html`. Effort spreads across
Phases 3–6 of the existing 13-week schedule. Total IP effort moves
from **102 PD → 117.5 PD**.

## WBS-9 expansion

Per UI Doc §7.1, with dual-agent lane assignment per
`docs/dual-agent/wbs-9-ui-handoff.md`:

| ID | Work item | Lane | PD |
|---|---|---|---|
| 9.1a | Console scaffold: `lib.rs`, base layout, sidebar, top bar | Claude | 1.0 |
| 9.1b | `svdc-console` crate scaffold, `rust-embed` wiring, assets directory | Antigravity | 1.0 |
| 9.2a | Dashboard screen + 4 tiles + typed SSE event contract | Claude | 1.0 |
| 9.2b | SSE emitter + 1 Hz mock data | Antigravity | 1.0 |
| 9.3a | MU detail screen + 10 Hz downsampler + 8-channel SVG | Claude | 2.0 |
| 9.3b | MU list page + per-MU card templating + 1 Hz vitals SSE | Antigravity | 1.0 |
| 9.4a | North-bound shell + enable/disable POST API | Claude | 1.5 |
| 9.4b | L0 / L1 / L2 / L3 layer detail cards | Antigravity | 1.5 |
| 9.5a | Latency histogram + p50/p99 + SVG renderer | Claude | 1.5 |
| 9.5b | PTP/CB line charts, audit-log table, 1 Hz polling | Antigravity | 1.0 |
| 9.6a | SCD upload validator + channel-registry model | Claude | 1.0 |
| 9.6b | Configuration form + About page + parameter wiring | Antigravity | 0.5 |
| 9.7 | Playwright end-to-end acceptance suite (5 workflows) | Antigravity | 1.5 |
| **Σ** | **WBS-9 Operator Console** | | **15.5 PD** |

Lane totals: **Claude 8.0 PD · Antigravity 7.5 PD.**

## Phase mapping (carries from UI Doc §7.2)

| Phase | WBS-9 work landing |
|---|---|
| Phase 3 (Sub + WB, W6–7) | 9.1a, 9.1b, 9.2a, 9.2b — UI scaffold + Dashboard |
| Phase 4 (Calib + Obs + Northbound, W8–9) | 9.3a, 9.3b, 9.4a, 9.4b, 9.6a, 9.6b — MU + Northbound + Config |
| Phase 5 (Performance, W10–11) | 9.5a, 9.5b — Monitoring (depends on benchmark histograms existing) |
| Phase 6 (Acceptance + Handover, W12–13) | 9.7 — Playwright E2E suite |

## Effort total update

| Item | v0.2 | v0.3 |
|---|---|---|
| WBS-1..8 | 102 PD | 102 PD |
| WBS-9 | — | 15.5 PD |
| **Total** | **102 PD** | **117.5 PD** |

Net schedule impact: zero. WBS-9 work is parallelizable with the
backend WBS items it surfaces. With two agents working in parallel,
the 13-week calendar window absorbs the +15.5 PD without slipping.

## Quality gates affected

- **G3 (end of Phase 3) demo** now includes a usable Dashboard
  (WBS-9.1 + 9.2 land in Phase 3).
- **G6 (end of Phase 6) handover** includes the Playwright suite
  passing (WBS-9.7).

## Dependency on spec-lock

WBS-9 work does **not** depend on the spec-lock Q1–Q6 answers. The
Console is a presentation layer; what it presents is whatever the
backend produces. The Phase 1 backend work (which does depend on
spec-lock Q2 for sample-rate) blocks the *data* the Console will
eventually display, but the Console scaffold can land ahead.

WBS-9.6a (SCD validator) does interact with spec-lock Q5 (whether a
reference SCD schema exists). If Q5 is still open when 9.6a is
picked up, the validator targets the SSIEC default schema described
in `docs/spec-lock-v0.1.md` Q5.

## References

- `docs/SVDC_UI_Design_Document_v0.1.html` — authoritative UI design
- `docs/decisions/0004-ui-stack.md` — tech-stack decision
- `docs/decisions/0005-daemon-vs-ui-mode.md` — `--no-ui` runtime mode
- `docs/dual-agent/wbs-9-ui-handoff.md` — lane partition and ordering
- `docs/SVDC_Implementation_Plan_v0.2.html` — base IP this patches

## WBS-9.9 Southbound Industrial Grid & Bulk Actions (v0.3.1 patch)

Following operator feedback that managing Merging Units (MUs) using a card layout is inefficient when the number of MUs scales up, we introduce **WBS-9.9: Southbound Industrial Grid & Bulk Actions** (Antigravity Lane, 1.0 PD) to Phase 4 (W8–W9). 

### Scope & Technical Specifications
- **Layout Shift:** Replace the current three-column grid card system on the `/south/mus` page with a high-density, single-row SCADA-style data grid (table). 
- **High-Density Metrics:** Display Selection Checkbox, MU Name/ID, Status Badge (pulsing indicator), IP/MAC Address, Sample Rate, Dropped Frames, Latency, and inline quick actions.
- **Client-Side Engine:** Wire up an Alpine.js framework block to perform instantaneous, zero-latency local search and status filtering (All, Healthy, Degraded, Disconnected) of MU rows.
- **Bulk Operations (Dynamic Toolbar):** Display a warm amber/blue sticky toolbar at the top of the data grid when one or more rows are checked. Expose:
  - **Bulk Ping:** Post a simulated concurrent ping to all checked Merging Units, updating their latency states in real-time.
  - **Bulk Calibrate:** Prompt a dynamic bulk calibration multiplier offset input box, applying adjustments concurrently to the selected devices.

### Effort & Schedule Impact
- **Owner:** Antigravity Lane
- **Estimated Effort:** 1.0 PD (Phase 4)
- **Net Schedule Impact:** Zero. This refinement fits within Phase 4 parallel UI/configuration implementation windows.

