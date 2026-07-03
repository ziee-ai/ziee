#!/usr/bin/env bash
# ============================================================================
# load.sh — import the showcase conversation into a dev DB + place file bytes.
# ============================================================================
# Idempotent: safe to re-run (fixed UUIDs + ON CONFLICT DO NOTHING in the SQL,
# and a plain overwrite for the on-disk file bytes).
#
# What it does:
#   1. Resolves the target DB (DATABASE_URL env, else the embedded dev PG).
#   2. Resolves the OWNER user (OWNER env, else the root admin: is_admin=true).
#   3. Generates the file assets if missing (generate_files.py).
#   4. Copies each asset into the file store:  <FILES_DIR>/originals/<owner>/<id>.<ext>
#   5. Runs showcase.sql with -v owner=<uuid>.
#
# Requirements: psql, python3 (+ Pillow & openpyxl for asset generation),
# and a server that has BOOTED against this DB at least once (so the built-in
# mcp_servers rows referenced by tool_use blocks exist) and a completed
# first-run setup (so a root admin user exists).
#
# Env overrides:
#   DATABASE_URL   full libpq URL of the target DB
#   OWNER          user UUID to own the conversation (skips the is_admin lookup)
#   FILES_DIR      the server's <app_data>/files dir (holds originals/, text/, ...)
# ============================================================================
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVER_DIR="$(cd "$HERE/../.." && pwd)"   # src-app/server

# ---- 1. target DB ----------------------------------------------------------
# Default: embedded dev Postgres from config/dev.example.yaml (port 54323).
DB_URL="${DATABASE_URL:-postgresql://postgres:password@127.0.0.1:54323/postgres}"
echo "==> target DB: ${DB_URL%%\?*}"

psql_do() { psql "$DB_URL" -v ON_ERROR_STOP=1 -qtAX "$@"; }

# ---- 2. owner user ---------------------------------------------------------
if [[ -n "${OWNER:-}" ]]; then
  OWNER_ID="$OWNER"
else
  OWNER_ID="$(psql_do -c "SELECT id FROM users WHERE is_admin = true LIMIT 1;")"
fi
if [[ -z "${OWNER_ID:-}" ]]; then
  echo "ERROR: no owner user found. Complete first-run setup (creates the root admin)" >&2
  echo "       or pass OWNER=<user-uuid>." >&2
  exit 1
fi
echo "==> owner user: $OWNER_ID"

# ---- 3. generate assets if missing -----------------------------------------
if [[ ! -f "$HERE/files/chart.png" ]]; then
  echo "==> generating file assets"
  python3 "$HERE/generate_files.py"
fi

# ---- 4. copy bytes into the file store -------------------------------------
# FILE_MAP: "<basename>:<file_id>.<ext>"  (ext must match files.mime -> get_original_path)
FILE_MAP=(
  "chart.png:f1000000-0000-0000-0000-000000000001.png"
  "photo.jpg:f1000000-0000-0000-0000-000000000002.jpg"
  "workbook.xlsx:f1000000-0000-0000-0000-000000000003.xlsx"
  "data.csv:f1000000-0000-0000-0000-000000000004.csv"
  "report.pdf:f1000000-0000-0000-0000-000000000005.pdf"
  "script.py:f1000000-0000-0000-0000-000000000006.py"
  "notes.md:f1000000-0000-0000-0000-000000000007.md"
  "large.txt:f1000000-0000-0000-0000-000000000008.txt"
)

# Resolve the file store dir. Default mirrors file/mod.rs: <app_data>/files, and
# dev.example.yaml's app.data_dir = ../../ziee-data/dev/app-data (rel to server).
FILES_DIR="${FILES_DIR:-$SERVER_DIR/../../ziee-data/dev/app-data/files}"
ORIG_DIR="$FILES_DIR/originals/$OWNER_ID"
mkdir -p "$ORIG_DIR"
echo "==> copying assets into $ORIG_DIR"
for entry in "${FILE_MAP[@]}"; do
  src="$HERE/files/${entry%%:*}"
  dst="$ORIG_DIR/${entry##*:}"
  cp -f "$src" "$dst"
  echo "    ${entry%%:*} -> ${entry##*:}"
done

# ---- 5. run the seed SQL ---------------------------------------------------
echo "==> running showcase.sql"
psql "$DB_URL" -v ON_ERROR_STOP=1 -v owner="$OWNER_ID" -f "$HERE/showcase.sql"

echo ""
echo "==> done. Conversation id: 11111111-1111-1111-1111-111111111111"
echo "    Open it in the chat UI (owned by user $OWNER_ID) to eyeball rendering."
