# ADR-0004 — Operator Console technology stack

- **Status:** accepted (supersedes ADR-0002)
- **Date:** 2026-05-21
- **Deciders:** SSIEC SVDC team

## Context

`docs/SVDC_UI_Design_Document_v0.1.html` ("UI Doc") was added to the
repository as the authoritative specification for the Operator Console
that ships with the SVDC. It defines five screens (Dashboard, South-bound
MUs, North-bound layers, Monitoring, Configuration), the information
architecture, the real-time data flow, and the technology stack (UI Doc
§6).

ADR-0002 (`0002-web-based-ui-monitoring-dashboard.md`) predated the UI Doc
and made some technology choices that the UI Doc later overrode:

| Topic | ADR-0002 | UI Doc §6 |
|---|---|---|
| Push channel | WebSocket (`tokio-tungstenite`) | Server-Sent Events |
| Frontend interactivity | Vanilla JS direct DOM | HTMX + Alpine.js |
| Charts | Chart.js via CDN | Inline SVG generated server-side |
| Aesthetics | Glassmorphism dark theme, vibrant accents | Operator-grade, dense info, Inter + JetBrains Mono |
| Templates | (unspecified) | `maud` (type-safe HTML in Rust) |

The UI Doc is the authoritative SDD-level specification for the Console.
ADR-0002's choices are inconsistent with it and must be retired.

## Decision

ADR-0002 is superseded. The Operator Console adopts the stack specified
in UI Doc §6:

| Layer | Choice |
|---|---|
| HTTP server | `axum` |
| Template engine | `maud` (type-safe HTML in Rust) |
| Asset bundling | `rust-embed` (CSS, JS, fonts embedded into the binary) |
| Frontend interactivity | HTMX + Alpine.js (self-hosted, no CDN) |
| Live updates | Server-Sent Events (one-way push) |
| Charts | Inline SVG generated server-side; optional Chart.js for the Monitoring page if SVG proves limiting |
| Fonts | Inter (body) and JetBrains Mono (mono) self-hosted |
| Styling | Plain CSS with CSS variables; no preprocessor |

The Console lives in a **new crate `svdc-console`**, linked into
`svdc-bin` so the SVDC remains a single binary. ADR-0005 specifies the
runtime toggle that enables, disables, or rebinds the Console.

Anti-stack (per UI Doc §6 callout): no React, no Vue, no SPA framework,
no Vite/webpack/esbuild, no Node toolchain at any point in the build.
`cargo build --release` produces the binary; nothing else is required.

## Consequences

- Antigravity's earlier WebSocket + Chart.js skeleton work (if any was
  drafted but not landed) is discarded.
- New workspace dependencies land: `axum`, `tokio`, `maud`, `rust-embed`,
  `serde`, `serde_json`. Approximate release-binary size increase: 1–2 MB.
  Acceptable per UI Doc §6 cons table.
- The Console runs on a separate `tokio` thread pool from the hot path
  (per ADR-0002 §1 "Strict Isolation," which remains valid here even
  though the surrounding ADR is superseded). NFR-4 (no allocation on
  hot path) is unaffected.
- WBS-9 (Operator Console) is added to the IP — see
  `docs/SVDC_Implementation_Plan_v0.3_patch.md`.

## Alternatives considered

- **Keep ADR-0002.** Rejected — directly contradicts the UI Doc, which
  is the canonical specification.
- **WebSocket instead of SSE.** Rejected per UI Doc §5 callout: the
  Console is read-mostly, operator inputs are infrequent and tolerate
  REST round-trips, SSE is simpler and proxy-friendly.
- **Separate frontend project (React/Vue SPA).** Rejected per UI Doc §6
  anti-stack: violates the single-binary deployment property and adds a
  Node toolchain to a Rust project.
- **No UI in v0.1.** Rejected per the user's directive that the final
  delivery to Prof. Meliopoulos includes the UI.

## References

- `docs/SVDC_UI_Design_Document_v0.1.html` — authoritative Console
  specification (§4 screens, §6 stack, §7 WBS-9 expansion).
- `docs/SVDC_Implementation_Plan_v0.3_patch.md` — IP patch adding WBS-9.
- `docs/dual-agent/wbs-9-ui-handoff.md` — Claude / Antigravity lane
  partition for executing WBS-9.
- ADR-0001 — dual-agent workflow.
- ADR-0002 — superseded.
- ADR-0005 — daemon vs UI mode runtime toggle.
