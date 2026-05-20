# WBS-9 Operator Console — Dual-Agent Handoff Plan

> **Audience.** This document is read by both Claude Code and the Google
> Antigravity sub-agent before any WBS-9 issue is picked up. It is the
> normative source for *who works on what file* and *which order things
> land in*. Surface deviations as `for:claude` issues; do not silently
> diverge.

## Read first

In order:

1. `AGENTS.md` — operational rules for autonomous agents
2. `docs/decisions/0001-dual-agent-workflow.md` — routing protocol
3. `docs/SVDC_UI_Design_Document_v0.1.html` — the authoritative spec
   for what the Console is (§1 workflows, §3 IA, §4 screens, §5 data
   flow, §6 stack, §7 WBS-9 expansion)
4. `docs/decisions/0004-ui-stack.md` — tech-stack confirmation,
   supersedes ADR-0002
5. `docs/decisions/0005-daemon-vs-ui-mode.md` — `--no-ui` / `--ui-bind`
   runtime toggle
6. `docs/SVDC_Implementation_Plan_v0.3_patch.md` — IP patch adding
   WBS-9 with phase mapping and PD estimates

## Crate structure

```
crates/
├── svdc-bin/        # binary; wires svdc-console behind --ui flag (ADR-0005)
├── svdc-console/    # NEW — the Operator Console (this work item)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs           # router registration; single public entry
│   │   ├── routes/
│   │   │   ├── dashboard.rs
│   │   │   ├── mus_list.rs
│   │   │   ├── mu_detail.rs
│   │   │   ├── northbound.rs
│   │   │   ├── monitoring.rs
│   │   │   └── config.rs
│   │   ├── sse/
│   │   │   ├── mod.rs        # SSE event contract (typed)
│   │   │   └── emitter.rs    # mock + real source wiring
│   │   ├── templates/
│   │   │   ├── base.rs       # maud layout, sidebar, top bar
│   │   │   ├── components.rs # cards, badges, status dots
│   │   │   └── charts.rs     # inline-SVG generators
│   │   └── assets/           # CSS, JS, fonts — rust-embed
│   └── tests/
│       ├── unit/             # per-module
│       └── playwright/       # E2E (WBS-9.7)
```

Splitting `mus_list.rs` from `mu_detail.rs` (rather than one `mus.rs`)
is deliberate — it lets Claude and Antigravity edit MU work without
file-level collisions.

## Lane assignment

| WBS | File(s) | Owner | Why |
|---|---|---|---|
| 9.1a | `lib.rs`, `templates/base.rs` (layout, sidebar, top bar) | **Claude** | Single structural design point |
| 9.1b | `Cargo.toml` (deps), `assets/` wiring, `rust-embed` setup | **Antigravity** | Mechanical wiring, build-loop friendly |
| 9.2a | `routes/dashboard.rs`, `sse/mod.rs` (event contract types) | **Claude** | Read-mostly design + typed SSE contract |
| 9.2b | `sse/emitter.rs` (mock data + later real wiring) | **Antigravity** | Iteration / mock generation |
| 9.3a | `routes/mu_detail.rs`, `templates/charts.rs` (8-channel SVG + downsampler) | **Claude** | Hot-path-adjacent (downsampling math) |
| 9.3b | `routes/mus_list.rs`, MU-card component in `templates/components.rs` | **Antigravity** | Repeated card layout, MU-count permutations |
| 9.4a | `routes/northbound.rs` shell + enable/disable POST handler signature | **Claude** | API contract decision |
| 9.4b | L0 / L1 / L2 / L3 per-layer detail cards inside `routes/northbound.rs` | **Antigravity** | Four near-identical card variants — replicate |
| 9.5a | Latency-histogram math + p50/p99 SVG renderer | **Claude** | Statistics correctness sensitive |
| 9.5b | PTP/CB line charts, audit-log table, 1 Hz polling wiring | **Antigravity** | Repeated chart pattern |
| 9.6a | SCD upload validator + channel-registry model | **Claude** | Domain-model careful authoring |
| 9.6b | Config form HTML, About page, parameter read/write wiring | **Antigravity** | Form scaffolding |
| 9.7 | `tests/playwright/` end-to-end suite | **Antigravity** | Browser verification per AGENTS.md |

Total: 15.5 PD (UI Doc §7.1). Claude lane: ~8 PD. Antigravity lane:
~7.5 PD.

## File ownership rules

1. **One file, one author per PR.** If file `X` is in Antigravity's
   lane this iteration, Claude does not touch it in any PR opened
   during that iteration, and vice versa.
2. **If a split becomes painful, split the file further.** Don't
   tolerate cross-lane edits to a single file. Module-split is cheap
   in Rust; do it.
3. **Shared files** — `Cargo.toml` (workspace root), `CLAUDE.md`,
   `AGENTS.md`, `docs/decisions/*` — only Claude edits. Antigravity
   surfaces requested changes as `for:claude` issues.
4. **Generated assets** — anything under `crates/svdc-console/src/assets/`
   that is the verbatim HTMX or Alpine.js source is Antigravity's. CSS
   custom to this project may go either way per the per-WBS table.

## Dependency graph for issues

```
issue #9   [for:claude]      ADR-0004 confirmation review
issue #10  [for:claude]      ADR-0005 confirmation review
issue #11  [for:claude]      IP v0.3 patch confirmation review
                  │
                  ▼ (blocked-on the three reviews — but not really,
                     they are records of decisions already taken;
                     these issues exist for traceability)
issue #12  [for:antigravity] WBS-9.1b: svdc-console crate scaffold + rust-embed
issue #13  [for:antigravity] CI: add svdc-console to build matrix + Playwright job stub
                  │
                  ▼ (12 and 13 unblock everything below)
issue #14  [for:claude]      WBS-9.1a: base layout, sidebar, top bar
issue #15  [for:claude]      WBS-9.2a: Dashboard tiles + typed SSE contract
issue #16  [for:antigravity] WBS-9.2b: SSE emitter + mock data
                  │
                  ▼ (Dashboard end-to-end visible, parallel work resumes)
issue #17  [for:claude]      WBS-9.3a: MU detail + downsampler + SVG
issue #18  [for:antigravity] WBS-9.3b: MU list page + cards
issue #19  [for:claude]      WBS-9.4a: Northbound shell + enable/disable API
issue #20  [for:antigravity] WBS-9.4b: L0/L1/L2/L3 layer cards
issue #21  [for:claude]      WBS-9.5a: latency histogram + p50/p99
issue #22  [for:antigravity] WBS-9.5b: PTP/CB charts + audit log
issue #23  [for:claude]      WBS-9.6a: SCD validator + channel model
issue #24  [for:antigravity] WBS-9.6b: Config form + About
issue #25  [for:antigravity] WBS-9.7: Playwright E2E suite
```

After #12 and #13 land on `main`, parallel work is possible: any
`for:claude` issue can run alongside any `for:antigravity` issue
provided their files don't overlap (the table above guarantees they
don't).

## Branch naming

Per ADR-0001 §3:

- Claude branches: `claude/wbs-9-<id>-<short>`, e.g.
  `claude/wbs-9-2a-dashboard-tiles`
- Antigravity branches: `antigravity/wbs-9-<id>-<short>`, e.g.
  `antigravity/wbs-9-1b-crate-scaffold`

## Commit message template

Per ADR-0001 §7 and CONTRIBUTING.md:

```
WBS-9.<id>: <imperative summary>

<body — what changed, why now>

Closes #<issue-number>.

OWNER: claude-code        # or:  OWNER: antigravity
                          #      Agent: antigravity-subagent-<id>
```

For Claude commits authored via Claude Code, append:

```
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

## PR format

The `.github/PULL_REQUEST_TEMPLATE.md` enforces the format:

- Summary
- WBS items addressed
- How it was tested (cargo + Playwright if applicable)
- ADR check
- Asks of the reviewer (Claude reviews Antigravity PRs for design
  coherence; Antigravity runs CI/E2E on Claude PRs)
- Closes `#<issue>`.

## CI implications

Antigravity's WBS-9.1b / CI follow-up (issue #13) must extend
`.github/workflows/ci.yml` to:

- Build `svdc-console` on both Ubuntu and Windows (existing matrix).
- Verify `cargo run -p svdc-bin -- --no-ui --help` succeeds (proves
  the daemon-only path is intact).
- Add a Playwright job stub on Ubuntu only, gated by a path filter so
  it only runs when files under `crates/svdc-console/` or
  `tests/playwright/` change.

Branch protection on `main` will need to add `cargo build (windows-latest)`
and Playwright contexts to the required checks list once those land.
Do not modify branch protection until those checks exist and pass at
least once (otherwise PRs become unmergeable).

## When to stop and surface

Per AGENTS.md, autonomous agents err toward stopping. WBS-9 specifics:

- **If a screen's design requires a decision not in the UI Doc**, open
  a `for:claude` issue with `spec-ambiguity`. Do not invent.
- **If a Cargo dep is needed beyond the ADR-0004 list**, surface it
  before adding. `serde_json` may be needed for SSE payloads — that's
  implied by ADR-0004 and is fine to add. Adding a chart library or
  an SPA framework is **not** fine; that would supersede ADR-0004.
- **If a test breaks after your change**, revert and open an issue.
  Do not modify the test to match the broken behaviour.
- **If WBS-9.5 latency histograms need real benchmark data that
  doesn't exist yet** (Phase 5 hasn't run), use a mock data source
  in the SSE emitter; do not block.

## Start here (Antigravity action sequence)

After this document is merged to `main`:

1. Read this document, `0004-ui-stack.md`, `0005-daemon-vs-ui-mode.md`,
   `0001-dual-agent-workflow.md`, and UI Doc §4 + §6 + §7.
2. Pick up issue **#12** (WBS-9.1b crate scaffold). Branch:
   `antigravity/wbs-9-1b-crate-scaffold`. Land it. Open PR.
3. Pick up issue **#13** (CI extension). Branch:
   `antigravity/wbs-9-1-ci-matrix`. Land it. Open PR.
4. After both merge, you are unblocked on issues **#16, #18, #20,
   #22, #24, #25** (mark them ready in priority order). Pick the one
   whose Claude-side prerequisite is already merged.

Claude's parallel sequence starts at issue **#14** (WBS-9.1a base
layout) once #12 has landed and the crate exists to put templates in.
