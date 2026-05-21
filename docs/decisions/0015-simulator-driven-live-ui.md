# ADR-0015: Two-process simulator + live UI

- Status: Accepted
- Date: 2026-05-21
- Owner: claude-code
- Supersedes: —
- Superseded by: —
- Related: ADR-0008 (`svdc-ingress`), ADR-0009 (`svdc-aligner`),
  ADR-0014 (vendor profiles), `docs/simulator-runbook.md`,
  `docs/field-connection-guide.md`

## Context

The SVDC's eight Phase 0 data-plane crates work in isolation (210
tests on `main`) and the `/dataplane` UI screen drives an
**in-process synthetic pipeline** that exercises every crate
end-to-end. But the rest of the operator UI — Dashboard, Merging
Units list, MU Detail, Monitoring, Northbound — still renders from
**mock fixtures** (`templates::charts::mock_histogram`,
`scd::sample::SAMPLE_SCD_XML`, hand-coded JSON in
`routes/northbound.rs`, etc.).

The professor's first-impression test is "can a human start the
simulator, start the daemon, and watch live data appear in every
panel?" Today that answer is "only on /dataplane". This ADR closes
that gap by defining a **two-process architecture** the operator
runs by hand, and the **UI migration plan** that swaps each mock
panel onto the resulting live tick stream.

## Decision

### 1. Two processes, UDP multicast between them

```
┌──────────────────────────────┐         239.0.0.1:9100
│ Terminal A                   │ ─── UDP multicast ───┐
│   ssiec-sv-publisher         │                      │
│   --vendor abb_relion_670    │                      │
│   --duration 3600            │                      │
└──────────────────────────────┘                      │
                                                      ▼
                                          ┌────────────────────────────┐
                                          │ Terminal B                 │
                                          │   svdc-bin                 │
                                          │   --ingress-udp …          │
                                          │                            │
                                          │   ┌────────────────────┐   │
                                          │   │ UdpSubscriber      │   │
                                          │   │   → Decoder        │   │
                                          │   │   → IngressRing    │   │
                                          │   └────────────────────┘   │
                                          │            │               │
                                          │   ┌────────▼───────────┐   │
                                          │   │ Aligner thread     │   │
                                          │   │   → TickBuffer     │   │
                                          │   │   → Historian      │   │
                                          │   └────────────────────┘   │
                                          │            │               │
                                          │   ┌────────▼───────────┐   │
                                          │   │ Console (UI)       │   │
                                          │   │   /, /south/mus,   │   │
                                          │   │   /mu/{id},        │   │
                                          │   │   /monitoring,     │   │
                                          │   │   /dataplane,      │   │
                                          │   │   /api/mgmt/*      │   │
                                          │   └────────────────────┘   │
                                          └────────────────────────────┘
                                                      │
                                                      ▼
                                              Browser :8080
```

**Transport choice — UDP multicast**. AF_PACKET (the real
production transport) is Phase 5. UDP unicast doesn't survive
multi-subscriber scenarios. UDP multicast:
- Maps onto every OS (Linux / Windows / macOS).
- Matches the real production semantics: SV is multicast on the
  wire; a real subscriber joins the group.
- The simulator's existing `udp <addr:port>` mode already produces
  the right payload — only the receive side is new.
- The 4-byte 802.1Q VLAN tag is stripped at the UDP boundary
  (UDP is L4); the daemon receives the L2-stripped 9-2 LE payload
  starting at APPID.

**Default port**: `9100`. SV does not have an IANA-assigned port
(it is an L2 protocol). 9100 is reserved by IANA as "HP JetDirect"
but is the de-facto convention for SV-over-UDP demo tooling. The
flag is configurable.

### 2. New crate component: `svdc_ingress::UdpSubscriber`

Implements the existing `Subscriber` trait by binding a
`std::net::UdpSocket` to a multicast group, joining the group on
the default interface, and yielding `(payload, IngressTimestamp)`
per datagram. The payload is the L2-stripped 9-2 LE frame (APPID
onwards). The decoder must therefore accept either:

- a full L2 frame (current Wireshark-PCAP path), **or**
- a L2-stripped payload (UDP path).

The current decoder expects the Ethernet header. We extend it
with `decode_l2_stripped_frame(...)` that starts at the APPID, so
both call sites are explicit. The UDP subscriber calls the new
entry point.

Wire-level note: the L2-stripped payload does **not** carry
`dst_mac`/`src_mac`/`VLAN`. The `IngressFrame.timestamp` is the
kernel receive timestamp; the simulator's vendor identity is
inferred from the decoded `svID` instead of the missing source
MAC OUI.

### 3. `svdc-bin` daemon wiring

A new CLI flag `--ingress-udp <addr:port>`:

- When **set**: the daemon spawns a tokio task that runs
  `UdpSubscriber::next_frame()` in a loop, pushes each
  `IngressFrame` into a shared `IngressRing`, and runs an
  `Aligner::process_frame` loop on a blocking thread that drains
  the ring into the **same `TickBuffer`** that
  `svdc-console::dataplane::global()` already exposes to the UI.
- When **absent**: behaviour is unchanged — `/dataplane`'s manual
  Start button still spawns the in-process synthetic loop. The
  two modes are mutually exclusive: starting the UDP feed
  auto-stops the in-process demo.

Test isolation: the existing 210 tests assume no UDP feed. The
daemon-wiring change does not affect them; only the new
`--ingress-udp` codepath is exercised by an integration test that
binds an ephemeral port.

### 4. Live-data migration plan, per UI panel

The following table maps each currently-mocked screen to the live
data source it should consume after the daemon is fed by the
simulator. **One panel per follow-up PR** so the diff stays
reviewable.

| Panel              | Today's source                                            | Target source                                                      | Phase   |
| ------------------ | --------------------------------------------------------- | ------------------------------------------------------------------ | ------- |
| Dashboard (`/`)    | Hand-coded `dashboard.rs` cards                           | TickBuffer length / capacity / ticks-per-sec; live MU count        | Migrate |
| MU list (`/south/mus`) | SCD upload + sample SCD                              | **Auto-register** each decoded `svID` (with MAC-OUI → vendor hint) | Migrate |
| MU detail (`/mu/{id}`) | Calibration form + mock waveform                     | Live `live_samples()` waveform from the matching MU's last tick    | Migrate |
| Monitoring (`/monitoring`) | `charts::mock_histogram` deterministic seed      | HDR histogram of ingress→tick-emit latency                         | Phase 5 (keep mock until PTP lands) |
| Northbound (`/north`) | Layer enum + mock metrics                             | Real historian rows/sec (L0); rest stay mock until Phase 4 wires   | Partial |
| `/dataplane`       | In-process synthetic pipeline                             | Toggle between in-process and live UDP feed                        | Migrate |

The mock paths stay in source for tests + the
no-simulator-attached fallback. Each handler gains a single
`if let Some(live) = …` branch that prefers live data when the
TickBuffer is non-empty.

### 5. The simulator owns its own dummy data

The user is explicit:
*"시뮬레이터는 연결정보 뿐만 아니라, dummy 데이터도 갖고 있을 수 있도록 해줘"*
— "The simulator should carry not just connection info but also
dummy data."

The existing `WaveformConfig` (PR #42) already provides
configurable three-phase sinusoid + harmonics + power-factor lag
generation, internal to `ssiec-sv-publisher`. **This ADR confirms
that scope is correct**: the simulator carries its own dummy data
generator, no external file required.

Extension hooks queued for a follow-up:

- `--scenario <path>` accepts a small TOML file describing a
  scripted waveform (e.g. "step from balanced 100 % to 50 % at
  t=2 s", "inject 5th harmonic at t=5 s") so an operator can demo
  fault detection without a real disturbance.
- `--dummy-csv <path>` reads pre-recorded waveform samples from a
  CSV (e.g. exported from a transient recorder) and emits them
  on the wire.

Both are out of scope for this ADR; queued as `ssiec-sv-publisher`
WBS-6.x extensions when needed.

### 6. Operator runbook is the canonical onboarding doc

`docs/simulator-runbook.md` (lands with this ADR) is the
six-step procedure the operator follows on first run. It
prescribes:

1. Open two terminals.
2. Pick a vendor preset (or feed an ICD).
3. Start the simulator on UDP 239.0.0.1:9100.
4. Start the daemon with `--ingress-udp`.
5. Open the browser; confirm Dashboard, MU list, MU detail show
   live data.
6. Vary the vendor (`Ctrl-C`, restart simulator with a different
   `--vendor`); confirm the daemon picks up the change.

The runbook is paired with `docs/field-connection-guide.md`
(ADR-0014) — same six steps, but step 3 is replaced with "the
real MU starts publishing." Same daemon command, same browser
verification. The simulator and the real MU are operationally
interchangeable.

## Consequences

- The professor's first-run experience is now a two-terminal
  procedure that does not require any code edits or YAML files.
  Two `cargo run` invocations + a browser tab.
- The eight data-plane crates remain library-only; the new wiring
  is a single tokio task in `svdc-bin` plus a `UdpSubscriber` in
  `svdc-ingress`. No public-API churn on the boundary types.
- Existing mock UI panels keep working when no simulator is
  attached — the "demo without hardware OR simulator" path that
  pre-dates this ADR survives.
- `/dataplane`'s in-process synthetic loop becomes the
  **fallback** when no UDP feed is configured. Operator can still
  exercise the integrity-violation flow with **Inject tamper**
  whether or not the simulator is running.
- The vendor-selector follow-up PR (ADR-0014 §"Out of scope")
  doesn't need to touch the daemon — the operator switches
  vendors by restarting the simulator process, not by
  reconfiguring the daemon. The UI selector on `/dataplane` only
  needs to emit the next "this is the simulator command you
  should run" string for the operator to paste; advanced UX
  (auto-restart the simulator from the daemon) is deferred.

## Out of scope

- AF_PACKET ingress (Phase 5; needs Linux + root + NIC binding).
- Multi-publisher discovery beyond one svID (Phase 2 — touches
  the channel registry).
- Authentication on the UDP transport (Phase 5; the bench is
  trusted-network).
- Production cross-host deployment (this ADR is bench-only;
  multicast traversal across switches/routers needs IGMP
  configuration covered by the field-connection guide).
- Scenario / pre-recorded-CSV dummy-data variants for the
  simulator (queued as a follow-up; the existing waveform
  synthesiser is enough for the first-run UX).

## Follow-up PR plan

Numbered in the order they should land:

| PR    | Scope                                                             |
| ----- | ----------------------------------------------------------------- |
| **A** | This ADR + `docs/simulator-runbook.md` (this PR).                 |
| **B** | `UdpSubscriber` + `--ingress-udp` + `/dataplane` toggle + smoke test |
| **C** | Dashboard migration to live TickBuffer counters                   |
| **D** | MU list auto-registration from incoming svID                      |
| **E** | MU detail live waveform                                           |
| **F** | `/dataplane` vendor selector (user's queued follow-up)            |

Each PR is independent except for B→C/D/E ordering (the UI
migrations need the live TickBuffer to be populated). PR F can
land any time after this ADR.
