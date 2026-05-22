# L1 OPC UA quickstart — self-verification recipe

**Goal:** confirm in 3 terminals that the SVDC daemon's L1 OPC UA
server (PR L+) publishes live channel values that the reference
client (PR M, this doc) can read. No external SCADA tool required.

## Prerequisites

- Workspace builds clean: `cargo build --workspace` once
- No other process on UDP `239.0.0.1:9100` or TCP `127.0.0.1:4840`

## Recipe (three terminals)

```sh
# Terminal A — southbound: simulate one MU pushing SV frames over UDP.
cargo run --release -p ssiec-sv-publisher -- udp 239.0.0.1:9100 \
    --vendor sel_2240 --duration 600

# Terminal B — SVDC daemon: ingest UDP, run L1 OPC UA server.
cargo run --release -p svdc-bin -- \
    --ingress-udp 239.0.0.1:9100 \
    --enable-opcua

# Terminal C — northbound: subscribe to L1, print data changes.
cargo run --release -p svdc-l1-opcua-client -- --samples 20
```

## What success looks like

**Terminal B** prints:

```
svdc-l1-opcua: server bound at 127.0.0.1:4840; anonymous, no security (ADR-0017 §5)
```

**Terminal C** prints (after `~1 s`):

```
svdc-l1-opcua-client: session established
svdc-l1-opcua-client: monitoring 16 items across 8 channels
[L1] Ch00_Va.instMag.i = Int32(4811) (Good)
[L1] Ch04_Ia.instMag.i = Int32(2200) (Good)
[L1] Ch00_Va.q = UInt16(0) (Good)
... (20 lines, then exits)
```

**Browser** at <http://127.0.0.1:8080/north/L1> shows:

- Status badge: `Wired · running` (green)
- `Last tick_id published` and `Total publishes` rise on each
  page reload
- "Verify with the L1 client simulator" card with a **Refresh
  status** button

## Troubleshooting

| Symptom                                      | Likely cause                                                         |
| -------------------------------------------- | -------------------------------------------------------------------- |
| Terminal C: `connect failed`                 | Terminal B not started yet, or port 4840 already in use              |
| Terminal C: connects but no `[L1] …` lines   | Terminal A not started (no upstream data); `/north/L1` stays "no traffic" |
| UI: `Wired · stub mode` badge persists       | Server bound but `set_values` has not fired — same root cause as above |
| UI: `Planned (Phase 4)`                      | Daemon started without `--enable-opcua`                              |
