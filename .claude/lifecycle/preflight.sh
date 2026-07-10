#!/usr/bin/env bash
# preflight.sh — Phase-1 environment gate for the feature-lifecycle.
#
# Catches the recurring, high-cost SETUP friction that wasted time across dozens
# of sessions BEFORE any code was written. Each check fails LOUD with the exact
# fix. Run once when you cut a worktree (and any time the build acts strange):
#
#   bash .claude/lifecycle/preflight.sh [--repo <path>]
#
# Exit 0 = ready to build. Non-zero = a blocking setup problem (fix printed).
# Warnings (stale Vite) do not fail the gate.
#
# CROSS-PLATFORM: runs under bash on Linux, macOS, and Windows (git-bash — the
# same shell git uses to run the pre-push hook, so it is guaranteed present
# wherever this infra's hook works). Every check avoids GNU-only flags and
# guards Unix-only tools (pgrep/pg_isready) with `command -v`; fix hints call
# out the per-OS command where it differs.
set -u

# OS label for platform-specific fix hints (Darwin / Linux / MINGW*/MSYS* = Windows git-bash).
case "$(uname -s 2>/dev/null)" in
  Darwin) OSLABEL=macos ;;
  Linux)  OSLABEL=linux ;;
  MINGW*|MSYS*|CYGWIN*) OSLABEL=windows ;;
  *) OSLABEL=unknown ;;
esac

REPO=""
while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [ -z "$REPO" ]; then
  REPO="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "preflight: FATAL: not in a git repo (pass --repo)"; exit 2; }
fi

FAIL=0
ok()   { printf '  \033[32mok  \033[0m %s\n' "$*"; }
bad()  { printf '  \033[31mFAIL\033[0m %s\n' "$1"; printf '        fix: %s\n' "$2"; FAIL=$((FAIL+1)); }
warn() { printf '  \033[33mwarn\033[0m %s\n' "$1"; [ -n "${2:-}" ] && printf '        fix: %s\n' "$2"; }

echo "== feature-lifecycle preflight ($REPO) =="

# 1. hub-seed present — else the server build PANICS (hub_seed is fail-hard).  32 sessions.
SEED="$REPO/src-app/server/binaries/hub-seed"
if [ -f "$SEED/index.json" ]; then
  ok "hub-seed present ($SEED/index.json)"
else
  bad "hub-seed missing at $SEED/index.json — server build.rs PANICS without it" \
      "cp -r <a-clone-with-hub-seed>/src-app/server/binaries/hub-seed \"$SEED\"  (or: rm -rf \"$SEED\" && cargo check -p ziee to refetch from GitHub)"
fi

# 2. pgvector submodule initialized — else migration 46 (CREATE EXTENSION vector) + the memory build fail.
PGV="$REPO/src-app/server/vendor/pgvector"
if [ -f "$PGV/Makefile" ] || [ -f "$PGV/CHANGELOG.md" ] || [ -d "$PGV/src" ]; then
  ok "pgvector submodule initialized"
elif grep -q "pgvector" "$REPO/.gitmodules" 2>/dev/null; then
  bad "pgvector submodule NOT initialized ($PGV is empty)" \
      "git -C \"$REPO\" submodule update --init src-app/server/vendor/pgvector"
else
  ok "pgvector submodule not declared (skipping)"
fi

# 3. node_modules hoisted — npm workspaces hoist to the REPO ROOT; a missing/stale
#    tree gives phantom tsc errors (esp. on kit branches).
if [ -d "$REPO/node_modules" ]; then
  ok "root node_modules present (workspace hoist)"
else
  bad "root node_modules missing — tsc/lint will show phantom missing-module errors" \
      "cd \"$REPO\" && npm install   (hoists deps for both ui workspaces)"
fi
# Per-workspace node_modules is usually a symlink into the root hoist; a missing
# one is only a problem if that workspace has un-hoistable deps. Warn, don't block.
for ws in src-app/ui src-app/desktop/ui; do
  if [ -d "$REPO/$ws" ] && [ ! -e "$REPO/$ws/node_modules" ]; then
    warn "$ws/node_modules absent (fine if fully hoisted; re-run npm install if tsc shows phantom errors)" \
         "cd \"$REPO\" && npm install"
  fi
done

# 4. Per-worktree build-DB isolation — the shared :54321 build DB is wiped by
#    every concurrent build; without isolation two worktrees clobber each other.  60 sessions.
if [ "${ZIEE_BUILD_DB_PERWORKTREE:-1}" = "0" ]; then
  bad "ZIEE_BUILD_DB_PERWORKTREE=0 — per-worktree build-DB isolation is DISABLED (shared :54321 race)" \
      "unset ZIEE_BUILD_DB_PERWORKTREE   (auto-isolation keys the build DB by worktree path)"
elif [ -n "${DATABASE_URL:-}" ] && ! printf '%s' "$DATABASE_URL" | grep -qE '127\.0\.0\.1:54321|localhost:54321|sqlx-build-sentinel'; then
  # A genuine external override (different host:port, e.g. CI) is honored as-is.
  warn "DATABASE_URL is an external override ($DATABASE_URL) — build.rs will use it verbatim (fine for CI; NOT auto-isolated)" \
       "unset DATABASE_URL to use per-worktree auto-isolation when developing locally"
else
  ok "per-worktree build-DB isolation active (auto-keyed by worktree path)"
fi

# 5. build-DB cluster reachable (info) — the pgvector cluster on :54321.
if command -v pg_isready >/dev/null 2>&1; then
  if pg_isready -h 127.0.0.1 -p 54321 -q 2>/dev/null; then ok "build-DB cluster reachable on 127.0.0.1:54321"
  else warn "build-DB cluster NOT reachable on :54321 (build.rs will provision it, or start it)" \
            "cd \"$REPO/src-app\" && docker compose up -d"; fi
else
  warn "pg_isready not installed — cannot verify the :54321 build cluster (build.rs handles provisioning)" ""
fi

# 6. no stale Vite serving old code — a lingering dev server hides code changes in e2e.
#    Portable detection: `pgrep -f` works on Linux AND macOS (the `-a` flag does
#    NOT — it is procps/Linux-only, so we drop it); fall back to `ps` piped to
#    grep; skip silently if neither tool exists (Windows git-bash may lack both).
VITE_PIDS=""
if command -v pgrep >/dev/null 2>&1; then
  VITE_PIDS="$(pgrep -f 'vite --config' 2>/dev/null | head -5)"
elif command -v ps >/dev/null 2>&1; then
  VITE_PIDS="$(ps ax 2>/dev/null | grep '[v]ite --config' | awk '{print $1}' | head -5)"
fi
if [ -n "$VITE_PIDS" ]; then
  VITE_PIDS_ONELINE="$(printf '%s ' $VITE_PIDS)"
  case "$OSLABEL" in
    windows) VITE_FIX='Stop-Process -Id <pid>  (PowerShell), or `wsl --shutdown` if Vite runs in WSL2' ;;
    *)       VITE_FIX='pkill -f "vite --config"   (NOT killall node)' ;;
  esac
  warn "stale Vite dev server(s) running (PIDs: ${VITE_PIDS_ONELINE}) — may serve OLD code in e2e" "$VITE_FIX"
elif command -v pgrep >/dev/null 2>&1 || command -v ps >/dev/null 2>&1; then
  ok "no stale Vite dev servers"
else
  warn "cannot scan for stale Vite (no pgrep/ps on PATH) — if e2e serves stale code, kill the dev server manually" ""
fi

echo ""
if [ "$FAIL" -ne 0 ]; then
  echo "preflight: $FAIL blocking problem(s) — fix them before building."
  exit 1
fi
echo "preflight: OK — environment ready."
exit 0
