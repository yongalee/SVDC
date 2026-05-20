<!--
Format mandated by AGENTS.md "PR comment etiquette" and ADR-0001
"Cross-tool PR review". Both Claude Code and Antigravity must use this.
-->

## Summary

<!-- One paragraph: what changed, why now. -->

## WBS items addressed

<!-- One bullet per WBS code touched. Reference the IP section. -->

- WBS-X.Y: <what was done>

## How it was tested

<!-- Concrete commands, expected vs observed. CI counts; manual verification too. -->

- [ ] `cargo build --workspace`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo fmt --all -- --check`
- [ ] `bash scripts/lint-english-only.sh`
- [ ] Other: ...

## ADR check

- [ ] No ADR in `docs/decisions/` is contradicted by this PR.
- [ ] If this PR introduces a cross-cutting design decision, a new ADR is
      included or linked.

## Asks of the reviewer

<!--
Tag the right reviewer for the right concern (per ADR-0001):
- Claude Code: design coherence, ADR compliance, API shape.
- Antigravity / human ops reviewer: CI/bench/conformance.
-->

- [ ] **Claude Code** — please verify: <specific design concern>
- [ ] **Antigravity** — please verify: <specific execution concern>
- [ ] **Human reviewer** — please confirm: <specific operational concern>

## Provenance

Commit messages include the `OWNER:` trailer per ADR-0001 §7.
