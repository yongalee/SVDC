# ADR-0001 — Dual-agent workflow: Claude Code and Antigravity

- **Status:** accepted
- **Date:** 2026-05-21
- **Deciders:** SSIEC SVDC team

## Context

The SVDC repository is worked by two classes of autonomous agent that do not
share memory, conversation context, or running state:

- **Claude Code** — deep reasoning, single-task careful writing, design
  authorship, code review, documentation.
- **Google Antigravity sub-agents** — parallel execution, build/test loops,
  CI maintenance, scheduled benchmarks, multi-file mechanical refactors,
  browser-based verification.

The two tools share exactly one surface: the git repository (plus its
GitHub-side artefacts — issues, PRs, Actions). Anything that needs to flow
from one to the other must be materialised as a commit, an issue, a PR
comment, or a CI artifact. There is no shortcut.

`AGENTS.md` already documents the operational rules. This ADR records the
WHY and freezes the protocol so a future contributor (human or agent) can
trace the rationale, not just the rule.

## Decision

### 1. Git as the sole handoff surface

Neither tool may rely on out-of-band communication (chat history, shared
filesystem state outside the repo, IDE workspace memory). Everything load-
bearing lives in `main` after a PR merge or in a branch open as a PR.

### 2. Issue routing by label

Every actionable issue carries exactly one routing label:

- `for:claude` — design, deep reasoning, ADR authorship, careful single-file
  edits, code review.
- `for:antigravity` — execution, parallel runs, CI/bench, multi-file
  mechanical changes (after design is settled), browser/MQTT/SQL verification.

Issues without a routing label are unowned and must be triaged before work
starts. Issues may carry additional labels (WBS code, phase, discipline tags)
but routing is one of the two above.

### 3. Branch naming

Branches use the agent prefix followed by a WBS code and short slug:

- `claude/<wbs-code>-<short-name>` — e.g. `claude/wbs-2-5-aligner-skeleton`
- `antigravity/<wbs-code>-<short-name>` — e.g. `antigravity/wbs-1-4-ci-pipeline`

Anyone reading `git branch -a` can immediately see who is working on what.
`main` is protected; all changes land via PR.

### 4. Cross-tool PR review

- PRs opened by Claude Code request execution-side review from Antigravity:
  CI must be green, benchmarks (when added) must show no regression,
  conformance vectors must pass.
- PRs opened by Antigravity request design coherence review from Claude
  Code: the change must not contradict an ADR; the change must stay within
  the categories listed in `AGENTS.md` "Antigravity-appropriate tasks".

The PR template (`.github/PULL_REQUEST_TEMPLATE.md`) enforces this with a
checklist that names both reviewers explicitly.

### 5. ADR-first for non-trivial design

Any decision that touches more than one crate, changes a public API, or
locks in a protocol choice (wire format, lock-free invariant, threading
model) is recorded as an ADR before code is written. Antigravity must not
authorise such a decision; it must open a `for:claude` issue and stop.

### 6. One file at a time; partition primarily by crate

To avoid merge conflicts between agents working concurrently:

- The primary partition is by crate. While `crates/svdc-ingress` is under
  active Claude Code authorship, Antigravity does not edit it (it may
  observe builds, run tests, post results).
- Across files within a single agent's task, edit one file per commit
  where reasonable.
- The OWNER-trailer mechanism in `AGENTS.md` ("File ownership" section) is
  the recovery procedure when partitioning is ambiguous: check the most
  recent commit's `OWNER:` trailer before editing.

### 7. Commit message trailer for provenance

Every commit ends with a trailer identifying the authoring agent:

```
OWNER: claude-code
```

or

```
Agent: antigravity-subagent-<id>
OWNER: antigravity
```

This makes `git blame` and the OWNER check above mechanically reliable.

## Consequences

- The repository becomes a complete record of what each tool did and why,
  legible to humans without access to either tool's internal state.
- Parallel work is safe by partition; merge conflicts are rare and the
  recovery procedure is documented.
- A small ceremony cost on every PR (the template's reviewer checklist).
  Accepted because the cost of cross-tool drift is much higher.
- If a future agent class is added (e.g. a third tool), this ADR is
  superseded by a new one explicitly listing all classes and their lanes.

## Alternatives considered

- **Single-agent workflow, drop Antigravity.** Rejected — Antigravity's
  execution parallelism is genuinely useful for CI, benchmarks, and
  long-running soak tests, especially in Phase 5 and Phase 6.
- **Shared memory via a side-channel (chat link, shared file).** Rejected —
  fragile, non-auditable, breaks the "complete record in git" property.
- **Branch-name-only routing (no labels).** Rejected — labels also drive
  issue queues for issues that have not yet been picked up; branch names
  exist only after work has started.

## References

- `AGENTS.md` — operational rules (the normative document for autonomous
  agents).
- `CLAUDE.md` — project context for Claude Code; cross-references this ADR.
- ADR-0000 — record architecture decisions (defines this file's format).
- User handoff message dated 2026-05-21 — original statement of the
  protocol that this ADR codifies.
