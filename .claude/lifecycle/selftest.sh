#!/usr/bin/env bash
# selftest.sh — portable, self-contained verification of the feature-lifecycle
# validator's FRONTEND gates (phase 3 e2e-tier requirement + phase 8
# npm-run-check / e2e result requirement) and the backend-only exemption.
#
# Builds throwaway git repos with controlled diffs + a full set of lifecycle
# artifacts, then asserts the validator's exit code for each scenario. No
# network, no repo-specific SHAs — runs on any clone.
#
#   bash .claude/lifecycle/selftest.sh
#
# Exit 0 = all scenarios behaved as specified.
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK="$HERE/lifecycle-check.mjs"
PASS=0
FAIL=0

note() { printf '  %s\n' "$*"; }
# assert_exit <expected 0|1> <label> -- <validator args...>
assert_exit() {
  local want="$1"; local label="$2"; shift 2; [ "$1" = "--" ] && shift
  node "$CHECK" "$@" >/tmp/lc-selftest.out 2>&1
  local got=$?
  # normalize: the validator uses exit 1 for a gate failure; treat >0 as 1
  [ "$got" -ne 0 ] && got=1
  if [ "$got" = "$want" ]; then
    PASS=$((PASS+1)); printf '  \033[32mok  \033[0m %s (exit %s)\n' "$label" "$got"
  else
    FAIL=$((FAIL+1)); printf '  \033[31mFAIL\033[0m %s (want exit %s, got %s)\n' "$label" "$want" "$got"
    sed 's/^/        | /' /tmp/lc-selftest.out
  fi
}

# ---------------------------------------------------------------------------
# artifact writers (shared by both fixtures)
# ---------------------------------------------------------------------------
# write_common <feature-dir> — phases 1,2,4,5,6,7 artifacts, valid.
# Coverage covers the changed source file's hunk (1..N) with 3 angles.
write_common() {
  local d="$1" srcfile="$2" srclines="$3"
  cat > "$d/PLAN_AUDIT.md" <<'EOF'
# PLAN_AUDIT
## Breakage risk
None — additive.
## Pattern conformance
Mirrors the reference module.
## Migration collisions
No migration.
## OpenAPI regen
Not required.

- **ITEM-1** — verdict: PASS — mirrors the reference module; additive only.
EOF
  cat > "$d/DECISIONS.md" <<'EOF'
# DECISIONS
### DEC-1: What does the change render/return?
**Resolution:** the minimal additive surface described in PLAN.md.
**Basis:** convention — matches the reference module.
EOF
  cat > "$d/DRIFT-1.md" <<'EOF'
# DRIFT round 1
- **DRIFT-1.1** — verdict: none — implementation matches the plan.
**Unresolved drifts:** 0
EOF
  # 12 distinct angles (>= ANGLE_MIN of 10)
  : > "$d/LEDGER.jsonl"
  for a in correctness security error-handling concurrency perms api-contract \
           state-management a11y patterns-conformance tests-quality perf i18n; do
    printf '{"angle":"%s","file":"%s","line":1,"severity":"info","finding":"none","status":"rejected"}\n' \
      "$a" "$srcfile" >> "$d/LEDGER.jsonl"
  done
  printf 'file\tstart\tend\tangles\n' > "$d/AUDIT_COVERAGE.tsv"
  printf '%s\t1\t%s\tcorrectness,a11y,patterns-conformance\n' "$srcfile" "$srclines" >> "$d/AUDIT_COVERAGE.tsv"
  cat > "$d/FIX_ROUND-1.md" <<'EOF'
# FIX_ROUND 1
No confirmed findings to fix.
**New confirmed findings:** 0
EOF
}

# ---------------------------------------------------------------------------
# scratch repo scaffolding
# ---------------------------------------------------------------------------
new_repo() {
  local root; root="$(mktemp -d)"
  git -C "$root" init -q -b main
  git -C "$root" config user.email t@t.t
  git -C "$root" config user.name t
  echo "seed" > "$root/README.md"
  git -C "$root" add -A && git -C "$root" commit -qm baseline
  echo "$root"
}

echo "== feature-lifecycle validator self-test =="

# ---------------------------------------------------------------------------
# FIXTURE 1 — FRONTEND-TOUCHING feature (src-app/ui/**)
# ---------------------------------------------------------------------------
FE="$(new_repo)"
git -C "$FE" checkout -q -b feat/foo
mkdir -p "$FE/src-app/ui/src/modules/foo" "$FE/src-app/ui/openapi" "$FE/.lifecycle/foo"
cat > "$FE/src-app/ui/src/modules/foo/FooPage.tsx" <<'EOF'
export function FooPage() {
  return (
    <div>
      <h1>Foo</h1>
      <button>Save</button>
    </div>
  );
}
EOF
# a GENERATED artifact also changes — must NOT count as a real UI touch
echo '{"openapi":"3.0.0"}' > "$FE/src-app/ui/openapi/openapi.json"

FD="$FE/.lifecycle/foo"
cat > "$FD/PLAN.md" <<'EOF'
# PLAN — foo
## Items
- **ITEM-1**: Add a FooPage component to the ui workspace.
## Files to touch
- `src-app/ui/src/modules/foo/FooPage.tsx` — new page (ITEM-1).
- `src-app/ui/openapi/openapi.json` — regenerated (excluded from gates).
## Patterns to follow
- Mirror an existing settings page in `src-app/ui/src/modules/`.
EOF
write_common "$FD" "src-app/ui/src/modules/foo/FooPage.tsx" 8

# --- variant A: all-unit test plan (NO e2e) -> phase 3 must FAIL
cat > "$FD/TESTS.md" <<'EOF'
# TESTS — foo
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/foo/FooPage.test.tsx` — asserts: FooPage renders a Save button.
EOF
git -C "$FE" add -A && git -C "$FE" commit -qm feat
assert_exit 1 "FE phase 3: all-unit plan for UI work is REFUSED" -- --phase 3 --repo "$FE" --dir "$FD" --base main

# --- variant B: plan now enumerates an e2e-tier test -> phase 3 OK
cat > "$FD/TESTS.md" <<'EOF'
# TESTS — foo
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/foo/FooPage.test.tsx` — asserts: FooPage renders a Save button.
- **TEST-2** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/foo/foo.spec.ts` — asserts: user opens Foo page and clicks Save.
EOF
git -C "$FE" add -A && git -C "$FE" commit -qm tests-e2e
assert_exit 0 "FE phase 3: UI plan WITH an e2e-tier test passes" -- --phase 3 --repo "$FE" --dir "$FD" --base main

# --- variant C: phase 8 results WITHOUT npm-run-check / e2e lines -> FAIL
cat > "$FD/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — foo
- **TEST-1**: PASS
- **TEST-2**: PASS
EOF
git -C "$FE" add -A && git -C "$FE" commit -qm results-missing-fe-lines
assert_exit 1 "FE phase 8: missing 'npm run check (ui): PASS' is REFUSED" -- --phase 8 --repo "$FE" --dir "$FD" --base main

# --- variant C2: npm-run-check present but the e2e spec did not pass -> FAIL
cat > "$FD/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — foo
- **TEST-1**: PASS
- **TEST-2**: FAIL
npm run check (ui): PASS
EOF
git -C "$FE" add -A && git -C "$FE" commit -qm results-e2e-fail
assert_exit 1 "FE phase 8: a failing e2e spec is REFUSED" -- --phase 8 --repo "$FE" --dir "$FD" --base main

# --- variant D: full frontend results -> phase 8 OK, and --all OK
cat > "$FD/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — foo
- **TEST-1**: PASS
- **TEST-2**: PASS
npm run check (ui): PASS
EOF
git -C "$FE" add -A && git -C "$FE" commit -qm results-complete
assert_exit 0 "FE phase 8: npm-run-check + e2e PASS lines accepted" -- --phase 8 --repo "$FE" --dir "$FD" --base main
assert_exit 0 "FE --all: complete frontend lifecycle is green" -- --all --repo "$FE" --dir "$FD" --base main

# ---------------------------------------------------------------------------
# FIXTURE 2 — BACKEND-ONLY feature that ALSO regenerates the client.
# The generated ui/ artifacts must NOT trigger the frontend gates; an all-unit
# + integration plan (no e2e) must PASS, and results need no npm-run-check line.
# ---------------------------------------------------------------------------
BE="$(new_repo)"
git -C "$BE" checkout -q -b feat/bar
mkdir -p "$BE/src-app/server/src/modules/bar" "$BE/src-app/ui/src/api-client" "$BE/.lifecycle/bar"
cat > "$BE/src-app/server/src/modules/bar/repository.rs" <<'EOF'
pub fn list_bar() -> Vec<String> {
    vec!["a".into(), "b".into()]
}
EOF
# regenerated client types — generated, must be excluded from touch detection
echo 'export type Bar = { id: string };' > "$BE/src-app/ui/src/api-client/types.ts"

BD="$BE/.lifecycle/bar"
cat > "$BD/PLAN.md" <<'EOF'
# PLAN — bar
## Items
- **ITEM-1**: Add list_bar to the bar repository.
## Files to touch
- `src-app/server/src/modules/bar/repository.rs` — new fn (ITEM-1).
- `src-app/ui/src/api-client/types.ts` — regenerated (excluded from gates).
## Patterns to follow
- Mirror an existing server repository module.
EOF
write_common "$BD" "src-app/server/src/modules/bar/repository.rs" 3
cat > "$BD/TESTS.md" <<'EOF'
# TESTS — bar
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/bar/repository.rs` — asserts: list_bar returns two rows.
EOF
cat > "$BD/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — bar
- **TEST-1**: PASS
EOF
git -C "$BE" add -A && git -C "$BE" commit -qm feat-bar
assert_exit 0 "BE phase 3: backend-only + regen-client plan needs NO e2e" -- --phase 3 --repo "$BE" --dir "$BD" --base main
assert_exit 0 "BE phase 8: backend-only results need NO npm-run-check line" -- --phase 8 --repo "$BE" --dir "$BD" --base main
assert_exit 0 "BE --all: backend-only lifecycle is green" -- --all --repo "$BE" --dir "$BD" --base main

rm -rf "$FE" "$BE"
echo "== $PASS passed, $FAIL failed =="
[ "$FAIL" -eq 0 ]
