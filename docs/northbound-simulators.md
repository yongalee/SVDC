# SVDC northbound simulator runbook

Sister document to [`simulator-runbook.md`](simulator-runbook.md).
The southbound runbook covers "simulator feeds SVDC"; this one
covers "SVDC feeds simulated consumers" — one reference consumer
per northbound layer, so the operator can verify every layer
without a real SCADA / MQTT broker / historian / PDC on the bench.

Design rationale is in
[ADR-0016](decisions/0016-northbound-simulators.md). Per-layer
schema decisions land in ADRs 0017–0021 as their server-side
implementations arrive (mostly Phase 4).

---

## At a glance

```
   ┌──────────────────────────┐                 ┌──────────────────────────────────┐
   │ ssiec-sv-publisher       │   UDP multicast │ svdc-bin --ingress-udp           │
   │   (south simulator)      │ ──────────────► │                                  │
   │   Terminal A             │                 │   data plane → TickBuffer        │
   └──────────────────────────┘                 │                  │               │
                                                │                  ▼               │
                                                │   M3→M4 northbound layers:       │
                                                │     L0  in-process API           │
                                                │     L1  OPC UA Server   :4840    │
                                                │     L2  MQTT publisher  :1883    │
                                                │     L3  TimescaleDB     :5432    │
                                                │     C37.118 master PDC  :4712    │
                                                └────────┬─┬─┬─┬─┬─────────────────┘
                                                         │ │ │ │ │
                          ┌──────────────────────────────┘ │ │ │ │
                          │     ┌──────────────────────────┘ │ │ │
                          │     │      ┌─────────────────────┘ │ │
                          │     │      │       ┌───────────────┘ │
                          ▼     ▼      ▼       ▼                 ▼
                         L0    L1     L2      L3            C37.118
                       client  OPC    MQTT  TimescaleDB     PDC sim
                              client client  query
```

Each box on the right is a separate `cargo run` invocation. The
client binaries are named by their layer.

---

## Status matrix

| Layer | Server side                  | Client simulator binary       | Status              |
| ----- | ---------------------------- | ----------------------------- | ------------------- |
| L0    | `svdc-subscribe` (PR #47)    | `svdc-l0-consumer-demo`       | **Lands in PR H**   |
| L1    | `svdc-opcua` (Phase 4)       | `svdc-l1-opcua-client`        | Phase 4 (paired)    |
| L2    | `svdc-mqtt` (Phase 4)        | `svdc-l2-mqtt-subscriber`     | Phase 4 (paired)    |
| L3    | `svdc-historian-tsdb` (Phase 4) | `svdc-l3-historian-query`  | Phase 4 (paired)    |
| C37.118 | Phasor Computation Module (Phase 4) | `svdc-c37118-pdc-sim` | Phase 4 (paired)    |
| COMTRADE | Fault writer (Phase 5)    | `svdc-comtrade-dump`          | Phase 5 (paired)    |

This page describes the **target** per-layer procedures. Sections
matched by a "Status: not yet wired" badge stay informational
until their PR lands.

---

## L0 — In-process consumer (ready: PR H)

The L0 surface is the highest-performance binding: zero-copy
access for performance-critical applications (EBP relays, Phasor
Computation Module, QSE). The reference consumer is a thin tokio
task that subscribes via `svdc_subscribe::InProcessSubscriber`,
drains every fresh tick via `read_since()`, and prints a
configurable summary line to stdout.

### Run

```sh
# Terminal A — start the southbound simulator (any vendor):
cargo run --release -p ssiec-sv-publisher -- udp 239.0.0.1:9100 \
    --vendor sel_2240 --duration 3600

# Terminal B — daemon with L0 demo enabled:
cargo run --release -p svdc-bin -- \
    --ingress-udp 239.0.0.1:9100 \
    --enable-l0-demo

# What you should see in Terminal B (in addition to the usual
# operator-console boot lines):
#
#   svdc-l0-demo: subscribed (cursor = 0)
#   svdc-l0-demo: tick_id=480  ts=1717603200000000000 ch0=4811 ch4=22987 …
#   svdc-l0-demo: tick_id=960  ts=1717603200100000000 ch0=4682 ch4=22340 …
#   ...
```

The demo subscriber loops every 100 ms, calls `read_since()`,
prints a one-line summary for every Nth tick, and reports a "ticks
behind" counter when the buffer is rolling faster than the
consumer drains. This mirrors what an EBP relay would do —
except a real relay applies a protection algorithm to each tick
instead of printing.

### Verification

- Within 1 s of the daemon starting, the L0 demo prints its first
  tick.
- Tick IDs are monotonic; the demo never reports a gap when the
  buffer is not under pressure.
- `Ctrl-C` shuts down both processes cleanly; the demo joins on
  shutdown without dropping in-flight records.

---

## L1 — OPC UA SCADA client

Status: **not yet wired**. Lands together with the
`svdc-opcua` server in PR J (per ADR-0017).

When ready, the simulator will:

1. Open an OPC UA session against `opc.tcp://127.0.0.1:4840`.
2. Browse the SVDC namespace (per ADR-0017 address space mapping).
3. Subscribe to the per-channel `instMag.i` and `q` nodes for
   one MU.
4. Print sample updates to stdout at ~1 Hz (subsampled from the
   4800 Hz tick rate).

Run shape (target):

```sh
cargo run --release -p svdc-l1-opcua-client -- \
    --endpoint opc.tcp://127.0.0.1:4840 \
    --mu-id SVDC_DEMO_PB_MU \
    --rate 1
```

Real-world counterpart: any commercial OPC UA SCADA (Honeywell
Experion, ABB 800xA, Siemens WinCC, OSIsoft PI Connectors) can
connect to the same endpoint with the same address space. The
simulator's job is to prove the address space is browseable and
subscriptions deliver before the operator drags in the real SCADA.

---

## L2 — MQTT analytics subscriber

Status: **not yet wired**. Lands together with the
`svdc-mqtt` publisher in PR L (per ADR-0018).

When ready, the simulator will:

1. Connect to the MQTT broker (`mqtt://127.0.0.1:1883`).
2. Subscribe to `svdc/+/sv/+` (or the schema ADR-0018 settles on).
3. Print each message's topic + payload (JSON or CBOR per the
   schema decision).

Run shape (target):

```sh
cargo run --release -p svdc-l2-mqtt-subscriber -- \
    --broker 127.0.0.1:1883 \
    --topic 'svdc/+/sv/+'
```

Real-world counterpart: any MQTT-capable analytics pipeline —
Kafka with MQTT Source, AWS IoT Core, Azure IoT Hub, or
Node-RED for prototyping. Same connect-subscribe-receive pattern;
swap the broker URL.

---

## L3 — TimescaleDB historian query

Status: **not yet wired**. Lands together with the
`svdc-historian-tsdb` sidecar in PR N (per ADR-0019).

When ready, the simulator will:

1. Open a postgres connection to the TimescaleDB sidecar.
2. Run a sample window query (e.g. "last 60 seconds of channel 0
   for MU X").
3. Print row count + first / last timestamps + sample values to
   stdout.

Run shape (target):

```sh
cargo run --release -p svdc-l3-historian-query -- \
    --dsn 'postgres://svdc@127.0.0.1:5432/svdc_historian' \
    --mu-id SVDC_DEMO_PB_MU \
    --window-seconds 60
```

Real-world counterpart: Grafana dashboards reading the same
hypertables, jupyter notebooks doing post-fault analysis, or
custom dashboards inside the utility's existing PI / OSIsoft
infrastructure. The schema (ADR-0019) is documented so any of
those can query the same way.

---

## Cross-node — IEEE C37.118.2 master-node PDC

Status: **not yet wired**. Lands together with the Phasor
Computation Module in PR P (per ADR-0020).

This is the **single most important integration point** per the
CIGRE 2024 paper: the SVDC's local node emits synchrophasor
frames upstream to the master node's PDC, which aggregates many
local nodes. Without this, the a²SDP architecture doesn't close.

The simulator plays the role of the master-node PDC:

1. Open a TCP listener on `0.0.0.0:4712` (default C37.118 PDC port).
2. Wait for the SVDC's Phasor Computation Module to connect.
3. Negotiate the configuration frame (CFG-2).
4. Receive data frames at the configured reporting rate (typ.
   30 / 60 / 120 Hz).
5. Print per-channel phasor magnitude + angle to stdout.

Run shape (target):

```sh
cargo run --release -p svdc-c37118-pdc-sim -- \
    --listen 0.0.0.0:4712 \
    --idcode 200
```

Real-world counterpart: any commercial PDC — SEL-5073, OSI's
openPDC, GE PhasorPoint, ABB's RES670 PDC. Each speaks
C37.118.2 the same way; the simulator is the literal interop
test before the operator wires in the real PDC.

---

## File exchange — COMTRADE

Status: **not yet wired**. Lands in PR Q (per ADR-0021).

For fault analysis, the SVDC writes COMTRADE (`.cfg` + `.dat`)
files when a triggered fault window is captured. The "simulator"
here is the **reader** — a thin binary that parses the COMTRADE
files and prints their contents.

This is less of a runtime simulator and more of a verification
tool: feed it the COMTRADE files SVDC produced, confirm any
commercial fault-analysis tool (DOBLE F6150, OMICRON CMC,
Wavewin) would also accept them.

---

## Cross-references

- [`docs/decisions/0016-northbound-simulators.md`](decisions/0016-northbound-simulators.md) — design rationale, integration roster, full PR plan
- [`docs/decisions/0010-subscriber-api.md`](decisions/0010-subscriber-api.md) — L0 in-process subscriber API design
- [`docs/decisions/0013-management-api.md`](decisions/0013-management-api.md) — management API surface (different from northbound — monitoring/control, not data)
- [`docs/simulator-runbook.md`](simulator-runbook.md) — paired southbound runbook
- [`docs/field-connection-guide.md`](field-connection-guide.md) — paired southbound real-MU procedure
- CIGRE 2024 Paper ID 10427 — the driver document

---

## Operator checklist (when all layers are wired)

For the eventual "everything turned on" demo:

- [ ] Southbound simulator running (`ssiec-sv-publisher udp ...`)
- [ ] Daemon running with `--ingress-udp` + all northbound layers enabled
- [ ] L0 consumer demo printing live ticks
- [ ] L1 OPC UA client browsing the address space + receiving updates
- [ ] L2 MQTT subscriber receiving topic messages
- [ ] L3 historian query returning recent rows
- [ ] C37.118 PDC simulator receiving configuration + data frames
- [ ] COMTRADE-dump tool successfully reads a captured fault window
- [ ] Browser `/dataplane` shows the live buffer + the consumer count

When all eight boxes check, the SVDC's northbound surface is
verified against every layer the CIGRE paper specifies — without
any external system on the bench.
