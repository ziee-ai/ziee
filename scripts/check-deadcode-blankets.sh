#!/usr/bin/env bash
# Tier-C guard: deny NEW module-level `#![allow(dead_code)]` blankets.
#
# The workspace lint policy (src-app/Cargo.toml [workspace.lints]) keeps
# `dead_code = "warn"` — non-breaking — so the ~90 existing whole-module
# `#![allow(dead_code)]` blankets can be paid down incrementally (Tier C in
# WARNING_AUDIT.md §6) without breaking the build. The risk is that NEW
# blankets get added faster than the old ones are removed, so the backlog
# never shrinks. This guard freezes the set: the baseline lists every file
# that carried a blanket at audit time; ADDING a blanket to any other file
# fails CI. REMOVING one (the whole point of Tier C) is always allowed and
# should be followed by dropping that path from the baseline.
#
# Run from the repo root: `bash scripts/check-deadcode-blankets.sh`
set -euo pipefail

cd "$(dirname "$0")/.."   # repo root
BASELINE="scripts/deadcode-blanket-baseline.txt"

# Current set of files with a module-level `#![allow(dead_code)]`.
current="$(grep -rl '#!\[allow(dead_code)\]' \
    src-app/server/src src-app/desktop/tauri/src | sort)"

# Files that have a blanket now but are NOT in the baseline = additions.
added="$(comm -23 <(printf '%s\n' "$current") <(sort "$BASELINE") || true)"

if [ -n "$added" ]; then
    echo "FAIL: new module-level #![allow(dead_code)] blanket(s) added:" >&2
    printf '  %s\n' $added >&2
    echo >&2
    echo "Whole-module dead_code suppression hides FUTURE dead code too." >&2
    echo "Prefer a narrow per-item #[allow(dead_code)] with a reason, or delete" >&2
    echo "the unused item. If a blanket is truly justified, add its path to" >&2
    echo "$BASELINE with a note." >&2
    exit 1
fi

# Informational: baseline entries whose blanket was removed (paydown progress).
removed="$(comm -13 <(printf '%s\n' "$current") <(sort "$BASELINE") || true)"
if [ -n "$removed" ]; then
    echo "note: $(printf '%s\n' $removed | grep -c .) baseline file(s) no longer carry a blanket" >&2
    echo "      (Tier-C paydown — drop these from $BASELINE):" >&2
    printf '  %s\n' $removed >&2
fi

echo "✓ no new #![allow(dead_code)] module blankets ($(printf '%s\n' "$current" | grep -c .) present, baseline $(grep -c . "$BASELINE"))"
