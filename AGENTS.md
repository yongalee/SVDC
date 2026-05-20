# SVDC — Agent Operating Manual

> This file is the operating manual for autonomous agents working on this
> repository, including Google Antigravity sub-agents. Claude Code reads
> `CLAUDE.md`; agents that use this repository as a workspace should read
> both, but the operational protocol below takes precedence for
> autonomous, parallel, or scheduled work.

## Project context — read first

This is the SVDC project. Before any work, read in this order:

1. `CLAUDE.md` — project identity, conventions, current phase
2. `docs/SVDC_Design_Document_v0.1.html` — the authoritative SDD
3. `docs/SVDC_Implementation_Plan_v0.2.html` — the IP
4. `docs/spec-lock-v0.1.md` — if open, **Phase 1+ work is blocked**

## Division of labour (dual-tool workflow)

This repository is worked by two classes of agent:

- **Claude Code** — design, deep reasoning, careful single-task writing,
  documentation, code review.
- **Antigravity sub-agents** — parallel execution, build/test loops,
  CI maintenance, scheduled benchmarks, multi-file mechanical
  refactors, browser-based verification.

Antigravity sub-agents must restrict themselves to the task categories
listed under "Antigravity-appropriate tasks" below. If a task does not
fit cleanly, leave it for Claude Code (open an issue labelled
`for:claude` and stop).

### Antigravity-appropriate tasks

- Running `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`
  loops; reporting results.
- Authoring and maintaining GitHub Actions workflow files in
  `.github/workflows/`.
- Scheduled benchmark runs (Phase 5 onward); collecting HDR-histogram
  output and posting summaries to PR comments.
- Long-running soak tests (Phase 6); monitoring and reporting.
- Multi-file mechanical refactors **after** Claude Code has authored
  the design ADR — e.g., renaming a type across crates after Claude
  has decided the new name and rationale.
- Dependency upgrade sweeps: bump versions, run tests, open PR.
- Scaffolding new crates after the design is settled (skeleton
  Cargo.toml + lib.rs + initial test).
- Issue triage: applying labels by WBS code based on issue body.
- Browser-based verification: launching `UaExpert` to browse the
  L1 OPC UA AddressSpace and capturing screenshots; running an MQTT
  client to verify L2 publication; running SQL queries against
  TimescaleDB to verify L3 persistence.
- Pcap capture comparison: replaying recorded MU captures through the
  SVDC and diffing against expected outputs.
- Building and pushing release artifacts (Phase 6).

### Tasks that must go to Claude Code instead

- Any change to `crates/svdc-core/` data structures.
- Any change to the lock-free protocol in `crates/svdc-cb/` (when it
  exists).
- ASN.1 BER decoder logic in `crates/svdc-ingress/` (when it exists).
- Any change to `docs/SVDC_Design_Document_v0.1.html` or
  `docs/SVDC_Implementation_Plan_v0.2.html`.
- Authoring ADRs in `docs/decisions/`.
- Any change to the C ABI signature in `crates/svdc-cabi/` once it
  exists.
- Resolution of spec-lock questions.
- Initial author of any non-trivial PR description, commit message
  body, or user-facing documentation.

If unsure, open an issue, label it `for:claude`, and stop.

## Branch and commit protocol

- Branch names: `antigravity/<wbs-code>-<short-name>`, e.g.
  `antigravity/wbs-1-4-ci-pipeline`. Claude Code uses
  `claude/<wbs-code>-<short-name>`.
- Never push to `main` directly. Always via PR.
- Commit message format: `WBS-X.Y: <imperative summary>` on the first
  line, optional body, and a trailer `Agent: antigravity-subagent-<id>`
  so reviewers can see provenance.
- One PR closes one or more issues identified by WBS code.

## File ownership (concurrent-edit protection)

To prevent merge conflicts with parallel Claude Code work, before
editing, check the `OWNER` trailer in a file's most recent commit:

- If the most recent commit on a file is by `claude-code`, do not edit
  that file in a long-running task unless the file is in the
  "Antigravity-appropriate tasks" categories above.
- Conversely, files most recently touched by `antigravity-subagent-*`
  are open for Antigravity work.
- This convention prevents both tools from editing the same file at
  the same time.

## When in doubt, stop and surface

Autonomous agents should err strongly toward stopping and asking
rather than proceeding. Specifically:

- If an exit criterion in the current Phase appears not yet met but a
  task seems to belong to the next Phase, **stop** and open an issue
  labelled `phase-discipline-question`.
- If a test starts failing after a change you made, do not modify the
  test to make it pass; revert your change and open an issue
  describing what broke.
- If the SDD or IP appears to contradict itself or the paper, **stop**
  and open an issue labelled `spec-ambiguity`.
- If a dependency upgrade would introduce a breaking API change, do
  not auto-resolve; surface it for review.

## PR comment etiquette (cross-tool handoff)

When you complete a task and open a PR, the PR body must include:

```
## Summary
<one paragraph>

## WBS items addressed
- WBS-X.Y: <what was done>

## How it was tested
<commands run, results>

## Asks of the reviewer
- [ ] Claude Code: please verify <specific design concern>
- [ ] Human reviewer: please confirm <specific operational concern>
```

Claude Code will tag you (`@antigravity-team`) in review comments
that require execution work — e.g., "run the benchmark suite on this
branch and post p50/p99 distributions in a comment."

## What this project is not (do not be tempted)

- Not a Korean deployment project.
- Not an SSIEC product launch.
- Not a place to add cloud-only Google features (Firebase, Cloud Run
  integrations) that would couple the SVDC to a specific platform.
  The Georgia Tech deployment must remain self-hostable on commodity
  Linux.

## Quick reference

- Phase status: see `CLAUDE.md` "Current state" section.
- Spec-lock status: see `docs/spec-lock-v0.1.md` header.
- Open questions: see `docs/spec-lock-v0.1.md` Q1–Q6.
- Build commands: `CLAUDE.md` "Build and test commands" section.
- Code conventions (NFR-10 English-only): `CONTRIBUTING.md`.
