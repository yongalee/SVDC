# ADR-0016: Northbound simulators + existing-systems integration

- Status: Accepted (planning)
- Date: 2026-05-21
- Owner: claude-code
- Supersedes: —
- Superseded by: —
- Related: ADR-0010 (subscriber API), ADR-0011 (CSV historian),
  ADR-0013 (management API), ADR-0015 (two-process simulator),
  CIGRE 2024 Paper ID 10427 (Meliopoulos et al., *Protection and
  Control of Active Distribution Systems*)

## Context

The southbound simulator (`ssiec-sv-publisher`) is complete: it
mimics four vendor merging units (ABB / Siemens / GE / SEL), feeds
the SVDC daemon over UDP multicast, and the operator can verify
ingestion end-to-end on a bench with no hardware (ADR-0015, PR
#54).

The **northbound** side is the inverse problem. The SVDC's purpose
is to serve four layers of consumers (SDD §8.2, layered M4 surface):

| Layer | Transport                        | Real-world consumers                                               |
| ----- | -------------------------------- | ------------------------------------------------------------------ |
| L0    | In-process C ABI + UDS           | EBP relays, Phasor Computation Module, QSE, Transient Recorder, Fault Locator |
| L1    | OPC UA (IEC 62541 + OPC 10040)   | SCADA, HMI, asset management, vendor SCADA tools                   |
| L2    | MQTT (5.0)                       | Analytics, ML pipelines, cloud fan-out                             |
| L3    | TimescaleDB sidecar              | Historian, replay, audit, Grafana                                  |

Plus two **cross-system** interfaces the CIGRE paper calls out:

| Interface       | Spec               | Counterpart                                          |
| --------------- | ------------------ | ---------------------------------------------------- |
| Northbound PDC  | IEEE C37.118.2     | Master-node Phasor Data Concentrator                 |
| File exchange   | COMTRADE (IEEE C37.111) | Transient analysis tools, third-party fault locators |

Without **reference consumers** for each, the professor cannot
verify any northbound layer end-to-end. The mock cards on
`/north` (PR #39) show the right boxes but no real client has
connected to any of them.

This ADR plans the **northbound simulator** — a family of
reference consumer binaries, one per layer, that mimic the
**real systems** an operator would actually wire to the SVDC in
the field. The plan is staged to match each layer's server-side
landing PR (Phase 4 mostly).

## Decision

### 1. One reference consumer per layer; each ships as its own binary

Mirroring the south-bound design (`ssiec-sv-publisher` is a
standalone binary), each northbound layer gets a paired
**`svdc-l*-client`** binary in this workspace:

| Binary                       | Layer | Mimics                       | When |
| ---------------------------- | ----- | ---------------------------- | ---- |
| `svdc-l0-consumer-demo`      | L0    | EBP relay / Phasor module    | **Phase 0** (now — in-process API exists) |
| `svdc-l1-opcua-client`       | L1    | OPC UA SCADA / HMI client    | Phase 4 (after L1 server lands)            |
| `svdc-l2-mqtt-subscriber`    | L2    | MQTT analytics consumer      | Phase 4 (after L2 publisher lands)         |
| `svdc-l3-historian-query`    | L3    | TimescaleDB / Grafana query  | Phase 4 (after L3 sidecar lands)           |
| `svdc-c37118-pdc-sim`        | C37.118 | Master-node PDC            | Phase 4 (after Phasor Computation Module)  |
| `svdc-comtrade-dump`         | COMTRADE | Transient analysis tool   | Phase 5                                    |

Each binary is **deliberately minimal** — it does the smallest
thing that exercises the layer end-to-end:

- subscribe / poll the layer's transport,
- decode the layer's payload,
- log to stdout in a human-readable shape,
- exit cleanly on Ctrl-C.

Operator verification is "I started the simulator → I ran the
client → I see live data scrolling in the client's terminal."
Same UX as the southbound simulator runbook (ADR-0015).

### 2. Each layer's payload schema is its own ADR

The simulators are thin; the schemas they consume are not.
Each layer's wire-level mapping gets its own ADR (to be written
when the server-side lands):

| ADR  | Subject                                                |
| ---- | ------------------------------------------------------ |
| 0017 | OPC UA address space mapping: `TickRecord` → nodes     |
| 0018 | MQTT topic structure + payload schema (JSON / CBOR)    |
| 0019 | TimescaleDB schema, hypertables, retention policy      |
| 0020 | IEEE C37.118.2 frame mapping from `TickRecord` + phasor computation |
| 0021 | COMTRADE writer: config (`.cfg`) + data (`.dat`) layout |

This ADR commits the **simulator** scope; the **server-side**
schemas are decoupled so the L1/L2/L3 implementation PRs can land
incrementally without re-relitigating the wire format.

### 3. The CIGRE paper drives the integration target

Per CIGRE 2024 ID 10427 §3 + §7 + §10, the a²SDP local node is
the SVDC's customer. Cross-checking the paper's integration points
against our layer roster:

| Paper section | Existing system role          | Our layer | Status            |
| ------------- | ----------------------------- | --------- | ----------------- |
| §3 EBP relays | Per-zone protection algorithms | L0       | client demo Phase 0 |
| §3 QSE        | Self-healing state estimator (writes back) | L0 + write-back | server-side Phase 4 |
| §3 Phasor Computation Module | Synchrophasor + emits C37.118 | L0 + cross-node | Phase 4 |
| §3 Transient Recorder | Captures fault windows  | L3        | Phase 4–5         |
| §3 Fault Locator | Cross-zone impedance analysis | L1 / L0 | Phase 4–5         |
| §4 (master node)   | PDC receives C37.118        | cross-node | Phase 4         |
| §10 Test procedures | Factory + field acceptance  | all       | runbook Phase 4   |

In every column, the SVDC's northbound surface is the seam where
a real existing utility system would plug in. The simulator's job
is to **be that real system** for bench testing.

### 4. The southbound contract is the symmetric template

The southbound simulator wins because the operator can run two
`cargo run` invocations and see live data in a browser. Each
northbound simulator follows the **same UX contract**:

- One binary, one `cargo run` line, one URL or terminal output.
- A `docs/northbound-runbook.md` companion to
  `docs/simulator-runbook.md` documents the per-layer commands.
- Each simulator emits a one-line summary at startup printing
  the wire-level details it expects (matches publisher's
  `print_summary_vendor` shape).
- Each simulator accepts `--vendor`, `--ingress-target`,
  `--duration` flags consistent with the publisher's CLI where
  it makes sense.

### 5. L0 reference consumer lands now; the rest stage per layer

L0 is the only layer whose server side already works
(`svdc-subscribe` + `InProcessSubscriber`, PR #47). The L0
consumer demo is therefore the next northbound deliverable
(scoped as **PR G** in the cumulative follow-up plan below).

L1/L2/L3 and C37.118 simulators land **with** their server-side
implementation PRs — each pair is a single deliverable from the
operator's point of view ("OPC UA server + client both work").

### 6. The northbound runbook is the operator's onboarding doc

Sister to `docs/simulator-runbook.md`, the new
`docs/northbound-simulators.md` lays out the per-layer procedures
that don't exist yet so the future PRs land into an existing
doc skeleton. Operators of any vintage of the project see one
consistent runbook that grows section-by-section as the layers
turn on.

## Consequences

- Every northbound layer gets a paired client simulator in this
  repo. Third parties writing real consumers can copy the
  reference and modify; tests can ride the same binary.
- The northbound surface becomes **the same UX as southbound**:
  start the daemon, start the client, watch live data. No vendor
  hardware, no external broker, no Grafana setup required for
  the first verification pass.
- The ADR roster grows by five (0017–0021) — one per layer's
  schema. Each is a single-decision document, not a kitchen sink.
- The runbook becomes the single source of truth for "how do I
  exercise this layer?" Both for the simulator path and (per
  ADR-0014 / `field-connection-guide.md`) the real-equipment path.
- Cross-node integration (C37.118 to master PDC) is **explicitly
  in scope** for the Phase 4 horizon — the paper requires it.
  Phasor computation is the prerequisite; the simulator (a PDC
  receiver) is the verification harness.

## Cumulative follow-up PR plan (updated from ADR-0015)

| PR | Scope                                                              | Phase |
| -- | ------------------------------------------------------------------ | ----- |
| A  | ✅ ADR-0015 + simulator-runbook + README                            | 0     |
| B  | ✅ UdpSubscriber + `--ingress-udp` + `/dataplane` toggle             | 0     |
| C  | Dashboard live counters from TickBuffer                            | 0     |
| D  | MU list auto-registration from incoming svIDs                      | 0     |
| E  | MU detail live waveform                                            | 0     |
| F  | `/dataplane` vendor selector (queued from ADR-0014)                | 0     |
| **G** | **ADR-0016 + northbound-simulators runbook (this PR)**          | **0** |
| **H** | **L0 in-process consumer demo (`svdc-l0-consumer-demo`)**       | **0** |
| I  | ADR-0017: OPC UA address space + `svdc-opcua` server scaffold      | 4     |
| J  | `svdc-l1-opcua-client` simulator                                   | 4     |
| K  | ADR-0018: MQTT topic schema + `svdc-mqtt` publisher scaffold       | 4     |
| L  | `svdc-l2-mqtt-subscriber` simulator                                | 4     |
| M  | ADR-0019: TimescaleDB schema + `svdc-historian-tsdb` sidecar       | 4     |
| N  | `svdc-l3-historian-query` simulator                                | 4     |
| O  | ADR-0020: Phasor Computation Module + C37.118 emission             | 4     |
| P  | `svdc-c37118-pdc-sim` master-node PDC simulator                    | 4     |
| Q  | ADR-0021: COMTRADE writer for fault windows                        | 5     |

## Out of scope

- DNP3 / Modbus / IEC 61850-90-5 routable GOOSE northbound — these
  are common in legacy substations but not in the a²SDP scope.
  Treat as Phase 5+ if a deployment demands them.
- IEC 61850-90-2 inter-substation comms (used for wide-area
  protection) — out of the SVDC's role; that's the master node's
  job per the CIGRE paper.
- Authentication / role-based access control on northbound
  consumers — Phase 5 security plan.
- Failover / load-balancing across multiple SVDC instances —
  Phase 5+ deployment topology.

## References

- CIGRE 2024 Paper ID 10427: Meliopoulos et al., *Protection and
  Control of Active Distribution Systems*. The driver document.
- IEC 62541 (OPC UA), IEC 61850-90-1 (OPC UA mapping)
- OPC Foundation 10040 (Power System Models Companion Spec)
- MQTT 5.0 (OASIS standard)
- IEEE C37.118.1-2011 (synchrophasor data) + C37.118.2-2011 (transport)
- IEEE C37.111-2013 (COMTRADE)
