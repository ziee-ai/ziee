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
#   6. PROCESSES the files through the live API so they get real thumbnails,
#      preview pages, and extracted text — see below.
#
# Why step 6: steps 4-5 only place original bytes + insert rows with
# has_thumbnail=false / preview_page_count=0. The file module's thumbnail +
# preview-page + text extraction runs on UPLOAD, and there's no reprocess
# endpoint. So step 6 uploads each asset through the live API (real pipeline),
# then grafts the produced artifacts + processing columns onto the seed's FIXED
# file ids (artifacts are keyed by file_id on disk) and deletes the temp upload.
# It is idempotent (skips a seed file already marked has_thumbnail) and degrades
# gracefully: if the server API isn't reachable or login fails it just warns and
# leaves the files raw (re-run with the server up to fill them in). Set
# SKIP_FILE_PROCESSING=1 to skip it entirely.
#
# Requirements: psql, python3 (+ Pillow & openpyxl for asset generation), curl,
# and a server that has BOOTED against this DB at least once (so the built-in
# mcp_servers rows referenced by tool_use blocks exist) and a completed
# first-run setup (so a root admin user exists). Step 6 additionally needs the
# server's HTTP API reachable + an admin login.
#
# Env overrides:
#   DATABASE_URL         full libpq URL of the target DB
#   OWNER                user UUID to own the conversation (skips is_admin lookup)
#   FILES_DIR            the server's <app_data>/files dir (originals/, text/, ...)
#   API_URL              server base URL for step 6 (default http://localhost:3000)
#   ADMIN_USERNAME       admin login for step 6 (default: admin)
#   ADMIN_PASSWORD       admin password for step 6 (default: Password123!)
#   ZIEE_ADMIN_TOKEN     a bearer token to use instead of logging in
#   SKIP_FILE_PROCESSING 1 = skip step 6 (files stay raw, no thumbnails/previews)
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

# ---- 6. process files through the real pipeline ----------------------------
# Upload each asset via the live API (runs thumbnail + preview-page + text
# extraction), then graft the produced artifacts + processing columns onto the
# seed's fixed file id and drop the temp upload. Best-effort: warns and skips if
# the server/login isn't available. errexit off for the block so one bad file
# (or an unreachable server) never aborts the whole seed.
API_URL="${API_URL:-http://localhost:3000}"
process_files() {
  set +e
  if [[ "${SKIP_FILE_PROCESSING:-0}" == "1" ]]; then
    echo "==> SKIP_FILE_PROCESSING=1 — files left raw (no thumbnails/previews)"; return
  fi
  if ! command -v curl >/dev/null 2>&1; then
    echo "==> WARN: curl not found — skipping file processing (files have no thumbnails/previews)"; return
  fi
  if ! curl -sf -o /dev/null "$API_URL/api/health" 2>/dev/null; then
    echo "==> WARN: server API not reachable at $API_URL — skipping file processing."
    echo "         Boot the server against this DB and re-run (or set API_URL) to fill in thumbnails/previews."
    return
  fi
  local token="${ZIEE_ADMIN_TOKEN:-}"
  if [[ -z "$token" ]]; then
    token="$(curl -s -X POST "$API_URL/api/auth/login" -H 'Content-Type: application/json' \
      -d "{\"username\":\"${ADMIN_USERNAME:-admin}\",\"password\":\"${ADMIN_PASSWORD:-Password123!}\"}" \
      | python3 -c 'import sys,json;print(json.load(sys.stdin).get("access_token",""))' 2>/dev/null)"
  fi
  if [[ -z "$token" ]]; then
    echo "==> WARN: admin login failed — skipping file processing."
    echo "         Set ADMIN_USERNAME/ADMIN_PASSWORD or ZIEE_ADMIN_TOKEN, then re-run."
    return
  fi
  # Uploader's user id = where the API writes NEW artifacts on disk (its own dir).
  local uploader
  uploader="$(curl -s "$API_URL/api/auth/me" -H "Authorization: Bearer $token" \
    | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d.get("id") or d.get("user",{}).get("id",""))' 2>/dev/null)"
  [[ -z "$uploader" ]] && uploader="$OWNER_ID"
  echo "==> processing files via $API_URL"
  local entry name seed new resp done_flag su du kind sd dd
  for entry in "${FILE_MAP[@]}"; do
    name="${entry%%:*}"; seed="${entry##*:}"; seed="${seed%.*}"
    done_flag="$(psql_do -c "SELECT has_thumbnail FROM files WHERE id='$seed';" 2>/dev/null)"
    if [[ "$done_flag" == "t" ]]; then echo "    $name -> already processed, skip"; continue; fi
    resp="$(curl -s -X POST "$API_URL/api/files/upload" -H "Authorization: Bearer $token" -F "file=@$HERE/files/$name")"
    new="$(echo "$resp" | python3 -c 'import sys,json;print(json.load(sys.stdin).get("id",""))' 2>/dev/null)"
    if [[ -z "$new" ]]; then echo "    WARN: upload failed for $name: ${resp:0:120}"; continue; fi
    # graft processing columns SEED <- NEW
    psql_do -c "UPDATE files s SET has_thumbnail=n.has_thumbnail, preview_page_count=n.preview_page_count, text_page_count=n.text_page_count, processing_metadata=n.processing_metadata FROM files n WHERE s.id='$seed' AND n.id='$new';" >/dev/null 2>&1
    # move disk artifacts NEW(uploader dir) -> SEED(owner dir); keyed by file_id
    su="$FILES_DIR/thumbnails/$uploader/$new.jpg"; du="$FILES_DIR/thumbnails/$OWNER_ID/$seed.jpg"
    if [[ -f "$su" ]]; then mkdir -p "$FILES_DIR/thumbnails/$OWNER_ID"; mv -f "$su" "$du"; fi
    for kind in images text; do
      sd="$FILES_DIR/$kind/$uploader/$new"; dd="$FILES_DIR/$kind/$OWNER_ID/$seed"
      if [[ -d "$sd" ]]; then mkdir -p "$FILES_DIR/$kind/$OWNER_ID"; rm -rf "$dd"; mv -f "$sd" "$dd"; fi
    done
    # drop the temp upload (artifacts already moved; removes NEW row + its original)
    curl -s -X DELETE "$API_URL/api/files/$new" -H "Authorization: Bearer $token" >/dev/null 2>&1
    echo "    $name -> processed + grafted onto $seed"
  done
}
process_files
set -e

echo ""
echo "==> done. Conversation id: 11111111-1111-1111-1111-111111111111"
echo "    Open it in the chat UI (owned by user $OWNER_ID) to eyeball rendering."
