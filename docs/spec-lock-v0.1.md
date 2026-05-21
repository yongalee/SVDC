# Spec Lock — SVDC v0.1

> **Status:** *closed (provisional)* — accepted SSIEC defaults on 2026-05-21
> pending later Prof. Meliopoulos review.
>
> See ADR-0006 for the rationale behind proceeding on defaults without
> synchronous professor sign-off. The proposal document
> (`docs/spec-lock-proposal-v0.1.md`) remains available for the professor
> to ratify or revise when reachable; any revision will be recorded as
> ADR-XXXX and back-applied to code.

## How to use this document

For each question below, SSIEC proposed a recommended default. With this
revision (2026-05-21) all six defaults are **provisionally accepted**
without synchronous professor review, per ADR-0006. The "Professor
response" block under each question is held open for later revision.

After ratification (or revision) by the professor, update each block in
place, change the `Status` above to `closed`, and tag the repository as
`spec-lock-v0.1`. The current provisional state is tagged
`spec-lock-v0.1-provisional`.

---

## Q1 — Default value of `N` (records per circular buffer)

**SDD reference:** §3 FR-3; §15(1)

**SSIEC recommended default:** `N = 64 power-frequency cycles × 80 SPC = 5,120 records`
(≈ 1.07 s of buffered data at 60 Hz).

**Rationale:** Provides ≥1 second of replay window for Phasor Computation Module,
Transient Recorder, and QSE write-back lookups. At ~256 bytes per record, total
memory per CB is ~1.3 MB; dual CB is ~2.6 MB — fits L2/L3 cache on typical node
hardware.

**Resolution (provisional, 2026-05-21):** ✅ accept default. Phase 1 work
proceeds with `N = 5,120`.

**Professor response:**

> _(deferred — awaiting synchronous review)_

---

## Q2 — Canonical sample rate

**SDD reference:** §15(2)

**SSIEC recommended default:** Support both 80 SPC and 256 SPC concurrently
(per-MU configurable in SCD). Default reference deployment uses 80 SPC; 256 SPC
available for measurement-class MUs.

**Rationale:** Different MU vendors and use cases assume different rates;
forcing one excludes legitimate hardware. Per-MU configuration is no extra
implementation cost given the alignment design.

**Resolution (provisional, 2026-05-21):** ✅ accept default. `ssiec-sv-publisher`
default rate stays at 80 SPC × 60 Hz = 4,800 Hz; the per-MU SCD overrides for
256 SPC merging units.

**Professor response:**

> _(deferred — awaiting synchronous review)_

---

## Q3 — Interpolation order

**SDD reference:** §15(3)

**SSIEC recommended default:** Linear interpolation in v0.1; expose interpolation
order as a per-channel configuration so quadratic (matching Standard-PMU) can be
added in v0.2 without breaking the API.

**Rationale:** Linear is sufficient for ≤5% gap rate per acceptance criterion.
Quadratic costs more CPU and only matters in higher-loss environments.

**Resolution (provisional, 2026-05-21):** ✅ accept default. M3 (Interpolation
module) ships with linear in v0.1; quadratic is a per-channel option behind a
config flag.

**Professor response:**

> _(deferred — awaiting synchronous review)_

---

## Q4 — Write-back authentication / authorization

**SDD reference:** FR-6; §15(4)

**SSIEC recommended default:** v0.1: process-level isolation only (write-back
endpoint listens on a UDS owned by `svdc` user, peer-credential check restricts
to the QSE process UID). v0.2: add an HMAC-signed token option for the QSE to
include in each correction batch.

**Rationale:** UDS peer-cred is the minimum-viable authorization that prevents
accidental cross-process writes; HMAC adds defense-in-depth for environments
where multiple processes share the UID.

**Resolution (provisional, 2026-05-21):** ✅ accept default. v0.1 ships UDS
peer-cred; v0.2 adds HMAC. Console write-back UI (WBS-9.6) reflects this.

**Professor response:**

> _(deferred — awaiting synchronous review)_

---

## Q5 — Reference SCD schema

**SDD reference:** §15(5)

**SSIEC recommended default:** SSIEC defines a minimal SCD schema as part of M0
based on the channels described in the paper (per-MU: 4 voltage + 4 current
channels). The schema is included in the repo as `docs/sample-scd.cid` and
documented in the SDD revision. If the Georgia Tech team has an existing schema
from the US partner sites, SSIEC will adopt it instead.

**Resolution (provisional, 2026-05-21):** ✅ accept default. SSIEC defines the
minimal schema; `docs/sample-scd.cid` lands as part of M0. If a Georgia Tech
schema arrives later, we migrate; the migration is in-scope as a follow-up.

**Professor response:**

> _(deferred — awaiting synchronous review; a Georgia Tech schema, if
> available, would supersede the SSIEC default)_

---

## Q6 — linuxptp clock-holdover reporting

**SDD reference:** FR-8; §15(6)

**SSIEC recommended default:** SVDC polls `linuxptp`'s `pmc` management interface
every 1 s and exposes three fields in `/health`: `ptp_offset_ns`, `ptp_mean_path_delay_ns`,
`ptp_holdover_state` (one of `LOCKED | HOLDOVER | FREE_RUNNING`). The holdover
state is derived from `pmc` clockClass field (clockClass 7 = holdover; > 7 = free-run).

**Resolution (provisional, 2026-05-21):** ✅ accept default. Dashboard tile
(WBS-9.2) and Monitoring chart (WBS-9.5) consume these three fields.

**Professor response:**

> _(deferred — awaiting synchronous review)_

---

## Sign-off

- **SSIEC lead (provisional):** SVDC implementation team, 2026-05-21
- **Prof. Meliopoulos:** _(deferred — see ADR-0006)_
- **Repository tag:** `spec-lock-v0.1-provisional` (applied 2026-05-21).
  Final `spec-lock-v0.1` tag is applied only after professor ratification.
