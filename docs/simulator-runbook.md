# SVDC simulator runbook

First-run UX with the SSIEC SV publisher **simulator** feeding the
**SVDC daemon** over UDP multicast. Two terminals, one browser,
no hardware required. See [ADR-0015](decisions/0015-simulator-driven-live-ui.md)
for design rationale; the same procedure with a real MU is in
[`field-connection-guide.md`](field-connection-guide.md).

Status as of this commit: this runbook describes the **target**
flow. The wiring lands in follow-up PRs B through E (per
ADR-0015 §"Follow-up PR plan"). Until then the operator path is
the in-process `/dataplane` demo from PR #51.

---

## What you need

- Rust toolchain (the repo's `rust-toolchain.toml` pins stable)
- Cloned repo, `cargo build --workspace` succeeds
- Two terminals
- A browser (anything modern — the UI is server-rendered)

Nothing else. No vendor hardware. No PTP grandmaster. No special
network setup. (The runbook is bench-only; the multicast traffic
stays on the loopback / lo interface.)

---

## The five-step procedure

### Step 1 — Build once

```sh
cargo build --release --workspace
```

Phase 0 takes ~90 s on a stock laptop. Subsequent runs are
incremental.

### Step 2 — Start the simulator (Terminal A)

```sh
cargo run --release -p ssiec-sv-publisher -- \
    udp 239.0.0.1:9100 \
    --vendor abb_relion_670 \
    --duration 3600
```

What's happening:

- `udp 239.0.0.1:9100` — emit SV payloads as UDP datagrams to
  the SV multicast group `239.0.0.1` on port `9100`. (The L2
  multicast MAC `01:0C:CD:04:00:01` maps onto IP multicast
  `239.0.0.1` by convention; the daemon joins the same group on
  Step 3.)
- `--vendor abb_relion_670` — frames look like an ABB Relion
  670 / SAM600 merging unit (APPID `0x4000`, VLAN PCP 4 VID 100,
  long ABB-style svID). Substitute `siemens_siprotec_5`,
  `ge_ur_series`, or `sel_2240` to switch vendor identity.
- `--duration 3600` — emit for one hour at the vendor's default
  rate (4800 frames / s = 17 280 000 frames in the run).

The simulator does **not** need any external file. The dummy
waveform (3-phase 60 Hz sinusoid + optional harmonics) is built
into `ssiec-sv-publisher::waveform`. Add `--harmonics 3,5,7` to
inject 5 % each at the listed harmonics (useful for THD demos).

Optional — replace the preset values with a real ICD file:

```sh
cargo run --release -p ssiec-sv-publisher -- \
    udp 239.0.0.1:9100 \
    --vendor abb_relion_670 \
    --vendor-icd docs/vendor-samples/abb_relion_670.icd \
    --calibration-csv docs/vendor-samples/abb_relion_670.csv \
    --duration 3600
```

The audit trail at startup prints which ICD fields overrode the
preset (APPID, svID, multicast MAC, VLAN-ID, smpRate, confRev).

### Step 3 — Start the daemon (Terminal B)

```sh
cargo run --release -p svdc-bin -- \
    --ui-bind 127.0.0.1:8080 \
    --ingress-udp 239.0.0.1:9100 \
    --operational-config /tmp/svdc-operational.toml \
    --audit-log /tmp/svdc-audit.jsonl
```

What's happening:

- `--ui-bind 127.0.0.1:8080` — operator console listens on
  loopback only (ADR-0005).
- `--ingress-udp 239.0.0.1:9100` — daemon joins the multicast
  group, receives the simulator's frames, decodes them, runs them
  through the aligner, lands them in the global `TickBuffer`.
- `--operational-config` — calibration triples and other
  SVDC-local state persist here (ADR-0007).
- `--audit-log` — operator-action history (ADR-0007 / PR #43).

Within ~250 ms of starting, the daemon's `tracing` output should
show:

```
INFO svdc_bin: ingress UDP listener 239.0.0.1:9100 bound
INFO svdc_bin: aligner thread started, bin period = 208333 ns
INFO svdc_console: operator console listening on http://127.0.0.1:8080
```

### Step 4 — Open the browser

<http://127.0.0.1:8080>

What you should see, panel by panel:

- **`/` Dashboard** — tick rate ≈ 4800 / s, MU count = 1,
  integrity OK. Numbers update every 500 ms (htmx polling).
- **`/south/mus` Merging Units** — one row, the vendor's svID
  (e.g. `AA1J1Q01A1MU01/LLN0$MX$Phsmeas9$svID` for ABB).
  Auto-registered from the incoming frames; no manual registration
  step required.
- **`/mu/{id}` MU detail** — click into the row. Live waveform
  scrolling, calibration form editable, three-phase phasor
  diagram rendering from the latest tick.
- **`/north` Northbound** — L0 historian shows real rows/sec.
  L1/L2/L3 stay mocked until Phase 4 wires real backends.
- **`/monitoring` Monitoring** — latency histogram (will swap
  from mock to live in Phase 5 PTP work).
- **`/dataplane`** — same as before, but the synthetic in-process
  pipeline is disabled when the UDP feed is active. The status
  panel reflects the **live** buffer.
- **`/api/mgmt/health`** — `{"status":"ok","data_plane":
  {"tick_buffer_len":N,"integrity_violations":0}}`.
- **`/api/mgmt/metrics`** — Prometheus text with `svdc_tick_buffer_len`
  tracking the live buffer.

### Step 5 — Try the operator settings

- On `/mu/{id}`, edit a calibration row (gain / offset / unit_scale)
  and save. The TOML in `/tmp/svdc-operational.toml` updates on
  disk; the audit log gains a `calibration_set` entry.
- On `/dataplane`, click **Inject tamper**. Within 500 ms the
  status panel flips to `degraded`; `/api/mgmt/health` JSON now
  shows `"status":"degraded"`.
- Restart **only Terminal A** with a different `--vendor`. The
  daemon does not need to restart — the new svID appears on
  `/south/mus`; the old svID rolls off the buffer within the
  retention window.

### Step 6 — Tear down

`Ctrl-C` Terminal A first, then Terminal B. The daemon flushes
the audit log and the historian CSV on shutdown.

---

## Verification checklist

If anything below isn't true, the integration isn't complete:

- [ ] Daemon starts without error against an idle network
      (UDP socket binds, multicast group joined).
- [ ] Within 1 second of the simulator starting, the daemon's
      tick-emit counter is non-zero.
- [ ] `/south/mus` shows the auto-registered svID with the right
      vendor inferred from the simulator's `--vendor` flag.
- [ ] `/mu/{id}` waveform updates visibly (60 Hz sinusoid, with
      harmonic distortion if `--harmonics` was passed).
- [ ] Edit a calibration row, save, see the audit log entry.
- [ ] Inject tamper from `/dataplane`, see `/api/mgmt/health`
      flip to `"degraded"`.
- [ ] Restart simulator with a different vendor, see the new svID
      arrive on `/south/mus`.
- [ ] Historian CSV at `$TMP/svdc-dataplane-demo.csv` (or the
      configured path) is appending ≈ 4800 rows/s while the
      simulator runs.

When all eight boxes check, the simulator → daemon → UI loop is
verified end-to-end without any hardware.

---

## Troubleshooting

| Symptom                            | Likely cause                          | Fix                                              |
| ---------------------------------- | ------------------------------------- | ------------------------------------------------ |
| Daemon `--ingress-udp` bind fails  | Port in use, or no multicast support  | Pick a different port; on Windows confirm Npcap installed |
| Daemon starts, MU list empty       | Simulator's UDP target differs        | Confirm both processes use the same `239.0.0.1:9100` |
| `/dataplane` shows running but `latest_tick_id` stays at zero | Decoder rejects payload | Check simulator output's `frame bytes` matches what the decoder expects (162 for ABB tag, etc.) |
| Vendor change on simulator restart doesn't reflect | TickBuffer still holds old svID | Wait for retention to roll off, or click `Reset` on `/dataplane` |
| `/api/mgmt/metrics` shows `svdc_tick_buffer_len 0` while simulator emits | Simulator is on a different host | Multicast across hosts needs IGMP / VLAN — see `field-connection-guide.md` §3 |
| Calibration edit doesn't persist   | `--operational-config` not passed     | Add the flag to the daemon command in Step 3     |
| Audit page empty after action      | `--audit-log` not passed              | Add the flag to the daemon command in Step 3     |

---

## What's still mock

Until Phase 5 (PTP wiring) and Phase 4 (real northbound), the
following stay synthetic even when the simulator is running:

- The latency histogram on `/monitoring` (no real `ingress→emit`
  timing data yet).
- L1 / L2 / L3 cards on `/north` (no real OPC UA / MQTT /
  TimescaleDB backends yet).
- The PTP / dual-CB / failover dials on the Dashboard (no real
  values yet; placeholders only).

These are flagged in the UI with a `mock` badge so the operator
can tell which numbers are live and which are placeholders.

---

## See also

- [ADR-0015](decisions/0015-simulator-driven-live-ui.md) — design rationale
- [ADR-0014](decisions/0014-vendor-profiles.md) — vendor profile design
- [`field-connection-guide.md`](field-connection-guide.md) — same procedure with a real MU instead of the simulator
- [`mu-vendor-profiles.md`](mu-vendor-profiles.md) — vendor wire-level reference
- [`vendor-samples/`](vendor-samples/README.md) — synthetic ICD + CSV files
