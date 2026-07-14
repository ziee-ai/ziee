#!/usr/bin/env bash
# Standalone-migration-apply gate (chunk sdk-standalone-fixes / gap N-1).
#
# The hole that let N-1 through: the equivalence gates only ever applied ziee's
# FULL merged migration set, where chat/scheduler/workflow/auth tables are all
# present — so a domain FK smuggled into an SDK *infra* crate's migration never
# failed. A SECOND app that links only that infra crate (no domain schema) hits
# `relation "public.<domain>" does not exist` on first boot.
#
# This gate closes it: for EACH schema-bound SDK crate, apply ITS `migrations/`
# ALONE to a fresh DB — plus ONLY its declared crate-deps' migrations — and
# require success with NO "relation does not exist". A crate that references a
# table it does not create (and does not inherit from a declared dep) fails here.
#
# Run from the worktree root:  bash .extraction/sdk-standalone-fixes/standalone_apply_gate.sh
set -uo pipefail

ADMIN_URL="${ADMIN_URL:-postgresql://postgres:password@127.0.0.1:54321/postgres}"
APPLY="$(dirname "$0")/../tools/apply_migrations.sh"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

# Per crate: "<crate-name> : <its migrations dir> [<declared-dep migrations dirs>...]"
# ziee-identity ships NO migrations, so notification (dep = ziee-identity) lists
# only its own dir. auth/file are self-contained. Extend this table when a new
# schema-bound SDK crate lands.
CRATES=(
  "ziee-auth|$ROOT/sdk/crates/ziee-auth/migrations"
  "ziee-file|$ROOT/sdk/crates/ziee-file/migrations"
  "ziee-notification|$ROOT/sdk/crates/ziee-notification/migrations"
  "ziee-onboarding|$ROOT/sdk/crates/ziee-onboarding/migrations"
)

fail=0
for row in "${CRATES[@]}"; do
  name="${row%%|*}"
  dirs="${row#*|}"
  IFS='|' read -r -a dir_arr <<< "$dirs"
  db="sdk_standalone_gate_$(echo "$name" | tr -c 'a-z0-9' '_')"
  err="$(mktemp)"
  if ADMIN_URL="$ADMIN_URL" bash "$APPLY" "$db" "${dir_arr[@]}" >/dev/null 2>"$err"; then
    # Ignore the benign `DROP DATABASE IF EXISTS` NOTICE ("database ... does not
    # exist, skipping"); a REAL missing-relation is what we gate on.
    if grep -i "does not exist" "$err" | grep -qvi "does not exist, skipping"; then
      echo "FAIL  $name — 'relation does not exist' during standalone apply"; fail=1
    else
      echo "PASS  $name — migrations/ apply standalone (deps: ${dir_arr[*]:1:9} <none if empty>)"
    fi
  else
    echo "FAIL  $name — apply errored:"; sed 's/^/      /' "$err" | grep -vi "does not exist, skipping" | head
    fail=1
  fi
  rm -f "$err"
done

exit $fail
