#!/usr/bin/env bash
# scripts/lint-english-only.sh
#
# Enforces NFR-10 (English-only artefacts). Greps tracked files for any
# Hangul code points and fails if found. Vendor-supplied HTML documents
# in docs/ may contain CJK characters from rendering libraries; they are
# excluded.
#
# Exit codes:
#   0 — no Hangul outside excluded paths.
#   1 — Hangul found; offending file(s) printed.
#   2 — invoked outside a git working tree.

set -euo pipefail

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "lint-english-only: not inside a git working tree" >&2
  exit 2
fi

# Files to exclude from the check. Keep this list tight.
EXCLUDES=(
  "docs/SVDC_Design_Document_v0.1.html"
  "docs/SVDC_Implementation_Plan_v0.2.html"
  "scripts/lint-english-only.sh"
)

# Hangul ranges:
#   AC00..D7A3  Hangul Syllables
#   3130..318F  Hangul Compatibility Jamo
#   1100..11FF  Hangul Jamo
HANGUL_RE=$'[\xea-\xed][\x80-\xbf][\x80-\xbf]'   # broad UTF-8 sieve covering the above
HANGUL_RE_STRICT='[\xEA\xEB\xEC\xED]'             # quick screen — refined by grep -P below

# Build the git-ls-files invocation, excluding the listed paths.
mapfile -t FILES < <(
  git ls-files |
    while read -r f; do
      skip=0
      for ex in "${EXCLUDES[@]}"; do
        if [[ "$f" == "$ex" ]]; then skip=1; break; fi
      done
      [[ $skip -eq 0 ]] && echo "$f"
    done
)

# Look for Hangul code points using a Perl-regex grep on UTF-8 bytes.
# Hangul Syllables block covers the practical 99% case.
HITS=$(
  if (( ${#FILES[@]} > 0 )); then
    LC_ALL=C grep -lP '[\xEA-\xED][\x80-\xBF][\x80-\xBF]' -- "${FILES[@]}" 2>/dev/null \
      | xargs -r -I{} grep -lP '\p{Hangul}' --binary-files=without-match -- {} 2>/dev/null \
      || true
  fi
)

if [[ -n "${HITS}" ]]; then
  echo "lint-english-only: NFR-10 violation — Hangul found in:" >&2
  printf '  %s\n' ${HITS} >&2
  echo "Move user-facing text to English. See CONTRIBUTING.md 'Language (NFR-10)'." >&2
  exit 1
fi

echo "lint-english-only: OK (no Hangul in tracked files outside excluded paths)"
exit 0
