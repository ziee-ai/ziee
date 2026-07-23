#!/usr/bin/env bash
#
# Manual-test kit for the agent-core chat send-loop (Phase C).
#
# Builds + runs the ziee server (with ZIEE_CHAT_AGENT_CORE on OR off) + the UI,
# so Khoi can exercise the ON path live and compare it side-by-side with legacy.
#
#   scripts/manual-test-agent-core.sh on      # start with ZIEE_CHAT_AGENT_CORE=1 (agent-core)
#   scripts/manual-test-agent-core.sh off     # start with the flag unset (legacy — the default)
#   scripts/manual-test-agent-core.sh status  # show what's running + which flag
#   scripts/manual-test-agent-core.sh stop     # stop the server + UI
#
# The DB persists across restarts, so to compare OFF vs ON on the SAME
# conversation: run `on`, exercise it, `stop`, run `off`, re-open the same
# conversation. (This is the FLAG toggle, not a data reset.)
#
# GUARDRAIL: this is a LOCAL manual-test harness. It does NOT change the shipped
# default (the flag stays opt-in until Khoi signs off) — it only sets the env var
# for THIS local process.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$REPO_ROOT/src-app"
SERVER_DIR="$APP/server"
UI_DIR="$APP/ui"
RUN_DIR="$REPO_ROOT/.manual-test"
SERVER_LOG="$RUN_DIR/server.log"
UI_LOG="$RUN_DIR/ui.log"
SERVER_PIDF="$RUN_DIR/server.pid"
UI_PIDF="$RUN_DIR/ui.pid"
FLAGF="$RUN_DIR/flag"
BACKEND_PORT=3000
UI_PORT=5173

mkdir -p "$RUN_DIR"

log()  { printf '\033[1;36m[manual-test]\033[0m %s\n' "$*"; }
err()  { printf '\033[1;31m[manual-test] ERROR:\033[0m %s\n' "$*" >&2; }

wait_for_port() { # host port label timeout_s
  local host=$1 port=$2 label=$3 timeout=${4:-180} start; start=$(date +%s)
  log "waiting for $label on $host:$port ..."
  while :; do
    if curl -s -o /dev/null -m 2 "http://$host:$port/" 2>/dev/null; then
      log "$label is up (http://$host:$port)"; return 0; fi
    if [ $(( $(date +%s) - start )) -ge "$timeout" ]; then
      err "$label did not come up within ${timeout}s — see the log"; return 1; fi
    sleep 2
  done
}

is_running() { local pidf=$1; [ -f "$pidf" ] && kill -0 "$(cat "$pidf")" 2>/dev/null; }

do_stop() {
  for pair in "UI:$UI_PIDF" "server:$SERVER_PIDF"; do
    local name=${pair%%:*} pidf=${pair#*:}
    if is_running "$pidf"; then
      local pid; pid=$(cat "$pidf")
      log "stopping $name (pid $pid)"
      # kill the whole process group (cargo run / vite spawn children)
      kill -TERM -- "-$(ps -o pgid= "$pid" | tr -d ' ')" 2>/dev/null || kill "$pid" 2>/dev/null || true
    fi
    rm -f "$pidf"
  done
  rm -f "$FLAGF"
  log "stopped."
}

do_status() {
  if is_running "$SERVER_PIDF"; then
    log "server RUNNING (pid $(cat "$SERVER_PIDF")), flag: $(cat "$FLAGF" 2>/dev/null || echo '?')"
  else log "server: stopped"; fi
  if is_running "$UI_PIDF"; then log "UI RUNNING (pid $(cat "$UI_PIDF")) → http://localhost:$UI_PORT"
  else log "UI: stopped"; fi
}

do_start() { # mode = on|off
  local mode=$1
  if is_running "$SERVER_PIDF" || is_running "$UI_PIDF"; then
    err "already running — run '$0 stop' first."; do_status; exit 1; fi

  # Phase-1 gate: seeds config/dev.yaml (jwt secret), checks hub-seed / DB / node_modules.
  if [ -x "$REPO_ROOT/.claude/lifecycle/preflight.sh" ]; then
    log "running preflight (auto-seeds config/dev.yaml, checks env) ..."
    bash "$REPO_ROOT/.claude/lifecycle/preflight.sh" --repo "$REPO_ROOT" || {
      err "preflight failed — fix the printed problem and retry."; exit 1; }
  else
    log "preflight not found; ensure $SERVER_DIR/config/dev.yaml exists with a real jwt.secret."
  fi

  local flag_env=()
  if [ "$mode" = "on" ]; then flag_env=(ZIEE_CHAT_AGENT_CORE=1); log "flag: ZIEE_CHAT_AGENT_CORE=1 (AGENT-CORE path)"
  else log "flag: unset (LEGACY path — the shipped default)"; fi
  echo "$mode" > "$FLAGF"

  # Build once up front so the port-wait doesn't race a cold compile.
  log "building server (cargo build) — first build can take a few minutes ..."
  ( cd "$SERVER_DIR" && CONFIG_FILE=config/dev.yaml cargo build ) 2>&1 | tail -3

  log "starting server on :$BACKEND_PORT (log: $SERVER_LOG)"
  ( cd "$SERVER_DIR" && exec env "${flag_env[@]}" CONFIG_FILE=config/dev.yaml cargo run ) \
    > "$SERVER_LOG" 2>&1 &
  echo $! > "$SERVER_PIDF"
  wait_for_port 127.0.0.1 "$BACKEND_PORT" "server" 300 || { err "server log tail:"; tail -20 "$SERVER_LOG"; exit 1; }

  # Confirm the flag actually took (the server logs which loop it uses per turn).
  log "server started. Verify the ON path by watching $SERVER_LOG during a chat send."

  log "starting UI (npm run dev) on :$UI_PORT (log: $UI_LOG)"
  ( cd "$UI_DIR" && exec npm run dev ) > "$UI_LOG" 2>&1 &
  echo $! > "$UI_PIDF"
  wait_for_port 127.0.0.1 "$UI_PORT" "UI" 180 || { err "UI log tail:"; tail -20 "$UI_LOG"; exit 1; }

  cat <<EOF

  ────────────────────────────────────────────────────────────
   READY — open  http://localhost:$UI_PORT   (flag: $mode)
   server log: $SERVER_LOG
   ui log:     $UI_LOG
   Follow the checklist in .lifecycle/agent-core/MANUAL_TEST_PLAN.md
   Toggle: $0 stop  &&  $0 $([ "$mode" = on ] && echo off || echo on)
   Stop:   $0 stop
  ────────────────────────────────────────────────────────────
EOF
}

case "${1:-}" in
  on)     do_start on ;;
  off)    do_start off ;;
  stop)   do_stop ;;
  status) do_status ;;
  *) echo "usage: $0 {on|off|status|stop}"; exit 2 ;;
esac
