# SVDC Spec-Lock Proposal v0.1 — Sign-off Request

| | |
|---|---|
| **To** | Prof. A. P. Sakis Meliopoulos · Georgia Institute of Technology |
| **From** | Shinsung Industrial Electric Co., Ltd. (SSIEC) · SVDC implementation team |
| **Date** | 2026-05-21 |
| **Re** | Phase 0 spec-lock — six open questions in SVDC SDD v0.1 §15 |
| **Action requested** | Per-question sign-off so Phase 1 (Core Data Plane) can begin |

---

## How to respond

For each of the six questions below, please tick **one** of:

- **✅ Accept default** — proceed with SSIEC's proposed value
- **✏️ Accept with modification** — note the modification in the space provided
- **❌ Reject default** — note your preferred alternative

Reply via email or by editing this document and returning. After your
response we update `docs/spec-lock-v0.1.md` accordingly and tag the
repository as `spec-lock-v0.1`, which clears **Gate G0** and unblocks
Phase 1.

Full rationale and references for each answer are in
`docs/spec-lock-v0.1.md` in the SVDC repository.

---

## Q1 — Circular-buffer size `N`

**Default proposed:** `N = 64 cycles × 80 SPC = 5,120 records` (≈ 1.07 s
at 60 Hz). Memory per CB ≈ 1.3 MB; dual CB ≈ 2.6 MB. Fits L2/L3 cache.

**Why this default:** Provides ≥1 second of replay window for Phasor
Computation Module, Transient Recorder, and QSE write-back lookups.

**Response:**

- [ ] ✅ Accept default
- [ ] ✏️ Accept with modification: `N = ________________`
- [ ] ❌ Reject default; alternative: ________________

---

## Q2 — Canonical sample rate

**Default proposed:** Support both **80 SPC (protection)** and
**256 SPC (measurement)** concurrently, per-MU configurable in the SCD.
Reference deployment defaults to 80 SPC.

**Why this default:** Different MU vendors and use cases assume
different rates; forcing one excludes legitimate hardware. Per-MU
configuration is no extra implementation cost given the alignment
design.

**Response:**

- [ ] ✅ Accept default
- [ ] ✏️ Accept with modification: ________________
- [ ] ❌ Reject default; choose one rate only:
      - [ ] 80 SPC only
      - [ ] 256 SPC only

---

## Q3 — Interpolation order

**Default proposed:** **Linear** in v0.1. Interpolation order is exposed
as a per-channel configuration so **quadratic** (matching Standard-PMU)
can be added in v0.2 without breaking the API.

**Why this default:** Linear is sufficient for the ≤5 % gap-rate
acceptance criterion. Quadratic costs more CPU and only matters in
higher-loss environments.

**Response:**

- [ ] ✅ Accept default (linear in v0.1, quadratic option in v0.2)
- [ ] ✏️ Quadratic from v0.1 instead
- [ ] ❌ Reject; alternative: ________________

---

## Q4 — Write-back authorization model (QSE → SVDC)

**Default proposed:**

- **v0.1:** Process-level isolation only. Write-back endpoint listens on
  a Unix domain socket owned by the `svdc` user; peer-credential check
  restricts to the QSE process UID.
- **v0.2:** Add an HMAC-signed token option for environments where
  multiple processes share the UID.

**Why this default:** UDS peer-cred is the minimum-viable authorization
that prevents accidental cross-process writes. HMAC adds
defense-in-depth where needed.

**Response:**

- [ ] ✅ Accept default
- [ ] ✏️ Modify v0.1 model: ________________
- [ ] ❌ Reject; require API-level auth (HMAC or mTLS) from v0.1 onward

---

## Q5 — Reference SCD schema

**Default proposed:** SSIEC defines a minimal SCD schema as part of M0
based on the channels described in the paper (per MU: 4 voltage + 4
current channels). Shipped as `docs/sample-scd.cid`. **If your team
already has an SCD schema from a US partner site, we adopt that
instead.**

**Why this default:** Avoids inventing a schema if one already exists
in the deployment chain. Avoids blocking on a schema that doesn't.

**Response:**

- [ ] ✅ Accept default (SSIEC defines minimal SCD; will adopt yours if
        you share one)
- [ ] ✏️ Existing SCD attached / will be sent to SSIEC: ________________
- [ ] ❌ Reject; alternative: ________________

---

## Q6 — `linuxptp` clock-holdover reporting

**Default proposed:** SVDC polls `linuxptp`'s `pmc` management interface
every 1 s and exposes three fields on `/health`:

| Field | Source |
|---|---|
| `ptp_offset_ns` | `pmc` GET current data set |
| `ptp_mean_path_delay_ns` | `pmc` GET parent data set |
| `ptp_holdover_state` | derived from `pmc` `clockClass`: 7 = `HOLDOVER`, > 7 = `FREE_RUNNING`, otherwise `LOCKED` |

**Why this default:** `pmc` is the standard `linuxptp` management
client; `clockClass` is the canonical field. 1 Hz polling matches the
SCADA tile refresh rate.

**Response:**

- [ ] ✅ Accept default
- [ ] ✏️ Accept with modification (different fields / polling rate): ________________
- [ ] ❌ Reject; alternative: ________________

---

## Sign-off

| | |
|---|---|
| **SSIEC lead** | _____________________ , 2026-__-__ |
| **Prof. Meliopoulos** | _____________________ , 2026-__-__ |
| **Repository tag** | `spec-lock-v0.1` (applied by SSIEC after sign-off) |

Once you sign off on the six items above, SSIEC will:

1. Update `docs/spec-lock-v0.1.md` with your responses verbatim.
2. Commit and tag the repository as `spec-lock-v0.1`.
3. Begin Phase 1 (WBS-2 Core Data Plane) per the implementation plan.

Phase 1 is fully blocked on this sign-off; the team is currently
executing Phase 0 housekeeping and WBS-9 (Operator Console)
preparatory work — neither of which depends on these answers.

Thank you for your time. We are available for a brief synchronous
review if any of the six items warrant discussion rather than a
simple per-item decision.

---

*References*

- SVDC SDD v0.1 §15 — open questions in full
- `docs/spec-lock-v0.1.md` — extended rationale and revision history
- SVDC Implementation Plan v0.3 patch §"Spec-lock dependency"
- CIGRE 2024 Paper ID 10427 (Meliopoulos et al.) — `a²SDP` reference
