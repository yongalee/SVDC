# SVDC — Sampled Value Data Concentrator

A reference implementation of the SVDC component of the **a²SDP** (autonomous, adaptive,
secure Distribution Protection) architecture, as described in Meliopoulos et al.,
*Protection and Control of Active Distribution Systems*, CIGRE 2024 Paper ID 10427.

## What it does

Ingests IEC 61850-9-2 Sampled Value streams from multiple Merging Units, time-aligns them
under PTP-disciplined timestamps, buffers them in dual redundant circular buffers, and
exposes them to all node-local and operational applications through a layered northbound
interface:

- **L0** in-process C ABI / shared memory (sub-ms latency for EBP relays, QSE, Phasor
  Computation Module)
- **L1** OPC UA Server (SCADA, HMI, asset management — IEC 61850 ↔ OPC UA per OPC 10040)
- **L2** MQTT publisher (analytics, ML, cloud fan-out)
- **L3** TimescaleDB sidecar (historian, replay, audit)

Layers are failure-isolated: an L3 outage cannot affect L0.

## Status

Phase 0 (Foundation and Spec Lock). The data-plane crates and the operator
console run end-to-end on a single machine; the AF_PACKET ingress and the
real northbound layers (OPC UA, MQTT, TimescaleDB) are Phase 4–5 work. See
`CLAUDE.md` for current state.

## First-run quickstart

Two terminals, no hardware:

```sh
# Terminal A — start the SV simulator (any of the four vendor presets):
cargo run --release -p ssiec-sv-publisher -- udp 239.0.0.1:9100 \
    --vendor abb_relion_670 --duration 3600

# Terminal B — start the SVDC daemon with the live ingress feed:
cargo run --release -p svdc-bin -- \
    --ingress-udp 239.0.0.1:9100 \
    --operational-config /tmp/svdc-operational.toml \
    --audit-log /tmp/svdc-audit.jsonl

# Browser → http://127.0.0.1:8080
```

The full step-by-step is in [`docs/simulator-runbook.md`](docs/simulator-runbook.md);
the same procedure against a real vendor MU is in
[`docs/field-connection-guide.md`](docs/field-connection-guide.md).

> Until the follow-up PRs land (per [ADR-0015](docs/decisions/0015-simulator-driven-live-ui.md)
> §"Follow-up PR plan"), the `--ingress-udp` flag is **not yet wired**; the
> in-process synthetic pipeline on the `/dataplane` screen is the current
> operator path.

## Documents

- `docs/SVDC_Design_Document_v0.1.html` — Software Design Document
- `docs/SVDC_Implementation_Plan_v0.2.html` — Implementation Plan
- [`docs/simulator-runbook.md`](docs/simulator-runbook.md) — first-run operator procedure
- [`docs/field-connection-guide.md`](docs/field-connection-guide.md) — connecting a real vendor MU
- [`docs/mu-vendor-profiles.md`](docs/mu-vendor-profiles.md) — vendor wire-level reference table
- [`docs/decisions/`](docs/decisions/) — Architecture Decision Records (ADR-0001 … 0015)

Open the two HTML files in a browser.

## Contributing

This project is developed by **Shinsung Industrial Electric (SSIEC)** as a software
contribution to Prof. A. P. Sakis Meliopoulos's a²SDP research programme at Georgia Tech.
See `CONTRIBUTING.md` for code conventions (notably: English-only for all artefacts).

## License

Apache-2.0. See `LICENSE`.
