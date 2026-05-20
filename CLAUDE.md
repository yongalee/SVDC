# SVDC — Claude Code Project Context

## What this project is

A reference implementation of the **Sampled Value Data Concentrator (SVDC)** for the a²SDP
architecture described in Meliopoulos et al., *Protection and Control of Active Distribution
Systems*, CIGRE 2024 Paper ID 10427.

The SVDC is the central data-plane component of an a²SDP local node. It ingests IEC 61850-9-2
Sampled Value (SV) streams from multiple Merging Units, time-aligns them under PTP-disciplined
timestamps, buffers them in dual redundant circular buffers, and exposes a layered northbound
interface (L0 in-process · L1 OPC UA Server · L2 MQTT · L3 TimescaleDB sidecar) to all
node-local and operational applications.

## Who this is for

This software is built by **Shinsung Industrial Electric (SSIEC)** as a software contribution
to **Prof. A. P. Sakis Meliopoulos's a²SDP research programme at Georgia Tech**. The intended
operators are the Georgia Tech research team and their US partner utility deployments
(Southern Company, Avista, Dominion).

SSIEC is an external software contributor. The deliverable is the SVDC implementation itself,
packaged for installation and operation by the Georgia Tech team.

## Authoritative documents

Read these before doing anything substantive. They are the source of truth for scope, design,
and execution; this file is a pointer, not a replacement.

- `docs/SVDC_Design_Document_v0.1.html` — **the SDD**. Defines what the SVDC shall be:
  functional and non-functional requirements (FR/NFR), module decomposition (M1–M9),
  data model, external interfaces.
- `docs/SVDC_Implementation_Plan_v0.2.html` — **the IP**. Defines how it will be built:
  MECE WBS (8 categories, ~50 work items), 7 execution phases, Quality Gates (G0/G3/G6),
  effort estimates.

When the SDD and the IP disagree, the SDD wins. When either disagrees with the paper,
raise an issue and surface the disagreement to the professor; do not silently choose.

## Non-negotiable conventions

- **NFR-10 — English only.** All identifiers, comments, log messages, error messages,
  HTTP/JSON field names, configuration-file keys, commit messages, and documentation
  must be in English. No Korean. See `CONTRIBUTING.md`.
- **Rust stable.** Pin via `rust-toolchain.toml` (add in Phase 0). No nightly features.
- **Apache-2.0 license** on everything we publish.
- **No allocation on hot path.** The hot path is M1 (ingest) → M2 (align) → M3 (interpolate)
  → M4 (calibrate) → M5/M6 (CB write). Verified by `heaptrack`. NFR-4.
- **No mutex on hot path.** Coordination via release/acquire ordering on cursors and
  versioned snapshots. NFR-2, NFR-6.

## Current state: Phase 0 (Foundation and Spec Lock)

We are at the very beginning. This starter pack itself constitutes the initial commit.

Phase 0 task checklist (from IP §9.1):

- [x] WBS-1.1 Repository initialization (this starter)
- [x] WBS-1.2 Cargo workspace structure (skeleton in `crates/`)
- [x] WBS-1.5 Coding standards (`CONTRIBUTING.md`)
- [ ] WBS-1.3 Toolchain pin (`rust-toolchain.toml`) and dev container
- [ ] WBS-1.4 CI pipeline (GitHub Actions: fmt, clippy, test)
- [ ] WBS-1.6 Issue tracker setup (one issue per WBS item, labels by WBS code)
- [ ] WBS-6.1 (skeleton) `ssiec-sv-publisher` emits one valid SV packet
- [ ] **Spec-lock review session with Prof. Meliopoulos → Gate G0**

**Do not proceed past Gate G0 into Phase 1 work** until the six open questions in SDD §15 are
resolved with the professor and recorded in `docs/spec-lock-v0.1.md` (create when answers
arrive).

## Spec-lock open questions (blocking Phase 1)

These must be answered by the professor before any work in WBS-2 (Core Data Plane):

1. Default value of `N` (records held per circular buffer).
2. Canonical sample rate: 80 SPC (protection) or 256 SPC (measurement) per 60 Hz cycle?
3. Interpolation order: linear sufficient, or quadratic (matching Standard-PMU)?
4. Write-back authentication model: API-level auth, or process-level isolation?
5. Existing reference SCD schema available, or define one as part of M0?
6. Clock-holdover reporting mechanism from `linuxptp` into the health surface.

Prepare a one-page proposal that fills in SSIEC's recommended default for each, formatted so
the professor can respond with yes/no per item rather than open-ended design.

## Build and test commands (once toolchain is pinned)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## Where to look when

- **Architecture question?** SDD §5 (system architecture) and §6 (component design).
- **What's in/out of scope?** SDD §2.
- **Why are we doing X?** IP §1 (purpose), §1.4 (northbound strategy rationale).
- **What phase are we in, what's next?** IP §4 (phased execution plan) and `Phase 0`
  checklist above.
- **How long should X take?** IP §2 has per-item effort estimates.
- **Found an ambiguity in the SDD?** Add to the spec-lock questions list. Do not guess.

## Working style

- Prefer small, reviewable commits over large ones.
- Every commit message references the WBS code: `WBS-2.5: implement Time Aligner skeleton`.
- Every PR closes one or more GitHub issues by WBS code.
- Before writing code for a WBS item, re-read the corresponding SDD section.
- After completing a WBS item, update its checkbox in this file.

## Coexistence with Antigravity sub-agents

This repository is worked by two classes of agent in parallel:

- **You (Claude Code)** — design, deep reasoning, careful single-task writing,
  documentation, code review. You are the "senior engineer."
- **Antigravity sub-agents** — parallel execution, build/test loops, CI
  maintenance, scheduled benchmarks, multi-file mechanical refactors,
  browser-based verification. They are the "build farm."

Their operating manual is `AGENTS.md`. You should be aware of the division of
labour so you don't accidentally pick up work that suits them better, and
vice versa:

- Use branches named `claude/<wbs>-<short>`; they use `antigravity/<wbs>-<short>`.
- Commit message trailer: `Agent: claude-code` (for provenance, so reviewers
  and the other tool can see who touched what).
- Do not edit a file whose most recent commit trailer is
  `Agent: antigravity-subagent-*` if that file is in their domain
  (CI configs, generated benchmark reports, scheduled task scripts).
- When you author an ADR or non-trivial design, tag `@antigravity-team`
  in a PR comment with an explicit ask, e.g.,
  "please run the benchmark suite on this branch and post p50/p99 distributions."

When in doubt about who should do a task, default to authoring an issue with
`for:claude` or `for:antigravity` label rather than proceeding.

## What this project is **not**

- Not a hardware MU/SCU product (that would be a separate SSIEC product effort).
- Not a Korean deployment (the deliverable goes to Georgia Tech, who deploys at US sites).
- Not a fork or replacement of any vendor's existing protection platform.
- Not a research paper artifact alone — it is intended to be operated.
