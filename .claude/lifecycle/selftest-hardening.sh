#!/usr/bin/env bash
# selftest-hardening.sh — verifies the LIFECYCLE-HARDENING additions:
#   • lifecycle-check.mjs A1/A2/A3/A4/A5/A7/A8/A9 deterministic gates
#   • merge-gate.mjs      C2 (migration collision) + C4 (stale branch) + clean
#   • preflight.sh        good env passes, missing-setup fails
#
# Each gate is proven BOTH ways: it PASSES a clean fixture and FAILS a
# seeded-bad one. Builds throwaway git repos; no network, no repo SHAs.
# Cross-platform: bash on Linux / macOS / Windows git-bash (git + node only).
#
#   bash .claude/lifecycle/selftest-hardening.sh
#
# Exit 0 = every gate behaved as specified.
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK="$HERE/lifecycle-check.mjs"
MG="$HERE/merge-gate.mjs"
PREFLIGHT="$HERE/preflight.sh"
# shellcheck source=selftest-lib.sh
. "$HERE/selftest-lib.sh"

lc()  { assert_exit_cmd "$1" "$2" -- node "$CHECK" "${@:3}"; }

CLEANUP=()
trap 'for d in "${CLEANUP[@]:-}"; do [ -n "$d" ] && rm -rf "$d"; done' EXIT

# ---------------------------------------------------------------------------
# build_be — a fully-valid BACKEND feature repo on branch feat/bar, committed
# clean. Echoes the repo root. The single source file is
# src-app/server/src/modules/bar/repository.rs (entirely diff-added vs main).
# ---------------------------------------------------------------------------
build_be() {
  local R; R="$(new_repo)"; CLEANUP+=("$R")
  git -C "$R" checkout -q -b feat/bar
  mkdir -p "$R/src-app/server/src/modules/bar" "$R/.lifecycle/bar"
  cat > "$R/src-app/server/src/modules/bar/repository.rs" <<'EOF'
pub fn list_bar() -> Vec<String> {
    vec!["a".into(), "b".into()]
}
EOF
  local D="$R/.lifecycle/bar"
  cat > "$D/PLAN.md" <<'EOF'
# PLAN — bar
## Items
- **ITEM-1**: Add list_bar to the bar repository.
## Files to touch
- `src-app/server/src/modules/bar/repository.rs` — new fn (ITEM-1).
## Patterns to follow
- Mirror an existing server repository module.
EOF
  write_common "$D" "src-app/server/src/modules/bar/repository.rs" 3
  cat > "$D/TESTS.md" <<'EOF'
# TESTS — bar
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/bar/repository.rs` — asserts: list_bar returns two rows.
EOF
  cat > "$D/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — bar
- **TEST-1**: PASS
EOF
  git -C "$R" add -A && git -C "$R" commit -qm feat-bar
  echo "$R"
}

# ---------------------------------------------------------------------------
# build_perm — a feature that INTRODUCES a user-facing permission (foo::use) in
# a modules/*/permissions.rs AND ships a UI surface gated by it. A9 (backend
# deny) is satisfied by TEST-2's 403; the A10 (frontend-hidden) test is left OFF
# so the caller can add it. Echoes the repo root; branch = feat/perm.
# ---------------------------------------------------------------------------
build_perm() {
  local R; R="$(new_repo)"; CLEANUP+=("$R")
  git -C "$R" checkout -q -b feat/perm
  mkdir -p "$R/src-app/server/src/modules/foo" "$R/src-app/ui/src/modules/foo" "$R/.lifecycle/foo"
  cat > "$R/src-app/server/src/modules/foo/permissions.rs" <<'EOF'
pub struct FooUse;
impl PermissionCheck for FooUse {
    const PERMISSION: &'static str = "foo::use";
}
EOF
  cat > "$R/src-app/ui/src/modules/foo/FooPage.tsx" <<'EOF'
export function FooPage() {
  return <div><h1>Foo</h1><button>Save</button></div>;
}
EOF
  local D="$R/.lifecycle/foo"
  cat > "$D/PLAN.md" <<'EOF'
# PLAN — foo
## Items
- **ITEM-1**: Define the foo::use permission (backend).
- **ITEM-2**: Add the FooPage UI, gated by foo::use.
## Files to touch
- `src-app/server/src/modules/foo/permissions.rs` — new perm (ITEM-1).
- `src-app/ui/src/modules/foo/FooPage.tsx` — new gated page (ITEM-2).
## Patterns to follow
- Mirror an existing permissions.rs and a settings page.
EOF
  write_common "$D" "src-app/server/src/modules/foo/permissions.rs" 5
  cat > "$D/TESTS.md" <<'EOF'
# TESTS — foo
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/foo/permissions.rs` — asserts: PERMISSION is foo::use.
- **TEST-2** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/foo/foo.rs` — asserts: a user lacking foo::use gets 403 forbidden.
- **TEST-3** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/foo/foo.spec.ts` — asserts: a permitted user opens Foo and clicks Save.
EOF
  cat > "$D/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — foo
- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
npm run check (ui): PASS
gate:ui (ui): PASS
EOF
  git -C "$R" add -A && git -C "$R" commit -qm feat-perm
  echo "$R"
}

# ---------------------------------------------------------------------------
# build_perm_desktop — a DESKTOP-ONLY feature that introduces foo::use ONLY in
# the desktop crate (desktop/tauri/**). The desktop app is single-admin, so A10
# (frontend-hidden) must NOT fire and NO restricted-user e2e is required.
# Backend-only diff (desktop/tauri) → phase 3 needs no e2e either.
# ---------------------------------------------------------------------------
build_perm_desktop() {
  local R; R="$(new_repo)"; CLEANUP+=("$R")
  git -C "$R" checkout -q -b feat/perm-desktop
  mkdir -p "$R/src-app/desktop/tauri/src/modules/foo" "$R/.lifecycle/foo"
  cat > "$R/src-app/desktop/tauri/src/modules/foo/permissions.rs" <<'EOF'
pub struct FooUse;
impl PermissionCheck for FooUse {
    const PERMISSION: &'static str = "foo::use";
}
EOF
  local D="$R/.lifecycle/foo"
  cat > "$D/PLAN.md" <<'EOF'
# PLAN — foo
## Items
- **ITEM-1**: Define the foo::use permission (desktop-only module).
## Files to touch
- `src-app/desktop/tauri/src/modules/foo/permissions.rs` — new perm (ITEM-1).
## Patterns to follow
- Mirror an existing desktop permissions.rs.
EOF
  write_common "$D" "src-app/desktop/tauri/src/modules/foo/permissions.rs" 5
  cat > "$D/TESTS.md" <<'EOF'
# TESTS — foo
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/foo/permissions.rs` — asserts: PERMISSION is foo::use.
- **TEST-2** (tier: integration) [covers: ITEM-1] file: `src-app/desktop/tauri/tests/foo/foo.rs` — asserts: a user lacking foo::use gets 403 forbidden.
EOF
  cat > "$D/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — foo
- **TEST-1**: PASS
- **TEST-2**: PASS
EOF
  git -C "$R" add -A && git -C "$R" commit -qm feat-perm-desktop
  echo "$R"
}

echo "== lifecycle-hardening self-test =="
echo "-- Part A: lifecycle-check.mjs A1-A9 --"

# --- control: a clean backend feature passes phase 8 (so a FAIL below is the A-check)
R="$(build_be)"; D="$R/.lifecycle/bar"
lc 0 "A-control: clean backend phase 8 passes" --phase 8 --repo "$R" --dir "$D" --base main

# --- A1: a SECOND .lifecycle feature dir on the branch -> --all FAILs (global)
R="$(build_be)"; D="$R/.lifecycle/bar"
mkdir -p "$R/.lifecycle/stray"
echo "# a second feature's plan that sneaked onto the branch" > "$R/.lifecycle/stray/PLAN.md"
git -C "$R" add -A && git -C "$R" commit -qm stray-dir
lc 1 "A1: two .lifecycle dirs is REFUSED even with explicit --dir" --all --repo "$R" --dir "$D" --base main
git -C "$R" rm -rq .lifecycle/stray && git -C "$R" commit -qm rm-stray
lc 0 "A1: one .lifecycle dir is accepted (control)" --all --repo "$R" --dir "$D" --base main

# --- A2: an uncommitted (dirty) working tree at phase 8 -> FAIL
R="$(build_be)"; D="$R/.lifecycle/bar"
echo "// stray uncommitted edit" >> "$R/src-app/server/src/modules/bar/repository.rs"
lc 1 "A2: dirty working tree at phase 8 is REFUSED" --phase 8 --repo "$R" --dir "$D" --base main

# --- A3: a diff-added #[ignore] -> phase 8 FAIL
R="$(build_be)"; D="$R/.lifecycle/bar"
printf '\n#[ignore]\nfn skipped_test() {}\n' >> "$R/src-app/server/src/modules/bar/repository.rs"
git -C "$R" commit -qam add-ignore
lc 1 "A3: a diff-added #[ignore] is REFUSED" --phase 8 --repo "$R" --dir "$D" --base main

# --- A4: a cosmetic assert!(true) -> phase 8 FAIL
R="$(build_be)"; D="$R/.lifecycle/bar"
printf '\nfn t() { assert!(true); }\n' >> "$R/src-app/server/src/modules/bar/repository.rs"
git -C "$R" commit -qam add-cosmetic
lc 1 "A4: a cosmetic assert!(true) is REFUSED" --phase 8 --repo "$R" --dir "$D" --base main

# --- A5: TESTS.md that dropped a previously-committed test -> phase 3 FAIL
R="$(build_be)"; D="$R/.lifecycle/bar"
# earlier commit had TEST-1 + TEST-2; now shrink to TEST-1
cat > "$D/TESTS.md" <<'EOF'
# TESTS — bar
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/bar/repository.rs` — asserts: one.
- **TEST-2** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/bar/bar.rs` — asserts: two.
EOF
git -C "$R" commit -qam tests-two
cat > "$D/TESTS.md" <<'EOF'
# TESTS — bar
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/bar/repository.rs` — asserts: one.
EOF
git -C "$R" commit -qam tests-shrunk
lc 1 "A5: TESTS.md shrink (dropped TEST-2) is REFUSED" --phase 3 --repo "$R" --dir "$D" --base main

# --- A8: a built-in MCP server without BOTH mcp.rs edits -> FAIL; with both -> PASS
R="$(build_be)"; D="$R/.lifecycle/bar"
printf '\nfn bar_mcp_server_id() -> u32 { 1 }\n' >> "$R/src-app/server/src/modules/bar/repository.rs"
git -C "$R" commit -qam add-mcp
lc 1 "A8: built-in MCP w/o auto_attach_builtin_ids+is_builtin_server_id is REFUSED" --phase 8 --repo "$R" --dir "$D" --base main
printf '// wires auto_attach_builtin_ids + is_builtin_server_id\n' >> "$R/src-app/server/src/modules/bar/repository.rs"
git -C "$R" commit -qam add-mcp-wiring
lc 0 "A8: built-in MCP WITH both mcp.rs edits passes" --phase 8 --repo "$R" --dir "$D" --base main

# --- A9: a new permission without a DENY test -> FAIL; with a 403 test -> PASS
R="$(build_be)"; D="$R/.lifecycle/bar"
printf '\nconst PERMISSION: &str = "bar::use";\n' >> "$R/src-app/server/src/modules/bar/repository.rs"
git -C "$R" commit -qam add-perm
lc 1 "A9: new permission without a deny/403 test is REFUSED" --phase 8 --repo "$R" --dir "$D" --base main
cat >> "$D/TESTS.md" <<'EOF'
- **TEST-2** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/bar/bar.rs` — asserts: a user lacking bar::use gets 403 forbidden.
EOF
cat >> "$D/TEST_RESULTS.md" <<'EOF'
- **TEST-2**: PASS
EOF
git -C "$R" commit -qam add-deny-test
lc 0 "A9: new permission WITH a 403 deny test passes" --phase 8 --repo "$R" --dir "$D" --base main

# --- A10: a new user-facing permission (::use/::read/::manage) needs a
#     RESTRICTED-USER e2e (frontend-hidden), not only the A9 backend deny.

# A10-1: perm introduced in permissions.rs, NO [negative-perm] e2e -> phase 3 + 8 FAIL
R="$(build_perm)"; D="$R/.lifecycle/foo"
lc 1 "A10: new ::use perm without a restricted-user e2e is REFUSED (phase 3)" --phase 3 --repo "$R" --dir "$D" --base main
lc 1 "A10: new ::use perm without a restricted-user e2e is REFUSED (phase 8)" --phase 8 --repo "$R" --dir "$D" --base main

# A10-2: a [negative-perm] tag on a NON-e2e (integration) test does NOT satisfy
#        A10 — a 403/deny test is A9; the frontend proof MUST be tier: e2e.
R="$(build_perm)"; D="$R/.lifecycle/foo"
cat >> "$D/TESTS.md" <<'EOF'
- **TEST-4** (tier: integration) [negative-perm] [covers: ITEM-1] file: `src-app/server/tests/foo/foo.rs` — asserts: 403 without foo::use.
EOF
cat >> "$D/TEST_RESULTS.md" <<'EOF'
- **TEST-4**: PASS
EOF
git -C "$R" commit -qam mistagged-integration
lc 1 "A10: a [negative-perm] tag on a NON-e2e test does NOT satisfy A10" --phase 8 --repo "$R" --dir "$D" --base main

# A10-3: add the restricted-user e2e -> phase 3 + 8 PASS (A9 backend + A10 frontend both present)
R="$(build_perm)"; D="$R/.lifecycle/foo"
cat >> "$D/TESTS.md" <<'EOF'
- **TEST-4** (tier: e2e) [negative-perm] [covers: ITEM-2] file: `src-app/ui/tests/e2e/foo/perm-gating.spec.ts` — asserts: a user LACKING foo::use sees NO Foo nav entry, page, or Save button.
EOF
cat >> "$D/TEST_RESULTS.md" <<'EOF'
- **TEST-4**: PASS
EOF
git -C "$R" commit -qam add-negperm-e2e
lc 0 "A10: new ::use perm WITH a restricted-user e2e passes (phase 3)" --phase 3 --repo "$R" --dir "$D" --base main
lc 0 "A10: new ::use perm WITH a restricted-user e2e passes (phase 8)" --phase 8 --repo "$R" --dir "$D" --base main

# A10-DESKTOP: a ::use perm introduced ONLY in the desktop crate (single-admin app)
# is EXEMPT — no restricted-user e2e required (there is no non-admin user to hide from).
R="$(build_perm_desktop)"; D="$R/.lifecycle/foo"
lc 0 "A10: a desktop-only ::use perm is EXEMPT (no restricted-user e2e; phase 3)" --phase 3 --repo "$R" --dir "$D" --base main
lc 0 "A10: a desktop-only ::use perm is EXEMPT (no restricted-user e2e; phase 8)" --phase 8 --repo "$R" --dir "$D" --base main
# Guard: a perm ALSO introduced in the server crate is NOT desktop-only → A10 still fires.
mkdir -p "$R/src-app/server/src/modules/foo"
cat > "$R/src-app/server/src/modules/foo/permissions.rs" <<'EOF'
pub struct FooRead;
impl PermissionCheck for FooRead {
    const PERMISSION: &'static str = "foo::read";
}
EOF
git -C "$R" add -A && git -C "$R" commit -qm add-server-perm
lc 1 "A10: a perm ALSO in the server crate is NOT exempt (still REFUSED, phase 8)" --phase 8 --repo "$R" --dir "$D" --base main

# A10-4: the restricted-user e2e is enumerated but its RESULT is FAIL -> phase 8 FAIL
R="$(build_perm)"; D="$R/.lifecycle/foo"
cat >> "$D/TESTS.md" <<'EOF'
- **TEST-4** (tier: e2e) [negative-perm] [covers: ITEM-2] file: `src-app/ui/tests/e2e/foo/perm-gating.spec.ts` — asserts: a user LACKING foo::use sees no Foo UI.
EOF
cat >> "$D/TEST_RESULTS.md" <<'EOF'
- **TEST-4**: FAIL
EOF
git -C "$R" commit -qam negperm-e2e-fails
lc 1 "A10: an enumerated-but-FAILING restricted-user e2e is REFUSED (phase 8)" --phase 8 --repo "$R" --dir "$D" --base main

# A10-5: a permission GRANTED IN A MIGRATION (no permissions.rs const, so A9
#        does NOT fire) still requires a restricted-user e2e — A10 catches what
#        A9 misses.
R="$(new_repo)"; CLEANUP+=("$R")
mkdir -p "$R/src-app/server/migrations"
echo "CREATE TABLE a();" > "$R/src-app/server/migrations/00000000000010_a.sql"
git -C "$R" add -A && git -C "$R" commit -qm mig-10
git -C "$R" checkout -q -b feat/permmig
mkdir -p "$R/.lifecycle/foo"
cat > "$R/src-app/server/migrations/00000000000011_grant_foo.sql" <<'EOF'
-- grant foo::use to the default Users group (mirrors migration 98)
UPDATE groups SET permissions = array_append(permissions, 'foo::use') WHERE name = 'Users';
EOF
D="$R/.lifecycle/foo"
cat > "$D/PLAN.md" <<'EOF'
# PLAN — foo
## Items
- **ITEM-1**: Grant foo::use to the default Users group (migration).
## Files to touch
- `src-app/server/migrations/00000000000011_grant_foo.sql` — grant (ITEM-1).
## Patterns to follow
- Mirror migration 98 (idempotent grant).
EOF
write_common "$D" "src-app/server/migrations/00000000000011_grant_foo.sql" 2
cat > "$D/TESTS.md" <<'EOF'
# TESTS — foo
- **TEST-1** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/foo/foo.rs` — asserts: the Users group gains foo::use.
EOF
cat > "$D/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — foo
- **TEST-1**: PASS
EOF
git -C "$R" add -A && git -C "$R" commit -qm feat-permmig
lc 1 "A10: a migration granting ::use without a restricted-user e2e is REFUSED (A9 alone misses it)" --phase 8 --repo "$R" --dir "$D" --base main
cat >> "$D/TESTS.md" <<'EOF'
- **TEST-2** (tier: e2e) [negative-perm] [covers: ITEM-1] file: `src-app/ui/tests/e2e/foo/perm-gating.spec.ts` — asserts: a user LACKING foo::use sees no Foo UI.
EOF
cat >> "$D/TEST_RESULTS.md" <<'EOF'
- **TEST-2**: PASS
EOF
git -C "$R" commit -qam add-negperm-mig
lc 0 "A10: the migration grant WITH a restricted-user e2e passes (phase 8)" --phase 8 --repo "$R" --dir "$D" --base main

# --- PLATFORM-SKIP: a [platform-skip]-tagged test (genuine #[cfg] platform gate)
#     may be SKIP instead of PASS at phase 8; an untagged SKIP still fails.
R="$(build_be)"; D="$R/.lifecycle/bar"
# Untagged SKIP → phase 8 FAIL (control).
cat >> "$D/TESTS.md" <<'EOF'
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/bar/other.rs` — asserts: a Linux-only path.
EOF
cat >> "$D/TEST_RESULTS.md" <<'EOF'
- **TEST-2**: SKIP
EOF
git -C "$R" commit -qam skip-untagged
lc 1 "PLATFORM-SKIP: an UNTAGGED SKIP result is REFUSED at phase 8" --phase 8 --repo "$R" --dir "$D" --base main
# Add the [platform-skip] tag → phase 8 accepts the SKIP.
perl -0pi -e 's/\*\*TEST-2\*\* \(tier: unit\)/**TEST-2** (tier: unit) [platform-skip]/' "$D/TESTS.md"
git -C "$R" commit -qam skip-tagged
lc 0 "PLATFORM-SKIP: a [platform-skip]-tagged SKIP is ACCEPTED at phase 8" --phase 8 --repo "$R" --dir "$D" --base main
# A [platform-skip] test that is FAIL (not SKIP) still fails — the tag only relaxes SKIP.
perl -0pi -e 's/\*\*TEST-2\*\*: SKIP/**TEST-2**: FAIL/' "$D/TEST_RESULTS.md"
git -C "$R" commit -qam skip-tagged-but-fail
lc 1 "PLATFORM-SKIP: a [platform-skip] test that is FAIL is still REFUSED" --phase 8 --repo "$R" --dir "$D" --base main

# --- A7: a UI diff whose results omit the boot/runtime canary -> phase 8 FAIL
R="$(new_repo)"; CLEANUP+=("$R")
git -C "$R" checkout -q -b feat/ui
mkdir -p "$R/src-app/ui/src/modules/foo" "$R/.lifecycle/foo"
cat > "$R/src-app/ui/src/modules/foo/FooPage.tsx" <<'EOF'
export function FooPage() {
  return <div><h1>Foo</h1><button>Save</button></div>;
}
EOF
D="$R/.lifecycle/foo"
cat > "$D/PLAN.md" <<'EOF'
# PLAN — foo
## Items
- **ITEM-1**: Add a FooPage component.
## Files to touch
- `src-app/ui/src/modules/foo/FooPage.tsx` — new page (ITEM-1).
## Patterns to follow
- Mirror an existing settings page.
EOF
write_common "$D" "src-app/ui/src/modules/foo/FooPage.tsx" 3
cat > "$D/TESTS.md" <<'EOF'
# TESTS — foo
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/foo/FooPage.test.tsx` — asserts: renders Save.
- **TEST-2** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/foo/foo.spec.ts` — asserts: user clicks Save.
EOF
cat > "$D/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — foo
- **TEST-1**: PASS
- **TEST-2**: PASS
npm run check (ui): PASS
EOF
git -C "$R" add -A && git -C "$R" commit -qm feat-ui-no-canary
lc 1 "A7: UI results missing the boot/runtime canary line is REFUSED" --phase 8 --repo "$R" --dir "$D" --base main
printf 'gate:ui (ui): PASS\n' >> "$D/TEST_RESULTS.md"
git -C "$R" commit -qam add-canary
lc 0 "A7: UI results WITH gate:ui canary passes" --phase 8 --repo "$R" --dir "$D" --base main

# --- R2-5: an e2e route-mock pointing at a route not in openapi.json -> FAIL;
#     fixing it to a live route -> PASS. (Needs an openapi.json to check against.)
R="$(new_repo)"; CLEANUP+=("$R")
git -C "$R" checkout -q -b feat/mock
mkdir -p "$R/src-app/ui/openapi" "$R/src-app/ui/tests/e2e/foo" "$R/.lifecycle/foo"
echo '{"paths":{"/api/things":{"get":{}},"/api/things/{id}":{"get":{}}}}' > "$R/src-app/ui/openapi/openapi.json"
cat > "$R/src-app/ui/tests/e2e/foo/foo.spec.ts" <<'EOF'
import { test } from '@playwright/test';
test('foo', async ({ page }) => {
  await page.route('**/api/ghosts', (r) => r.fulfill({ json: [] }));
});
EOF
D="$R/.lifecycle/foo"
cat > "$D/PLAN.md" <<'EOF'
# PLAN — foo
## Items
- **ITEM-1**: Add an e2e spec for the things flow.
## Files to touch
- `src-app/ui/tests/e2e/foo/foo.spec.ts` — new spec (ITEM-1).
## Patterns to follow
- Mirror an existing e2e spec.
EOF
write_common "$D" "src-app/ui/tests/e2e/foo/foo.spec.ts" 5
cat > "$D/TESTS.md" <<'EOF'
# TESTS — foo
- **TEST-1** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/foo/foo.spec.ts` — asserts: things list renders.
EOF
cat > "$D/TEST_RESULTS.md" <<'EOF'
# TEST_RESULTS — foo
- **TEST-1**: PASS
npm run check (ui): PASS
gate:ui (ui): PASS
EOF
git -C "$R" add -A && git -C "$R" commit -qm feat-mock-ghost
lc 1 "R2-5: e2e mock of an unknown /api/ route is REFUSED" --phase 8 --repo "$R" --dir "$D" --base main
# fix the mock to a live route
sed 's#\*\*/api/ghosts#**/api/things#' "$R/src-app/ui/tests/e2e/foo/foo.spec.ts" > "$R/tmp.ts" && mv "$R/tmp.ts" "$R/src-app/ui/tests/e2e/foo/foo.spec.ts"
git -C "$R" commit -qam fix-mock
lc 0 "R2-5: e2e mock of a live /api/ route passes" --phase 8 --repo "$R" --dir "$D" --base main

# ---------------------------------------------------------------------------
echo "-- Part B: merge-gate.mjs (deterministic gates, --skip-heavy) --"
# ---------------------------------------------------------------------------
# build_mg <branch-migration-file> — main has migration 10; branch adds the
# given file. Echoes repo root; branch = feat/mig.
build_mg() {
  local branchmig="$1"; local R; R="$(new_repo)"; CLEANUP+=("$R")
  mkdir -p "$R/src-app/server/migrations"
  echo "CREATE TABLE a();" > "$R/src-app/server/migrations/00000000000010_a.sql"
  git -C "$R" add -A && git -C "$R" commit -qm mig-10
  git -C "$R" checkout -q -b feat/mig
  echo "CREATE TABLE b();" > "$R/src-app/server/migrations/$branchmig"
  git -C "$R" add -A && git -C "$R" commit -qm branch-mig
  echo "$R"
}

# clean: branch migration 11 > main max 10
R="$(build_mg 00000000000011_b.sql)"
assert_exit_cmd 0 "merge-gate: clean branch (mig 11 > 10) passes" -- \
  node "$MG" feat/mig --repo "$R" --base main --no-fetch --skip-heavy

# C2 collision: branch migration 09 <= main max 10
R="$(build_mg 00000000000009_early.sql)"
assert_exit_cmd 1 "merge-gate C2: migration <= main max is REFUSED" -- \
  node "$MG" feat/mig --repo "$R" --base main --no-fetch --skip-heavy

# C4 stale: main advances after fork; branch is behind; --max-behind 0
R="$(new_repo)"; CLEANUP+=("$R")
mkdir -p "$R/src-app/server/migrations"
echo "CREATE TABLE a();" > "$R/src-app/server/migrations/00000000000010_a.sql"
git -C "$R" add -A && git -C "$R" commit -qm mig-10
git -C "$R" checkout -q -b feat/stale
echo "x" > "$R/x.txt"; git -C "$R" add -A && git -C "$R" commit -qm branch-work
git -C "$R" checkout -q main
echo "y" > "$R/y.txt"; git -C "$R" add -A && git -C "$R" commit -qm main-advance
assert_exit_cmd 1 "merge-gate C4: a branch behind main (--max-behind 0) is REFUSED" -- \
  node "$MG" feat/stale --repo "$R" --base main --no-fetch --skip-heavy --max-behind 0

# ---------------------------------------------------------------------------
echo "-- Part C: preflight.sh (env gate) --"
# ---------------------------------------------------------------------------
# good: hub-seed + pgvector + node_modules present
GOOD="$(new_repo)"; CLEANUP+=("$GOOD")
mkdir -p "$GOOD/src-app/server/binaries/hub-seed" \
         "$GOOD/src-app/server/vendor/pgvector" \
         "$GOOD/node_modules"
echo '{"hub_version":"v0.0.0"}' > "$GOOD/src-app/server/binaries/hub-seed/index.json"
echo 'all:' > "$GOOD/src-app/server/vendor/pgvector/Makefile"
assert_exit_cmd 0 "preflight: fully-provisioned env passes" -- \
  env -u DATABASE_URL -u ZIEE_BUILD_DB_PERWORKTREE bash "$PREFLIGHT" --repo "$GOOD"

# bad: hub-seed missing (build.rs would PANIC)
BAD="$(new_repo)"; CLEANUP+=("$BAD")
assert_exit_cmd 1 "preflight: missing hub-seed/node_modules is REFUSED" -- \
  env -u DATABASE_URL -u ZIEE_BUILD_DB_PERWORKTREE bash "$PREFLIGHT" --repo "$BAD"

# ---------------------------------------------------------------------------
echo "-- Part D: merge-gate --verify-head (the pre-push-to-main hook guard) --"
# ---------------------------------------------------------------------------
# clean HEAD: one migration, no .lifecycle
R="$(new_repo)"; CLEANUP+=("$R")
mkdir -p "$R/src-app/server/migrations"
echo "CREATE TABLE a();" > "$R/src-app/server/migrations/00000000000010_a.sql"
git -C "$R" add -A && git -C "$R" commit -qm mig
assert_exit_cmd 0 "verify-head: clean HEAD passes" -- node "$MG" --verify-head --repo "$R"

# C5: HEAD still carries .lifecycle/ artifacts
R="$(new_repo)"; CLEANUP+=("$R")
mkdir -p "$R/.lifecycle/foo"
echo "# plan" > "$R/.lifecycle/foo/PLAN.md"
git -C "$R" add -A && git -C "$R" commit -qm leaked-lifecycle
assert_exit_cmd 1 "verify-head C5: leaked .lifecycle/ on HEAD is REFUSED" -- node "$MG" --verify-head --repo "$R"

# C2: HEAD has two migrations with the same number prefix
R="$(new_repo)"; CLEANUP+=("$R")
mkdir -p "$R/src-app/server/migrations"
echo "CREATE TABLE a();" > "$R/src-app/server/migrations/00000000000010_a.sql"
echo "CREATE TABLE b();" > "$R/src-app/server/migrations/00000000000010_b.sql"
git -C "$R" add -A && git -C "$R" commit -qm dup-mig
assert_exit_cmd 1 "verify-head C2: duplicate migration prefix on HEAD is REFUSED" -- node "$MG" --verify-head --repo "$R"

echo "== $PASS passed, $FAIL failed =="
[ "$FAIL" -eq 0 ]
