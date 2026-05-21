# ADR-0017: L1 OPC UA server — library, address space, and thin-slice scope

- Status: Accepted (planning)
- Date: 2026-05-22
- Owner: claude-code
- Supersedes: IP v0.2 §3.7 library choice (`open62541`) for the
  Phase 4 thin slice; the `open62541` choice may be reinstated at
  Phase 5 if OPC UA PubSub becomes a hard requirement.
- Superseded by: —
- Related: ADR-0010 (subscriber API), ADR-0016 (northbound
  simulators), SDD §7 (TickRecord), SDD §7.2 (channel registry),
  SDD §8.2 (northbound subscriber API), IP v0.2 §3.7 (WBS-3.7),
  IEC 62541 (OPC UA), OPC 10040 (IEC 61850 ↔ OPC UA companion
  specification), IEC 61850-90-1 (PoCom application of OPC 10040).

## Context

L1 is the "industrial integration" northbound layer per ADR-0016:
SCADA, HMI, and engineering tools speak OPC UA, and the SVDC has
to publish its aligned `TickRecord` stream into an OPC UA
Information Model so those tools can subscribe without bespoke
glue. SDD v0.1 §8.2 does not specify L1 — the SDD scopes only the
in-process C ABI and the out-of-process UDS binding. The L1
adapter is an IP v0.2 §1.4 extension introduced to align with the
OPC Foundation's IEC 61850 ↔ OPC UA companion specification
(OPC 10040).

The `/north/L1` detail page (PR I) currently renders a "Planned
(Phase 4)" stub that points at *this* ADR. ADR-0016 §86 reserved
the slot. Three things have been deferred to this ADR:

1. **Library choice.** IP §3.7 names `open62541` (a mature C
   library). The workspace is Rust. Linking `open62541` via FFI is
   feasible (`open62541-sys` exists) but adds a CMake build
   dependency, a third-party allocator, and an `unsafe` boundary
   the rest of the workspace doesn't have.
2. **AddressSpace mapping.** A canonical mapping from
   `TickRecord` + `ChannelRegistry` to OPC UA nodes is required so
   SCADA clients see the same shape across deployments. OPC 10040
   provides the rules; this ADR locks the SVDC's interpretation of
   them.
3. **Thin-slice scope.** The full L1 surface is large (multi-MU,
   security, PubSub, structured types, certificate exchange). The
   first landing PR has to be small enough to review, so the ADR
   defines what stays in and what waits.

## Decision

### 1. Library: `async-opcua` (Rust) for the thin slice

Use the [`async-opcua`](https://github.com/freeopcua/async-opcua)
crate (formerly `locka99/opcua`) for both server and client
bindings.

**Trade-off matrix** (assessed 2026-05-22):

| Axis                       | `open62541` (C, via FFI)            | `async-opcua` (Rust)              |
| -------------------------- | ----------------------------------- | --------------------------------- |
| Build surface              | CMake + C toolchain + Cargo         | Cargo only                        |
| `unsafe` in our crate      | Yes (FFI shim)                      | No (`forbid(unsafe_code)` holds)  |
| OPC UA Client/Server       | Mature (production deployments)     | Mature (production deployments)   |
| OPC UA PubSub UDP          | Supported (MPLv2)                   | Partial; not validated at scale   |
| Companion-spec mappings    | Manual (general-purpose API)        | Manual (general-purpose API)      |
| Bench-test ergonomics      | Mixed (link against C, allocator)   | Native (`cargo test`)             |
| Async / tokio integration  | Manual                              | Native (`tokio` runtime)          |
| Workspace alignment        | New foreign build system            | Same crate ecosystem              |
| MSRV impact                | None                                | Tracks 1.75 (workspace MSRV)      |

For the Phase 4 thin slice (Client/Server, one MU, anonymous, no
encryption) `async-opcua` is strictly better: it does not change
the workspace build, it preserves the no-`unsafe` invariant, and
the test surface stays inside `cargo test`. The capability we
sacrifice — battle-tested PubSub UDP — is not needed until at
least Phase 5 (no current consumer demands it).

**This is a silent supersede of IP v0.2 §3.7** as documented in
the front matter. The IP change is intentional and recorded
here; it does not require an IP v0.3 patch note because the IP
already accepts ADR-level supersedes in §0 (and the choice
remains internal to WBS-3.7's implementation, not to the SDD).

If Phase 5 surfaces a PubSub requirement that `async-opcua`
cannot meet, a follow-on ADR (likely numbered 0023+) will revisit
this decision; the AddressSpace mapping locked below is library-
neutral and survives a swap.

### 2. AddressSpace: OPC 10040 IEC 61850 mapping, deterministic node IDs

The server's information model is built from the channel registry
(SDD §7.2) at startup. The tree is rooted at the standard
`/Objects/Substations` folder per OPC 10040 §6.2 and grows one
sub-folder per MU plus one object per channel:

```text
Objects/
└── Substations/
    └── <SubstationName>/                       (Folder)
        └── <MU_svID>/                          (Object, type: LogicalDevice)
            ├── ChannelRegistry/                (Folder)
            │   ├── Ch00_Va/                    (Object, type: AnalogValue)
            │   │   ├── instMag.i               (Variable, Int32, raw Q-value)
            │   │   ├── instMag.f               (Variable, Float, calibrated)
            │   │   ├── q                       (Variable, UInt16, IEC 61850 quality)
            │   │   ├── t                       (Variable, UtcTime, sample stamp)
            │   │   └── tick_id                 (Variable, UInt64, alignment tick)
            │   ├── Ch01_Vb/  …                 (same shape)
            │   └── …
            └── TickStatus/                     (Object, type: DiagnosticsFolder)
                ├── last_tick_id                (Variable, UInt64)
                ├── last_ts_utc_ns              (Variable, UInt64)
                └── n_channels                  (Variable, UInt16)
```

**Node ID scheme:** string-based, deterministic, namespace `2`
(the application-defined namespace; namespace `0` is reserved for
the standard, `1` for the server URI):

| Object                  | Node ID                                              |
| ----------------------- | ---------------------------------------------------- |
| Substation folder       | `s=Substations.<SubstationName>`                     |
| MU object               | `s=Substations.<SubstationName>.<MU_svID>`           |
| Channel object          | `s=…<MU_svID>.Ch{:02}_{name}`                        |
| `instMag.i` variable    | `s=…Ch{:02}.instMag.i`                               |
| `instMag.f` variable    | `s=…Ch{:02}.instMag.f`                               |
| `q` variable            | `s=…Ch{:02}.q`                                       |
| `t` variable            | `s=…Ch{:02}.t`                                       |
| `tick_id` variable      | `s=…Ch{:02}.tick_id`                                 |
| `TickStatus.last_tick`  | `s=…<MU_svID>.TickStatus.last_tick_id`               |

Reasoning: string IDs render readably in UA Expert and other
generic browsers; they are stable across daemon restarts
(numeric IDs would re-shuffle if the channel registry is
reordered); the prefix structure mirrors the folder hierarchy so
operators can predict any node's address. The dot-separated form
matches IEC 61850 logical-node naming conventions.

### 3. Timestamp and quality mapping

**Timestamp.** Every value's `DataValue.SourceTimestamp` is set
from `TickRecord.ts_utc_ns` translated to OPC UA's 100-ns
`DateTime` epoch (1601-01-01 UTC). `ServerTimestamp` is the
moment the value was published to the OPC UA stack — useful for
diagnosing publish-side jitter.

**Quality.** IEC 61850 `q` (u8) maps to OPC UA `StatusCode` per
OPC 10040 §6.3, simplified to the subset we use today:

| IEC 61850 `q` bit | OPC UA StatusCode                |
| ----------------- | -------------------------------- |
| 00 (good)         | `Good` (0x00000000)              |
| validity:invalid  | `Bad_NoData` (0x80AB0000)        |
| validity:questionable | `Uncertain_LastUsableValue` (0x40900000) |
| overflow set      | `Bad_OutOfRange` (0x803B0000)    |
| failure set       | `Bad_NoCommunication` (0x80B10000) |

The mapping is bidirectional only for the subset above; bits we
do not consume (`oldData`, `inaccurate`, …) leave the StatusCode
at `Good` unless another bit overrides it. The full table lives
in `crates/svdc-opcua/src/quality.rs` so it is testable in
isolation.

**Sample origin.** `Sample.origin` (LIVE / INTERPOLATED /
QSE_ESTIMATED) maps onto the OPC UA `StatusCode` substatus bits
per OPC 10040 §6.4: `LIVE` → no override; `INTERPOLATED` →
`Uncertain_InterpolatedValue`; `QSE_ESTIMATED` →
`Uncertain_LastUsableValue` with custom diagnostic info.

### 4. Subscription mechanism: standard MonitoredItem, queue depth 1

Clients subscribe via the standard OPC UA `CreateSubscription` +
`CreateMonitoredItems` flow. Server defaults:

- **Default publishing interval:** 100 ms (10 Hz, configurable
  per subscription). SCADA HMIs typically render at 10–20 Hz; the
  4800 Hz tick rate is intentionally not exposed at L1 — L0 is
  the right place for sub-ms consumers.
- **Queue size:** 1 (latest-value-only). SVDC is real-time; if a
  client misses a publish window the next tick is the right
  recovery, not a queued backlog.
- **Sampling interval:** equal to or larger than the publish
  interval; the server uses the configured tick rate divided
  by the publish-interval ratio as the effective sample.
- **No deadband.** L1 publishes every published tick verbatim;
  client-side filtering is the consumer's job.

### 5. Security and authentication (initial slice)

The first landing PR is **deliberately insecure** and exists for
bench verification only:

- **Security policy:** `None` (no signing, no encryption)
- **Message security:** `None`
- **User token:** `Anonymous`
- **Certificate store:** disabled

The runbook MUST flag this clearly and refuse to run with
`--bind` on a non-loopback address unless an explicit
`--allow-insecure-bind` flag is passed. (This is a code-level
guard, not a documentation guard; see PR K test surface.)

**Phase 5 upgrade** (deferred to a later ADR):

- `Basic256Sha256` security policy (sign + encrypt)
- `UserName` token with operational-state-backed credential store
- Certificate exchange and revocation list checking
- Optional Kerberos / X.509 mutual auth for SCADA integration

### 6. PubSub: out of scope for now

IEC 61850-90-12 / OPC 10040 PubSub UDP multicast is the obvious
next step (it would let L1 push samples without the
Subscription/MonitoredItem round-trip), but no current consumer
needs it. PubSub is the one capability where `open62541` is
materially ahead of `async-opcua`, so a Phase 5 PubSub
requirement would also be the trigger to revisit decision (1).

### 7. Crate layout

```text
crates/svdc-opcua/
├── Cargo.toml          # depends on svdc-core, svdc-subscribe, async-opcua
├── src/
│   ├── lib.rs          # OpcuaServer::start(addr, subscriber, registry)
│   ├── address_space.rs # ChannelRegistry → NodeId tree builder
│   ├── quality.rs      # IEC 61850 q → OPC UA StatusCode
│   ├── timestamp.rs    # ts_utc_ns → DateTime
│   └── publisher.rs    # background task: read_since() → update nodes
└── tests/
    └── round_trip.rs   # in-process client + server smoke test
```

Wiring:

- `svdc-bin` gains `--enable-opcua <bind-addr>` (default
  `127.0.0.1:4840` when the flag is present without a value);
  starting it spawns `OpcuaServer::start(...)` with the same
  `InProcessSubscriber` the L0 demo uses (one `TickBuffer`, two
  parallel readers).
- `DataPipeline` gains L1 counters mirroring the L0 atomics:
  `l1_opcua_active`, `l1_opcua_last_tick_id`,
  `l1_opcua_total_publishes`, `l1_opcua_client_count`.
- `/north/L1` transitions from `Planned (Phase 4)` to
  `Wired · running` / `Wired · not started`, surfacing the
  client count and the cursor per the PR I pattern.

### 8. Thin-slice scope (PR K candidate)

The first L1 server PR ships:

- One substation folder (built from `OperationalState.substation`
  or a hard-coded `DEFAULT_SUBSTATION` if unset)
- One MU object, populated from the first MU observed in the
  channel registry
- Eight channel objects (the SDD §7.1 reference layout: Va Vb Vc
  Vn Ia Ib Ic In) with the full five-variable shape from §2
- Default bind `127.0.0.1:4840`, anonymous, no security
- 10 Hz publish (matches default `100 ms` subscription interval;
  achieved by sampling every 480th tick at 4800 Hz)
- UA Expert (or equivalent generic OPC UA client) verification in
  the runbook with a screenshot

Out of the first PR:

- Multi-MU federation (one MU per OPC UA folder; first MU only)
- Dynamic registry reload (SCD changes ignored after server start;
  daemon restart required, same as today)
- Certificate exchange / signed messages
- L1 client simulator (Northbound L1 simulator `svdc-l1-opcua-client`
  in ADR-0016 §6 lands in a follow-up PR per the PR plan below)
- C ABI binding for the OPC UA server (the C ABI is L0-only by
  ADR-0010)

### 9. Test surface

- **Unit:** `address_space::tests::registry_builds_eight_channels`,
  `quality::tests::iec61850_to_status_code_table`,
  `timestamp::tests::ns_to_opcua_datetime_round_trips`.
- **Integration:** `tests/round_trip.rs` — start the server on a
  loopback port; subscribe via the same `async-opcua` crate as a
  client; push ten synthetic ticks via the
  `InProcessSubscriber`'s `TickBuffer::push`; assert the client
  observes monotonic `SourceTimestamp` and matching `tick_id`.
- **Manual:** runbook walks through opening UA Expert, pointing
  it at `opc.tcp://127.0.0.1:4840`, and browsing to one of the
  channel variables. Acceptance: the `instMag.i` variable updates
  visibly at ~10 Hz.

### 10. Operator UI

`/north/L1` mirrors the L0 pattern from PR I:

- Status badge: `Wired · running` when the OPC UA server has
  bound the socket; `Wired · not started` otherwise.
- Live cell shows `clients=N · last_tick_id=K · published=M`
  drawn from the new `DataPipeline` atomics.
- Detail page shows `Bind address`, `Last tick published`,
  `Total publishes`, `Client count`, `Security`, with a
  `How to enable` card showing the cargo command when not
  running.
- No enable/disable button — same justification as L0: the
  feature is gated by daemon startup flag, and toggling at
  runtime is not in the thin slice.

## Consequences

- IP §3.7's `open62541` library choice is silently superseded for
  the Phase 4 slice. The PubSub work item moves to Phase 5
  contingent on a real consumer asking for it.
- AddressSpace builder is a public surface of `svdc-opcua`:
  channel registry changes propagate into the OPC UA model
  through a single function call, so SCD evolution is unit-
  testable.
- Quality and timestamp mappings are locked. Downstream observers
  (SCADA HMIs) can rely on the table in §3 for alarm logic.
- The "first MU only" simplification will surface as a missing
  feature the moment a deployment has more than one MU — flagging
  it in the runbook and on `/north/L1` is a follow-up PR, not a
  spec change.
- The `--allow-insecure-bind` guard means a future operator
  cannot accidentally expose the SVDC's OPC UA server to a public
  interface in the no-security configuration; the guard ships
  with the first PR.

## Cumulative follow-up PR plan (extends ADR-0016 §171)

| PR | Title                                                                          | Phase | Status   |
| -- | ------------------------------------------------------------------------------ | ----- | -------- |
| J  | **ADR-0017: OPC UA address space + library decision** (this ADR, docs-only)    | 4 (planning) | **this PR** |
| K  | `svdc-opcua` crate scaffold + AddressSpace builder + unit tests                | 4     | planned  |
| L  | `--enable-opcua` daemon flag + `/north/L1` wiring (mirrors PR H/PR I pattern)  | 4     | planned  |
| M  | `svdc-l1-opcua-client` reference consumer per ADR-0016 §6                       | 4     | planned  |
| N+ | Multi-MU federation, security policies, PubSub — separate ADRs                  | 5+    | future   |

## Out of scope

- OPC UA PubSub UDP multicast (revisited at Phase 5 if a
  consumer asks)
- Encryption, signing, and `UserName` authentication
- Dynamic AddressSpace reload on SCD change
- Multi-MU federation in a single OPC UA folder
- IEC 61850 GOOSE-over-OPC UA mapping (not part of OPC 10040)
- The L1 client simulator (lands as its own PR per ADR-0016)
- Kerberos / Active Directory integration

## References

- IEC 62541 (OPC UA Unified Architecture)
- OPC 10040 (IEC 61850 ↔ OPC UA companion specification)
- IEC 61850-90-1 (OPC UA application of OPC 10040 for PoCom)
- [async-opcua](https://github.com/freeopcua/async-opcua) — MIT
- [open62541](https://www.open62541.org/) — MPL-2.0
- SVDC SDD v0.1 §7 (TickRecord), §7.2 (channel registry), §8.2
  (northbound subscriber API)
- SVDC IP v0.2 §1.4, §3.7 (WBS-3.7 OPC UA server)
- [[0010-subscriber-api]] — the L0 surface this server reads from
- [[0016-northbound-simulators]] — the L1 simulator client
  promised here lives in PR M
- CIGRE 2024 Paper ID 10427 §3 (northbound consumer requirements)
