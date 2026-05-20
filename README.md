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

Phase 0 (Foundation and Spec Lock). Not yet usable. See `CLAUDE.md` for current state.

## Documents

- `docs/SVDC_Design_Document_v0.1.html` — Software Design Document
- `docs/SVDC_Implementation_Plan_v0.2.html` — Implementation Plan

Open either HTML file in a browser.

## Contributing

This project is developed by **Shinsung Industrial Electric (SSIEC)** as a software
contribution to Prof. A. P. Sakis Meliopoulos's a²SDP research programme at Georgia Tech.
See `CONTRIBUTING.md` for code conventions (notably: English-only for all artefacts).

## License

Apache-2.0. See `LICENSE`.
