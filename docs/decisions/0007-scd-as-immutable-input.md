# ADR-0007 — SCD as immutable input; operational state lives elsewhere

- **Status:** accepted
- **Date:** 2026-05-21
- **Deciders:** SSIEC SVDC team

## Context

During WBS-9.6a implementation a question surfaced: when the operator
loads an SCD into the Operator Console, should the MU detail page
expose all the SCL-derived fields as **editable**, and should the SVDC
**write changes back** into the SCL/SCD file?

This is a load-bearing decision that touches:

- The IEC 61850-6 engineering workflow (which tool owns which file).
- Multi-IED interoperability (other IEDs are configured against the
  same SCD; one node editing it silently is a substation-safety risk).
- The SVDC's own operational tuning surface (calibration, subscription
  enable/disable, threshold overrides).
- Antigravity's WBS-9.6b form work (so they know which fields are
  editable and which are read-only).

The IEC 61850 standard answers this very clearly through the SCL file
type system (61850-6 §4 and §6):

| File | Authoring tool | Audience | Mutability from a node's POV |
|---|---|---|---|
| ICD | IED vendor | Sub-system engineer | n/a |
| SSD | System designer | SCT | n/a |
| **SCD** | **SCT** (System Configuration Tool) | All IEDs in the substation | **read-only** |
| CID | SCT → IED | One specific IED | read-only after import |
| IID | IED at commissioning | The SCT | written by the IED, *not* by other nodes |

The SCT is the canonical SCD owner. The SVDC is one of the SCD's many
consumers and **never** writes back.

## Decision

The SVDC project draws a hard line between two kinds of state:

### Class A — SCL/SCD-derived (read-only in SVDC)

Lives in `crates/svdc-console/src/scd::registry::ChannelRegistry`,
populated from the SCD on upload (`POST /api/config/scd`), the
embedded sample (`POST /api/config/scd/sample`), or one-MU manual
register (`POST /api/config/mus`). Fields per `MergingUnit`:

- `id` (IED name)
- `mac` (Ethernet multicast MAC from `ConnectedAP/Address/P[MAC-Address]`)
- `appid` (from `P[APPID]`)
- `sv_id` (from `SampledValueControl/smvID`)
- `smp_rate` (from `SampledValueControl/smpRate`)
- `channels[]` (from `DataSet/FCDA[]`)

These fields are **not** editable from the Console. The MU detail page
renders them in the **"From SCD"** panel. Changes require:

1. Editing the SCD in an SCT (vendor product, outside the SVDC).
2. Re-uploading the modified SCD.

Manual MU registration (`POST /api/config/mus`) is the one exception
that bypasses SCT round-trip — it exists for lab / ad-hoc test and
*creates* a `MergingUnit` directly. Once created, the same read-only
rule applies; further edits require re-registering the MU.

### Class B — SVDC-local operational state (editable)

Lives in `crates/svdc-console/src/operational::OperationalState`. In
v0.1 these are:

- Per-channel **calibration triple** (`gain`, `offset`, `unit_scale`)
  applied as `corrected = (raw * gain + offset) * unit_scale`.
- (Future) Per-MU **subscription enable/disable** flag.
- (Future) Per-channel **threshold / alarm** values.
- (Future) Per-layer (L0/L1/L2/L3) **enable** flag, currently held by
  `routes::northbound::NorthboundState` — will be moved here when the
  state is unified.

These are operator-editable on the MU detail / Northbound pages and
on `POST /api/config/calibration/:mu_id/:idx`. They are persisted in
the SVDC's own config file (`/etc/svdc/operational.toml` in
production — Phase 4 file binding still pending). They **never**
modify the SCD.

### What this rules out

- No "Save back to SCD" button anywhere in the Console.
- No `crates/svdc-console/src/scd/writer.rs`. The crate name says
  "parser" because there is no writer.
- No mutation API on `ChannelRegistry` other than `replace(Vec<MU>)`
  (which performs a wholesale swap on SCD re-upload, not a field edit).

### Why this is also the right product call

- **Multi-IED safety.** Other IEDs in the substation are commissioned
  with the same SCD. If the SVDC silently mutated it, the next IED
  re-flash from "the SCD" would pick up SVDC's local edits as if they
  were system-engineered choices. This is the kind of failure mode the
  IEC 61850-6 separation was designed to prevent.
- **Audit trail.** Calibration changes are SVDC events — they have an
  operator, a timestamp, a "previous value", and they fit cleanly in
  the audit log (per UI Doc §2.5). SCD changes are SCT events — they
  belong in the SCT's revision log, not in the SVDC's.
- **Tool scope.** Implementing an SCD editor properly is on the same
  order of magnitude as the whole rest of the SVDC. There are mature
  commercial products (ABB IET600, Helinks, OMICRON SCT, PVR-SCT) and
  open-source efforts (openscd). Re-implementing one would multiply
  the SVDC scope without compounding the value.

## Consequences

- The MU detail page presents two panels (From SCD / Operational)
  that are visually distinct: a static table vs. an editable form.
  This is the right operator affordance — read-only data should look
  read-only.
- `Operational state` is the natural home for any future tuning
  parameter that doesn't belong in the SCD. The pattern in
  `operational/mod.rs` (typed key → typed value, `RwLock`, snapshot()
  for audit) is reusable for subscription flags, thresholds, etc.
- Persistence of `OperationalState` to a TOML file ships in Phase 4
  alongside the operator manual. Until then it is in-memory only;
  restart loses operator edits.
- Antigravity's WBS-9.6b (Config form refinement) targets the upload
  + sample + manual-register endpoints, never an "edit SCD field"
  endpoint, because no such endpoint exists.

## Alternatives considered

- **Allow SVDC to edit SCD fields directly.** Rejected — see
  multi-IED safety above. Also makes the SVDC depend on writing valid
  SCL, which is a much bigger task than parsing.
- **Mirror everything into the SCD on operator edit (calibration too).**
  Rejected — calibration is not an SCL concept. SCL has
  `MergingUnit/Server/LDevice/...` parameters, not calibration triples.
  Stuffing calibration into a non-standard SCL extension would break
  interop with every other consumer of the same SCD.
- **Keep all state in a single combined registry.** Rejected — makes
  it visually unclear in the UI which fields are SCT-owned vs SVDC-
  owned. The two-table layout on the MU detail page is the right
  affordance, and that requires the underlying separation.

## References

- IEC 61850-6:2009 §4–6 — SCL file types and engineering workflow.
- SDD §6 M4 — Calibration module (NFR-9).
- UI Doc §1.1 — "Commission MUs" operator workflow.
- UI Doc §2.5 — "Read-mostly with explicit write paths".
- ADR-0006 — provisional spec-lock acceptance (Q5: SSIEC-default SCD
  schema; this ADR refines what happens to the schema *after* upload).
- PR #35 — implementation of this separation.
- Antigravity's earlier WBS-9.6b expectations: this ADR is the
  upstream guidance.
