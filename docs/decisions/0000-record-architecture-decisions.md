# ADR-0000 — Record architecture decisions

- **Status:** accepted
- **Date:** 2026-05-21
- **Deciders:** SSIEC SVDC team

## Context

The SVDC project is developed in parallel by two classes of agent (Claude Code,
Antigravity sub-agents) plus human contributors at SSIEC and Georgia Tech.
Without a durable, single-place record of WHY a decision was taken, parallel
agents will rediscover the same trade-offs, contradict each other, or silently
diverge from prior choices that are no longer visible in the code.

We need a low-friction format that:

- Records the reasoning behind a decision, not just the outcome.
- Lives in the repository so it travels with the code it constrains.
- Is referenced by `AGENTS.md` so autonomous agents check it before acting.

## Decision

Architecture Decision Records (ADRs) are stored as Markdown files in
`docs/decisions/` and follow the file-naming convention
`NNNN-kebab-case-title.md`, where `NNNN` is a zero-padded sequence number.

Each ADR follows MADR-lite structure:

```
# ADR-NNNN — <title>

- Status: proposed | accepted | superseded by ADR-XXXX | deprecated
- Date: YYYY-MM-DD
- Deciders: <names or roles>

## Context
<why is this decision needed? what forces are at play?>

## Decision
<what is decided?>

## Consequences
<what follows from this — positive, negative, neutral?>

## Alternatives considered (optional)
<what was rejected and why?>

## References (optional)
<links to SDD/IP sections, papers, prior ADRs>
```

ADRs are append-only. To change a prior decision, write a new ADR with status
`accepted` and set the prior ADR's status to `superseded by ADR-XXXX`. Never
edit the substance of an accepted ADR after the fact; only the status header.

Both Claude Code and Antigravity must read `docs/decisions/` before authoring
code that affects an area covered by an ADR. `AGENTS.md` enforces this for
autonomous agents.

## Consequences

- Decisions become discoverable from the repository alone — no external wiki
  to fall out of sync.
- The two-agent workflow has a single canonical place to settle design
  questions before code is written.
- Slight ceremony tax on small decisions; offset by the convention that
  trivial choices (variable naming, helper-function shape) do not need ADRs —
  only cross-cutting decisions do.

## References

- Michael Nygard, "Documenting Architecture Decisions" (2011).
- MADR project: https://adr.github.io/madr/
- `AGENTS.md` — "Project context — read first" section pointing here.
- `CONTRIBUTING.md` — "Decisions and ambiguities" section.
