# ADR-0006 — Provisional acceptance of spec-lock defaults

- **Status:** accepted
- **Date:** 2026-05-21
- **Deciders:** SSIEC SVDC team

## Context

`docs/spec-lock-v0.1.md` records six open questions (Q1–Q6) in SDD v0.1
§15 that require Prof. Meliopoulos's sign-off before Phase 1 (WBS-2 Core
Data Plane) can begin. The proposal one-pager
(`docs/spec-lock-proposal-v0.1.md`) is ready to send for review.

The professor is not synchronously reachable in the current window. The
SVDC schedule has Phase 1 starting in Phase 3 of the IP (W6–7), which we
do not want to slip on a review that may take days or weeks.

All six SSIEC-recommended defaults are technically defensible:

- They are within the bounds of the standards (IEC 61850-9-2 LE,
  IEEE 1588 / IEC 61588).
- They match the prevailing US utility deployment practice that the a²SDP
  programme targets.
- They make decisions that are reversible at modest cost should the
  professor revise: `N`, sample rate, interpolation order, write-back
  auth model, SCD schema source, and PTP holdover field set are all
  values consumed at a small number of well-isolated points in the code.

## Decision

We provisionally close Gate G0 by accepting all six SSIEC-recommended
defaults as the working values for Phase 1 onward.

Concretely:

| Q | Provisional value |
|---|---|
| Q1 (`N`) | `N = 5,120 records` (64 cycles × 80 SPC) |
| Q2 (sample rate) | Both 80 SPC and 256 SPC supported, default 80 SPC, per-MU configurable |
| Q3 (interpolation order) | Linear in v0.1; quadratic as per-channel config option for v0.2 |
| Q4 (write-back auth) | UDS peer-cred in v0.1; HMAC token added in v0.2 |
| Q5 (SCD schema) | SSIEC-defined minimal schema lands as `docs/sample-scd.cid` in M0; superseded by a Georgia Tech schema if/when one arrives |
| Q6 (PTP holdover) | 1 Hz `pmc` poll; three `/health` fields: `ptp_offset_ns`, `ptp_mean_path_delay_ns`, `ptp_holdover_state` (derived from `clockClass`) |

These resolutions are recorded inline in each question of
`docs/spec-lock-v0.1.md`, under a new "Resolution (provisional,
2026-05-21)" block.

The repository is tagged **`spec-lock-v0.1-provisional`** to mark this
state. The originally planned `spec-lock-v0.1` tag is held back for
genuine post-review sign-off.

The proposal document (`docs/spec-lock-proposal-v0.1.md`) is unchanged
and remains available to send to the professor when reachable.

### What changes if the professor revises

Each question maps to a small number of well-isolated code locations.
If a value comes back different from the provisional default, the
revision lands as:

1. A new ADR (ADR-0007 onward) recording the change and rationale.
2. An update to `docs/spec-lock-v0.1.md` Resolution blocks with the
   professor's response verbatim.
3. Code changes in the affected modules — for Q1 / Q2 / Q3 these are
   compile-time or config-file constants; for Q4 they're isolated to
   the API surface in `crates/svdc-api`; for Q5 they're isolated to
   the SCD parser in `crates/svdc-console` and `crates/svdc-ingress`;
   for Q6 they're isolated to the health endpoint formatter.
4. Re-tag as `spec-lock-v0.1` (without the `-provisional` suffix).

None of these require a Phase rollback. The cost of provisional
acceptance is bounded.

## Consequences

- **Phase 1 (WBS-2) is unblocked.** SVDC implementation work can begin
  on Core Data Plane crates against the values listed in the table
  above.
- **No false claim of professor sign-off.** The tag is
  `spec-lock-v0.1-provisional`, not `spec-lock-v0.1`. The proposal
  document is preserved unmodified. `CLAUDE.md` Phase 0 checklist
  reflects the provisional state, not a full close of Gate G0.
- **Forward compatibility is the test of these defaults.** If any code
  module hard-codes a value such that the professor's revision would
  require a substantial rewrite, that is a code-quality problem we catch
  in code review, not a spec problem.

## Alternatives considered

- **Block Phase 1 until the professor is reachable.** Rejected — the
  schedule cost outweighs the value of synchronous sign-off given how
  reversible the defaults are.
- **Skip the spec-lock document entirely.** Rejected — the questions
  document real ambiguities and the answers (even provisional) must be
  recorded somewhere durable. The Console (WBS-9) and the SV publisher
  (WBS-6) both depend on at least Q2 and Q6 being decided.
- **Tag as `spec-lock-v0.1` immediately without the `-provisional`
  suffix.** Rejected — falsifies the sign-off record and would have to
  be retconned later.

## References

- `docs/spec-lock-v0.1.md` — updated with provisional resolution blocks.
- `docs/spec-lock-proposal-v0.1.md` — unchanged proposal one-pager.
- `docs/SVDC_Design_Document_v0.1.html` §15 — original open questions.
- ADR-0001 — dual-agent workflow (ADRs as durable record).
- Issue #3 — Gate G0 review (closed with comment pointing here).
