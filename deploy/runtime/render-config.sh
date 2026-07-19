#!/usr/bin/env bash
# Render config.template.yaml -> config.yaml by substituting the 5 required
# ZIEE_* vars. Pure bash: NO envsubst (the build agent has no gettext) and NO
# sed (values may contain sed-special chars like / & \ $). Run before
# `docker compose up`; the compose bind-mounts the rendered config.yaml into
# ziee-web at /etc/ziee/config.yaml.
#
# FAILS LOUDLY (exit 1) if any required var is missing/empty or if any
# placeholder is left unsubstituted — the old envsubst version silently exited 0
# with an unrendered config when envsubst was absent, which broke a deploy.
#
#   ZIEE_DB_PASSWORD=... ZIEE_JWT_SECRET=... ZIEE_STORAGE_KEY=... \
#   ZIEE_PUBLIC_BASE_URL=https://chat.example.edu \
#   ZIEE_CORS_ALLOW_ORIGIN=https://chat.example.edu \
#     bash deploy/runtime/render-config.sh [output-path]
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
tmpl="$here/config.template.yaml"
out="${1:-$here/config.yaml}"

[[ -f "$tmpl" ]] || {
  echo "render-config.sh: ERROR — template not found: $tmpl" >&2
  exit 1
}

vars=(ZIEE_DB_PASSWORD ZIEE_JWT_SECRET ZIEE_STORAGE_KEY ZIEE_PUBLIC_BASE_URL ZIEE_CORS_ALLOW_ORIGIN)

# 1) Validate every required var is set + non-empty. Collect all, fail once, loud.
missing=()
for v in "${vars[@]}"; do
  [[ -n "${!v:-}" ]] || missing+=("$v")
done
if ((${#missing[@]})); then
  echo "render-config.sh: ERROR — missing/empty required env var(s): ${missing[*]}" >&2
  echo "render-config.sh: all of these must be set: ${vars[*]}" >&2
  exit 1
fi

# 2) Slurp the template verbatim (preserves newlines incl. the trailing one).
#    `read -d ''` returns non-zero at EOF but leaves the full file in $content.
content=""
IFS= read -r -d '' content < "$tmpl" || true

# 3) Substitute each literal ${VAR} via bash parameter expansion. The pattern
#    "\${$v}" is the literal string ${VAR}; the replacement "${!v}" is the var's
#    value, inserted literally (no sed regex/backreference pitfalls).
for v in "${vars[@]}"; do
  content=${content//"\${$v}"/"${!v}"}
done

# 4) Fail loudly if any ${ZIEE_*} placeholder survived (e.g. a template typo) —
#    never ship an unrendered config.
if [[ "$content" == *'${ZIEE_'* ]]; then
  echo "render-config.sh: ERROR — unsubstituted \${ZIEE_*} placeholder(s) remain after render" >&2
  exit 1
fi

# 5) Write it out.
printf '%s' "$content" > "$out"
echo "render-config.sh: wrote $out"
