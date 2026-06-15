#!/usr/bin/env bash
# hub-smoke.sh — consumer-side hub-workflows drift detector.
#
# Symmetric to cargo's crater runs: the Rust side (the ziee workflow / hub /
# code_sandbox modules) verifies it doesn't break the live hub workflow
# ecosystem. Pulls the published hub catalog, installs every `workflow`
# entry into a freshly-built ziee, and runs each workflow's bundled `tests/`
# fixtures via `POST /api/workflows/{id}/test`. Any install or test failure
# exits non-zero.
#
# Invoked by .github/workflows/hub-smoke.yml AFTER ziee is built + started.
# Runnable locally too: build ziee, start it with code_sandbox.enabled=true,
# then `ZIEE_BASE=http://127.0.0.1:3000 scripts/hub-smoke.sh`.
#
# Env:
#   ZIEE_BASE   — base URL of the running ziee server (default http://127.0.0.1:3000)
#   HUB_BASE    — hub Pages base (default https://ziee-ai.github.io/hub)
#   ADMIN_USER / ADMIN_PASS / ADMIN_EMAIL — first-user (admin) bootstrap creds
#
# Requires: curl, jq.

set -euo pipefail

ZIEE_BASE="${ZIEE_BASE:-http://127.0.0.1:3000}"
HUB_BASE="${HUB_BASE:-https://ziee-ai.github.io/hub}"
API="${ZIEE_BASE}/api"
ADMIN_USER="${ADMIN_USER:-hubsmoke_admin}"
ADMIN_PASS="${ADMIN_PASS:-hub-smoke-passw0rd-please}"
ADMIN_EMAIL="${ADMIN_EMAIL:-hubsmoke@example.com}"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

log()  { printf '[hub-smoke] %s\n' "$*"; }
fail() { printf '[hub-smoke] FAIL: %s\n' "$*" >&2; exit 1; }

command -v curl >/dev/null || fail "curl is required"
command -v jq   >/dev/null || fail "jq is required"

# ── 1. wait for the server to accept connections ─────────────────────────
log "waiting for ziee at ${ZIEE_BASE} ..."
ready=0
for _ in $(seq 1 60); do
  if curl -fsS "${ZIEE_BASE}/" -o /dev/null 2>/dev/null \
     || curl -fsS "${API}/auth/providers" -o /dev/null 2>/dev/null; then
    ready=1; break
  fi
  sleep 2
done
[ "$ready" = 1 ] || fail "ziee did not become reachable within 120s"
log "ziee is up"

# ── 2. bootstrap the first user (becomes admin) + obtain a token ─────────
# First registered user is granted Administrators on a fresh DB. If the user
# already exists (re-run against a persistent DB), fall back to login.
reg_resp="$(curl -fsS -X POST "${API}/auth/register" \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"${ADMIN_USER}\",\"password\":\"${ADMIN_PASS}\",\"email\":\"${ADMIN_EMAIL}\"}" \
  2>/dev/null || true)"

token="$(printf '%s' "$reg_resp" | jq -r '.token // .access_token // empty' 2>/dev/null || true)"

if [ -z "$token" ]; then
  log "register returned no token; trying login"
  login_resp="$(curl -fsS -X POST "${API}/auth/login" \
    -H 'Content-Type: application/json' \
    -d "{\"username\":\"${ADMIN_USER}\",\"password\":\"${ADMIN_PASS}\"}")"
  token="$(printf '%s' "$login_resp" | jq -r '.token // .access_token // empty')"
fi
[ -n "$token" ] || fail "could not obtain an auth token"
AUTH=(-H "Authorization: Bearer ${token}")
log "authenticated as ${ADMIN_USER}"

# ── 3. pull the live hub index ───────────────────────────────────────────
log "fetching hub index from ${HUB_BASE}/index.json"
curl -fsSL "${HUB_BASE}/index.json" -o "${WORKDIR}/index.json" \
  || fail "could not fetch hub index"

mapfile -t WF_NAMES < <(jq -r '.items[] | select(.category == "workflow") | .name' "${WORKDIR}/index.json")
log "found ${#WF_NAMES[@]} workflow(s) in the hub catalog"
# Graceful skip when the live hub publishes no workflows yet. This is the
# expected state on the PR that INTRODUCES workflows (the hub PR hasn't
# merged + Pages rebuilt yet) — there's simply nothing to drift-check, so
# the detector passes. Once the hub publishes workflow entries, subsequent
# runs (nightly + PRs) exercise them for real.
if [ "${#WF_NAMES[@]}" -eq 0 ]; then
  log "OK — live hub catalog lists zero workflows; nothing to drift-check (skipping)"
  exit 0
fi

# ── 4. install + test each workflow ──────────────────────────────────────
failures=0
for name in "${WF_NAMES[@]}"; do
  log "=== ${name} ==="

  install_resp="$(curl -fsS -X POST "${API}/workflows/install-from-hub" \
    "${AUTH[@]}" -H 'Content-Type: application/json' \
    -d "{\"hub_id\":\"${name}\"}" 2>/dev/null || true)"

  wf_id="$(printf '%s' "$install_resp" | jq -r '.id // .workflow.id // empty' 2>/dev/null || true)"
  if [ -z "$wf_id" ]; then
    log "  install FAILED: ${install_resp:0:400}"
    failures=$((failures + 1))
    continue
  fi
  log "  installed → ${wf_id}"

  test_resp="$(curl -fsS -X POST "${API}/workflows/${wf_id}/test" \
    "${AUTH[@]}" -H 'Content-Type: application/json' -d '{}' \
    2>/dev/null || true)"

  passed="$(printf '%s' "$test_resp" | jq -r '.passed // empty' 2>/dev/null || true)"
  failed="$(printf '%s' "$test_resp" | jq -r '.failed // empty' 2>/dev/null || true)"

  if [ -z "$passed" ] && [ -z "$failed" ]; then
    log "  test endpoint returned an unexpected payload: ${test_resp:0:400}"
    failures=$((failures + 1))
    continue
  fi

  log "  test: passed=${passed:-0} failed=${failed:-0}"
  if [ "${failed:-0}" != "0" ]; then
    printf '%s' "$test_resp" | jq -r \
      '.results[]? | select(.passed == false) | "    x \(.name): \(.error // .mismatch // "assertion failed")"' \
      2>/dev/null || true
    failures=$((failures + 1))
  fi
done

# ── 5. verdict ───────────────────────────────────────────────────────────
if [ "$failures" -ne 0 ]; then
  fail "${failures} workflow(s) failed install or test against this ziee build"
fi
log "OK — all ${#WF_NAMES[@]} hub workflow(s) installed + passed their fixtures"
