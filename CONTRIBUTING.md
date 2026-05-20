# Contributing to SVDC

## Code conventions

### Language (NFR-10)

**All user-facing artefacts shall be in English without exception.** This covers:

- Source-code identifiers (modules, functions, types, variables, constants)
- Source-code comments and docstrings
- CLI option names and `--help` text
- Log messages at any level
- Error messages and panic messages
- HTTP / JSON field names and response payloads
- Configuration-file keys
- Git commit messages
- Pull request titles and descriptions
- Issue titles and bodies
- All Markdown and HTML documentation in this repository

This convention supports international code review with the Georgia Tech team and
unambiguous integration with the reference QSE. It is enforced by `scripts/lint-english-only.sh`
(added in Phase 0 WBS-1.5) and reviewed at every PR.

### Style

- `cargo fmt` clean.
- `cargo clippy -- -D warnings` clean.
- Public APIs documented with `///` doc-comments including at least one usage example.
- Tests are mandatory for any logic that can fail; coverage target is 80% line.

### Commits

- Reference the WBS code in every commit message: `WBS-2.5: implement Time Aligner skeleton`.
- Conventional Commits style is fine but not required; the WBS reference is required.
- One logical change per commit. Squash on merge.

### PRs

- Each PR closes one or more GitHub issues identified by WBS code.
- PR description includes: what changed, why, and how it was tested.
- At least one reviewer approval before merge.
- CI must pass.

## Phase discipline

Work within the current phase as defined in `docs/SVDC_Implementation_Plan_v0.2.html` §4.
If a task would belong to a future phase, open an issue, label it, and defer.

Do not start Phase N+1 work until Phase N exit criteria pass. If exit criteria seem wrong,
raise an issue rather than working around them.

## Decisions and ambiguities

Architectural decisions land in `docs/decisions/` as dated ADRs (Architecture Decision Records).
Spec ambiguities discovered during implementation go into the active spec-lock document
(`docs/spec-lock-vX.Y.md`); they are not resolved silently by the implementer.

## Safety

This software is intended to participate in power-system protection. While SVDC itself does
not make protection decisions, it is on the critical data path. Code review should weigh
safety implications of every change to the hot path (M1 → M2 → M3 → M4 → M5/M6).
