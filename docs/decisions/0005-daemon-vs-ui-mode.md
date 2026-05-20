# ADR-0005 — Daemon vs UI runtime mode

- **Status:** accepted
- **Date:** 2026-05-21
- **Deciders:** SSIEC SVDC team

## Context

The final SVDC deliverable to Prof. Meliopoulos's group ships with the
Operator Console (per ADR-0004 and UI Doc v0.1). The user has stated
two deployment scenarios:

1. **Demo / commissioning** — the Console is on; an operator interacts
   with the SVDC through it.
2. **Production unattended operation** — the Console is off; the SVDC
   runs headless as a daemon, exposing only the SDD §8.4 management API.

The runtime must support both without rebuilds or separate binaries
(NFR: single-binary deployment, per UI Doc §6 anti-stack).

## Decision

`svdc-bin` accepts CLI flags and environment variables to toggle the
Console at startup:

| Flag | Env var | Default | Effect |
|---|---|---|---|
| `--ui` | `SVDC_UI=1` | on | Start the Console HTTP server |
| `--no-ui` | `SVDC_NO_UI=1` | off | Skip the Console entirely |
| `--ui-bind <addr>` | `SVDC_UI_BIND=<addr>` | `127.0.0.1:8080` | Bind address for the Console |

Resolution order: CLI flag > env var > built-in default. `--no-ui` and
`--ui` are mutually exclusive; passing both is a startup error.

### Default rationale

- **UI on by default.** The primary deliverable use case is the
  professor's demo and the SSIEC team's commissioning workflow. Both
  expect the Console to be available without extra flags.
- **Bound to `127.0.0.1` by default.** The Console has no built-in
  authentication in v0.1 (UI Doc §8 Q1). Loopback-only binding ensures
  the Console is reachable from a local SSH tunnel or VNC session but
  not from the substation LAN. To expose the Console on a real
  interface, the operator must pass `--ui-bind 0.0.0.0:8080` (or a
  specific address) **and** is expected to put a reverse proxy with
  authentication in front of it. The reverse-proxy expectation is
  documented in the operator manual that lands with WBS-9.7.

### Runtime topology

```
svdc-bin (single binary)
├── core data plane    (always on — protection-critical)
├── management API     (always on — SDD §8.4)
└── svdc-console       (toggleable — ADR-0004 stack)
       └── runs on its own tokio runtime / thread pool
```

The Console runtime is constructed only when enabled. When disabled, the
`svdc-console` crate is still linked (a binary-size cost of ~1–2 MB) but
no thread is spawned, no port is bound, no embedded assets are
decompressed. This keeps the headless-daemon footprint quiet at runtime.

### Binary-size escape hatch

If the link-time cost of the `svdc-console` crate ever becomes a
problem on resource-constrained substations, `svdc-console` can be made
optional via a Cargo feature on `svdc-bin` (e.g.
`--features console-ui`, default-on). This is **not** done in v0.1
because the build complexity is not justified by the current target
hardware; recorded here so the option is visible.

## Consequences

- Operator manual must document the `--no-ui` / `--ui-bind` flags and
  the reverse-proxy expectation for non-loopback binds.
- WBS-9.7 acceptance tests must cover both modes: Console-enabled
  startup and Console-disabled (`--no-ui`) startup.
- Health-check endpoint (`GET /health`, SDD §8.4) remains on the
  management port regardless of Console state — this is the surface
  the daemon supervisor (systemd, k8s liveness probe) reads.
- The Console and the management API are on **different listening
  sockets**: the management API stays on its SDD-defined port; the
  Console binds where `--ui-bind` says. This means an operator can
  firewall the Console without affecting the management API.

## Alternatives considered

- **Compile-time feature flag only (no runtime toggle).** Rejected —
  requires shipping two binaries (Console-enabled and headless),
  contradicting the single-binary delivery model.
- **UI off by default.** Rejected — the demo use case is primary and
  should be friction-free.
- **Bind to `0.0.0.0` by default.** Rejected — opens the Console to
  the substation LAN with no authentication. Unsafe default.
- **Run Console on the same port as the management API.** Rejected —
  couples two surfaces that may need different firewall policies and
  forces the Console's HTML to live behind the management API's auth
  model.

## References

- `docs/SVDC_UI_Design_Document_v0.1.html` §8 Q1 (auth open question).
- ADR-0004 — Console technology stack.
- SDD §8.4 — management endpoint surface.
- UI Doc §2.4 — single-binary, no build-step requirement.
