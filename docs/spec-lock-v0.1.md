# Spec Lock — SVDC v0.1

> **Status:** *open — awaiting Prof. Meliopoulos responses*
>
> This document captures the resolutions to the six open questions in
> `SVDC_Design_Document_v0.1.html` §15. Phase 1 (WBS-2 Core Data Plane)
> does not start until this document is filled in and committed.

## How to use this document

For each question below, SSIEC proposes a recommended default. The professor's
response is expected as one of:

- ✅ accept default
- ✏️ accept with modification (note the modification)
- ❌ reject default (provide alternative)

After resolution, change the `Status` above to `closed`, set the date and
sign-off line at the bottom, and tag the repo as `spec-lock-v0.1`.

---

## Q1 — Default value of `N` (records per circular buffer)

**SDD reference:** §3 FR-3; §15(1)

**SSIEC recommended default:** `N = 64 power-frequency cycles × 80 SPC = 5,120 records`
(≈ 1.07 s of buffered data at 60 Hz).

**Rationale:** Provides ≥1 second of replay window for Phasor Computation Module,
Transient Recorder, and QSE write-back lookups. At ~256 bytes per record, total
memory per CB is ~1.3 MB; dual CB is ~2.6 MB — fits L2/L3 cache on typical node
hardware.

**Professor response:**

> _(awaiting)_

---

## Q2 — Canonical sample rate

**SDD reference:** §15(2)

**SSIEC recommended default:** Support both 80 SPC and 256 SPC concurrently
(per-MU configurable in SCD). Default reference deployment uses 80 SPC; 256 SPC
available for measurement-class MUs.

**Rationale:** Different MU vendors and use cases assume different rates;
forcing one excludes legitimate hardware. Per-MU configuration is no extra
implementation cost given the alignment design.

**Professor response:**

> _(awaiting)_

---

## Q3 — Interpolation order

**SDD reference:** §15(3)

**SSIEC recommended default:** Linear interpolation in v0.1; expose interpolation
order as a per-channel configuration so quadratic (matching Standard-PMU) can be
added in v0.2 without breaking the API.

**Rationale:** Linear is sufficient for ≤5% gap rate per acceptance criterion.
Quadratic costs more CPU and only matters in higher-loss environments.

**Professor response:**

> _(awaiting)_

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

**Professor response:**

> _(awaiting)_

---

## Q5 — Reference SCD schema

**SDD reference:** §15(5)

**SSIEC recommended default:** SSIEC defines a minimal SCD schema as part of M0
based on the channels described in the paper (per-MU: 4 voltage + 4 current
channels). The schema is included in the repo as `docs/sample-scd.cid` and
documented in the SDD revision. If the Georgia Tech team has an existing schema
from the US partner sites, SSIEC will adopt it instead.

**Professor response:**

> _(awaiting)_

---

## Q6 — linuxptp clock-holdover reporting

**SDD reference:** FR-8; §15(6)

**SSIEC recommended default:** SVDC polls `linuxptp`'s `pmc` management interface
every 1 s and exposes three fields in `/health`: `ptp_offset_ns`, `ptp_mean_path_delay_ns`,
`ptp_holdover_state` (one of `LOCKED | HOLDOVER | FREE_RUNNING`). The holdover
state is derived from `pmc` clockClass field (clockClass 7 = holdover; > 7 = free-run).

**Professor response:**

> _(awaiting)_

---

## Sign-off

- **SSIEC lead:** _(name, date)_
- **Prof. Meliopoulos:** _(name, date)_
- **Repository tag:** `spec-lock-v0.1` _(applied when closed)_
