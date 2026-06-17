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

# POST/GET that PRESERVE the response body + HTTP status even on 4xx/5xx.
# Deliberately NOT `-f`: that flag makes curl discard the error body, which is
# exactly what used to turn every install failure into an undebuggable empty
# "install FAILED:" line. Output is the body with the numeric status code on a
# trailing line; callers split with ${var##*$'\n'} (status) / ${var%$'\n'*} (body).
api_post() {
  curl -sS -X POST "${API}$1" "${AUTH[@]}" \
    -H 'Content-Type: application/json' -d "$2" \
    -w $'\n%{http_code}' 2>&1 || true
}
api_get() {
  curl -sS "${API}$1" "${AUTH[@]}" -w $'\n%{http_code}' 2>&1 || true
}

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

# ── 2. bootstrap the first ADMIN + obtain a token ────────────────────────
# install-from-hub requires the admin-only `workflows::install` /
# `skills::install` permission, so the smoke user must be a real admin —
# i.e. an Administrators-group member. On a fresh DB that comes from the
# one-time setup flow (POST /app/setup/admin), NOT /auth/register (which
# creates a regular Users-group member that gets 403 on install). On a re-run
# (admin already exists) fall back to login.
needs_setup="$(curl -fsS "${API}/app/setup/status" 2>/dev/null | jq -r '.needs_setup // false' 2>/dev/null || echo false)"
token=""
if [ "$needs_setup" = "true" ]; then
  log "fresh deployment — creating the first admin via /app/setup/admin"
  setup_resp="$(curl -sS -X POST "${API}/app/setup/admin" \
    -H 'Content-Type: application/json' \
    -d "{\"username\":\"${ADMIN_USER}\",\"email\":\"${ADMIN_EMAIL}\",\"password\":\"${ADMIN_PASS}\"}" \
    2>/dev/null || true)"
  token="$(printf '%s' "$setup_resp" | jq -r '.token // .access_token // empty' 2>/dev/null || true)"
fi
if [ -z "$token" ]; then
  log "admin already exists (or setup returned no token); logging in as ${ADMIN_USER}"
  login_resp="$(curl -fsS -X POST "${API}/auth/login" \
    -H 'Content-Type: application/json' \
    -d "{\"username\":\"${ADMIN_USER}\",\"password\":\"${ADMIN_PASS}\"}" 2>/dev/null || true)"
  token="$(printf '%s' "$login_resp" | jq -r '.token // .access_token // empty' 2>/dev/null || true)"
fi
[ -n "$token" ] || fail "could not obtain an admin auth token"
AUTH=(-H "Authorization: Bearer ${token}")
log "authenticated as admin ${ADMIN_USER}"

# ── 3. pull the live hub index ───────────────────────────────────────────
log "fetching hub index from ${HUB_BASE}/index.json"
curl -fsSL "${HUB_BASE}/index.json" -o "${WORKDIR}/index.json" \
  || fail "could not fetch hub index"

mapfile -t WF_NAMES < <(jq -r '.items[] | select(.category == "workflow") | .name' "${WORKDIR}/index.json")
mapfile -t SKILL_NAMES < <(jq -r '.items[] | select(.category == "skill") | .name' "${WORKDIR}/index.json")
log "found ${#WF_NAMES[@]} workflow(s) + ${#SKILL_NAMES[@]} skill(s) in the hub catalog"
# Graceful skip when the live hub publishes neither yet. This is the
# expected state on the PR that INTRODUCES them (the hub PR hasn't merged +
# Pages rebuilt yet) — nothing to drift-check, so the detector passes. Once
# the hub publishes entries, subsequent runs (nightly + PRs) exercise them.
if [ "${#WF_NAMES[@]}" -eq 0 ] && [ "${#SKILL_NAMES[@]}" -eq 0 ]; then
  log "OK — live hub catalog lists zero workflows + zero skills; nothing to drift-check (skipping)"
  exit 0
fi

failures=0

# ── 4. install + test each workflow ──────────────────────────────────────
for name in "${WF_NAMES[@]}"; do
  log "=== workflow: ${name} ==="

  install_raw="$(api_post "/workflows/install-from-hub" "{\"hub_id\":\"${name}\"}")"
  install_code="${install_raw##*$'\n'}"
  install_resp="${install_raw%$'\n'*}"

  wf_id="$(printf '%s' "$install_resp" | jq -r '.id // .workflow.id // empty' 2>/dev/null || true)"
  if [ -z "$wf_id" ]; then
    log "  install FAILED (HTTP ${install_code}): ${install_resp:0:600}"
    failures=$((failures + 1))
    continue
  fi
  log "  installed → ${wf_id}"

  test_raw="$(api_post "/workflows/${wf_id}/test" '{}')"
  test_resp="${test_raw%$'\n'*}"

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

# ── 4b. install each skill + verify the row was created ──────────────────
# Skills don't execute (no /test endpoint), so the smoke is: install from
# the live hub, then confirm the skill now appears in the user's list.
for name in "${SKILL_NAMES[@]}"; do
  log "=== skill: ${name} ==="

  install_raw="$(api_post "/skills/install-from-hub" "{\"hub_id\":\"${name}\"}")"
  install_code="${install_raw##*$'\n'}"
  install_resp="${install_raw%$'\n'*}"

  skill_id="$(printf '%s' "$install_resp" | jq -r '.skill.id // .id // empty' 2>/dev/null || true)"
  if [ -z "$skill_id" ]; then
    log "  install FAILED (HTTP ${install_code}): ${install_resp:0:600}"
    failures=$((failures + 1))
    continue
  fi
  log "  installed → ${skill_id}"

  list_raw="$(api_get "/skills")"
  list_resp="${list_raw%$'\n'*}"
  if printf '%s' "$list_resp" | jq -e --arg id "$skill_id" \
       '.skills[]? | select(.id == $id)' >/dev/null 2>&1; then
    log "  verified in GET /skills"
  else
    log "  installed skill ${skill_id} not found in GET /skills"
    failures=$((failures + 1))
  fi
done

# ── 5. verdict ───────────────────────────────────────────────────────────
if [ "$failures" -ne 0 ]; then
  fail "${failures} hub item(s) failed install or test against this ziee build"
fi
log "OK — all ${#WF_NAMES[@]} hub workflow(s) + ${#SKILL_NAMES[@]} hub skill(s) installed cleanly"
