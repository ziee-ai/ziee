#!/usr/bin/env bash
# Render config.template.yaml -> config.yaml by substituting ONLY the known ZIEE_*
# env vars (so incidental '$' in a secret value survives). Run this before
# `docker compose up`; the compose bind-mounts the rendered config.yaml into
# ziee-web at /etc/ziee/config.yaml.
#
#   ZIEE_DB_PASSWORD=... ZIEE_JWT_SECRET=... ZIEE_STORAGE_KEY=... \
#   ZIEE_PUBLIC_BASE_URL=https://chat.example.edu \
#   ZIEE_CORS_ALLOW_ORIGIN=https://chat.example.edu \
#     ./deploy/runtime/render-config.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
tmpl="$here/config.template.yaml"
out="${1:-$here/config.yaml}"

: "${ZIEE_DB_PASSWORD:?set ZIEE_DB_PASSWORD}"
: "${ZIEE_JWT_SECRET:?set ZIEE_JWT_SECRET (>=32 chars)}"
: "${ZIEE_STORAGE_KEY:?set ZIEE_STORAGE_KEY (>=32 chars, STABLE forever, equal to source DB storage_key)}"
: "${ZIEE_PUBLIC_BASE_URL:?set ZIEE_PUBLIC_BASE_URL (public web origin, e.g. https://chat.example.edu)}"
: "${ZIEE_CORS_ALLOW_ORIGIN:=$ZIEE_PUBLIC_BASE_URL}"
export ZIEE_CORS_ALLOW_ORIGIN

envsubst '${ZIEE_DB_PASSWORD} ${ZIEE_JWT_SECRET} ${ZIEE_STORAGE_KEY} ${ZIEE_PUBLIC_BASE_URL} ${ZIEE_CORS_ALLOW_ORIGIN}' \
    < "$tmpl" > "$out"
echo "render-config.sh: wrote $out"
