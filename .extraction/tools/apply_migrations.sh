#!/usr/bin/env bash
# Apply a migration set to a FRESH scratch DB, exactly the way build.rs composes
# it: drop+recreate the DB, then apply every *.sql from the given source dirs in
# a single version-sorted stream (basename sort — valid for both the numeric
# 00000000000NNN_ prefixes and the squashed YYYYMMDDNNNN_ prefixes).
#
# Usage: apply_migrations.sh <db_name> <dir1> [dir2 ...]
# Env:   ADMIN_URL (default postgresql://postgres:password@127.0.0.1:54321/postgres)
set -euo pipefail

DB="$1"; shift
ADMIN_URL="${ADMIN_URL:-postgresql://postgres:password@127.0.0.1:54321/postgres}"
# Derive the target URL by swapping the trailing db name of ADMIN_URL.
TARGET_URL="$(python3 - "$ADMIN_URL" "$DB" <<'PY'
import sys
from urllib.parse import urlparse, urlunparse
u = urlparse(sys.argv[1]); db = sys.argv[2]
print(urlunparse(u._replace(path='/'+db)))
PY
)"

psql "$ADMIN_URL" -X -q -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS $DB WITH (FORCE);" >/dev/null
psql "$ADMIN_URL" -X -q -v ON_ERROR_STOP=1 -c "CREATE DATABASE $DB;" >/dev/null

# Collect + version-sort all .sql across the source dirs by BASENAME.
mapfile -t FILES < <(
  for d in "$@"; do
    find "$d" -maxdepth 1 -name '*.sql' -print
  done | awk -F/ '{print $NF"\t"$0}' | sort -k1,1 | cut -f2-
)

for f in "${FILES[@]}"; do
  psql "$TARGET_URL" -X -q -v ON_ERROR_STOP=1 -f "$f" >/dev/null
done

echo "$TARGET_URL"
