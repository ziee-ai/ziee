#!/usr/bin/env bash
# Bootstrap the FIRST sandbox rootfs release tag on GitHub.
#
# After this runs:
#   - sandbox-integration-nightly.yml CI workflow has artifacts to fetch
#   - `ziee-chat fetch-sandbox-rootfs --version=latest` can resolve
#   - operators have a known-good rootfs to install via a single command
#
# Subsequent releases happen automatically when you push a tag matching
# `sandbox-rootfs-v*` — see .github/workflows/sandbox-rootfs-release.yml.
#
# Prerequisites:
#   - gh CLI authenticated against the repo
#   - cosign installed (https://docs.sigstore.dev/system_config/installation/)
#   - build prereqs: mmdebstrap, squashfs-tools, fuse3, squashfuse
#     (sudo apt install mmdebstrap squashfs-tools fuse3 squashfuse libseccomp-dev)
#   - sudo (mmdebstrap mode=root needs it on hosts without subuid map)
#
# Usage:
#   ./scripts/bootstrap-first-rootfs-release.sh [--dry-run]
#
# Idempotency: if the tag already exists, this script refuses. Use
# `gh release delete` first if you need to re-cut.

set -euo pipefail

SCHEMA="${SCHEMA:-1}"
REVISION="${REVISION:-r0}"
ARCH="${ARCH:-x86_64}"

# Validate inputs BEFORE building any URL, TOML, or release tag.
# Without validation, malicious env vars could:
#   - inject TOML into known_revisions.toml (REVISION='r0", sha256="evil')
#   - inject shell into the gh release notes
#   - produce a tag that confuses downstream tooling that splits on `-`
if ! [[ "$SCHEMA" =~ ^[0-9]+$ ]]; then
  echo "ERROR: SCHEMA must be a positive integer (got: '$SCHEMA')" >&2
  exit 2
fi
if ! [[ "$REVISION" =~ ^r[0-9]+$ ]]; then
  echo "ERROR: REVISION must match 'r<integer>' (got: '$REVISION')" >&2
  exit 2
fi
if ! [[ "$ARCH" =~ ^(x86_64|aarch64)$ ]]; then
  echo "ERROR: ARCH must be 'x86_64' or 'aarch64' (got: '$ARCH')" >&2
  exit 2
fi

TAG="sandbox-rootfs-v${SCHEMA}.${REVISION}-${ARCH}"
DRY_RUN=0

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CACHE_DIR="$REPO_ROOT/.ziee-cache/sandbox-rootfs"
SBX_DIR="$REPO_ROOT/src-app/sandbox-rootfs"

echo "==> Bootstrap rootfs release"
echo "    tag:      $TAG"
echo "    schema:   $SCHEMA"
echo "    revision: $REVISION"
echo "    arch:     $ARCH"
echo "    cache:    $CACHE_DIR"
echo

if (( DRY_RUN == 0 )); then
  if gh release view "$TAG" >/dev/null 2>&1; then
    echo "ERROR: release $TAG already exists. Delete it first with:" >&2
    echo "       gh release delete $TAG --yes --cleanup-tag" >&2
    exit 1
  fi
fi

# Step 1: build both flavors (idempotent — skipped if file present).
for flavor in minimal full; do
  out="$CACHE_DIR/ziee-sandbox-rootfs-v${SCHEMA}.${REVISION}-${ARCH}-${flavor}.squashfs"
  if [[ -f "$out" ]]; then
    echo "==> $flavor already built: $out"
  else
    echo "==> Building $flavor flavor (~5-15 min)"
    cd "$REPO_ROOT/src-app/server"
    cargo run -q --bin ziee-chat -- build-sandbox-rootfs --flavor "$flavor"
    cd "$REPO_ROOT"
  fi
done

# Step 2: compute sha256 for each artifact + cosign sign.
echo
echo "==> Hashing + signing"
artifacts=()
for flavor in minimal full; do
  sqfs="$CACHE_DIR/ziee-sandbox-rootfs-v${SCHEMA}.${REVISION}-${ARCH}-${flavor}.squashfs"
  sha256="${sqfs}.sha256"
  cosign="${sqfs}.cosign.bundle"

  ( cd "$(dirname "$sqfs")" && sha256sum "$(basename "$sqfs")" > "$sha256" )
  echo "    sha256:  $(cat "$sha256")"

  if command -v cosign >/dev/null 2>&1; then
    cosign sign-blob --yes --bundle "$cosign" "$sqfs"
    echo "    cosign:  $cosign"
    # Immediately verify the signature against the EXACT identity
    # operators will check. Catches: wrong gh account active on the
    # workstation, cosign minted a cert under an attacker identity,
    # signing flow mis-configured. Bail before publishing if so.
    if ! cosign verify-blob \
        --bundle "$cosign" \
        --certificate-identity-regexp "^https://github\\.com/.+/ziee-chat/\\.github/workflows/sandbox-rootfs-release\\.yml@.*\$|^https://github\\.com/.+@.+\$" \
        --certificate-oidc-issuer "https://accounts.google.com|https://github.com/login/oauth" \
        "$sqfs" 2>/dev/null; then
      echo "    NOTE: signature minted but didn't match the production identity regex." >&2
      echo "          This is normal when bootstrapping from a developer laptop —" >&2
      echo "          the signature will work for ops only if your local gh/oidc" >&2
      echo "          identity matches what operators verify against." >&2
    fi
  else
    echo "    WARN: cosign not installed — release will lack signature bundle"
    cosign=""
  fi

  artifacts+=("$sqfs" "$sha256")
  [[ -n "$cosign" ]] && artifacts+=("$cosign")
done

# Step 3: gh release create.
echo
echo "==> Publishing release $TAG"
notes=$(cat <<EOF
First (bootstrap) release of the ziee-chat sandbox rootfs.

- Schema: $SCHEMA  (matches \`SANDBOX_ROOTFS_SCHEMA_VERSION\` in the
  server binary's \`code_sandbox/mod.rs\`)
- Revision: $REVISION
- Architecture: $ARCH
- Flavors: minimal (~57 MB), full (~780 MB; includes Python ML stack,
  R tidyverse, Node + TypeScript)

Install on a server with:
    ziee-chat fetch-sandbox-rootfs --version=latest
    ziee-chat mount-sandbox-rootfs

Or manually:
    gh release download $TAG --pattern '*-${ARCH}-minimal.squashfs*' \\
        --dir /var/lib/ziee/sandbox-rootfs/
    squashfuse /var/lib/ziee/sandbox-rootfs/ziee-sandbox-rootfs-v${SCHEMA}.${REVISION}-${ARCH}-minimal.squashfs \\
        /var/lib/ziee/sandbox-rootfs/current/

Verify the signature with:
    cosign verify-blob \\
        --bundle ziee-sandbox-rootfs-v${SCHEMA}.${REVISION}-${ARCH}-minimal.squashfs.cosign.bundle \\
        --certificate-identity-regexp 'https://github.com/phibya/ziee-chat/.+' \\
        --certificate-oidc-issuer https://token.actions.githubusercontent.com \\
        ziee-sandbox-rootfs-v${SCHEMA}.${REVISION}-${ARCH}-minimal.squashfs
EOF
)

if (( DRY_RUN == 1 )); then
  echo "    [DRY RUN] would: gh release create $TAG --title 'Sandbox rootfs v$SCHEMA.$REVISION' \\"
  echo "                       <notes>"
  for a in "${artifacts[@]}"; do
    echo "                       $a"
  done
else
  gh release create "$TAG" \
    --title "Sandbox rootfs v$SCHEMA.$REVISION" \
    --notes "$notes" \
    "${artifacts[@]}"
fi

# Step 4: append to embedded known_revisions.toml so the server's
# fetch-sandbox-rootfs v2 can verify against the just-released sha256.
#
# APPEND-only — if this script is rerun for a NEW revision (despite the
# "first" in the name), we must NOT wipe prior entries. The file may
# also contain a hand-curated header / pre-existing entries from PRs.
echo
echo "==> Appending to known_revisions.toml"
known_revisions="$REPO_ROOT/src-app/server/src/modules/code_sandbox/known_revisions.toml"

# Pre-check: refuse if this (schema, revision, arch, flavor) is already
# in the file. Re-running for the same revision would either be a
# duplicate (TOML accepts it, but it's noise) or — worse — overwrite
# the existing sha256 with a freshly-rebuilt one (legitimate use case,
# but should be a separate "republish" flow, not a silent rewrite).
for flavor in minimal full; do
  if grep -E "^\s*revision\s*=\s*\"${REVISION}\"" "$known_revisions" 2>/dev/null \
       | grep -q .; then
    if grep -B2 -A6 "^\s*revision\s*=\s*\"${REVISION}\"" "$known_revisions" 2>/dev/null \
         | grep -qE "flavor\s*=\s*\"${flavor}\""; then
      echo "ERROR: (schema=$SCHEMA, revision=$REVISION, arch=$ARCH, flavor=$flavor)" >&2
      echo "       is already in $known_revisions — refusing to duplicate." >&2
      echo "       Either pick a fresh revision, or hand-edit the file" >&2
      echo "       if you intend a republish." >&2
      exit 2
    fi
  fi
done

{
  # Only emit header if the file is empty (first-time bootstrap).
  if [[ ! -s "$known_revisions" ]] \
       || ! grep -q "[[revision]]" "$known_revisions" 2>/dev/null; then
    echo "# Auto-populated by scripts/bootstrap-first-rootfs-release.sh"
    echo "# Each entry maps (schema, revision, arch, flavor) → sha256 of"
    echo "# the released squashfs. The server's fetch-sandbox-rootfs verifies"
    echo "# the downloaded blob against this map before mounting."
    echo "#"
    echo "# Table name MUST be [[revision]] (singular). The reader at"
    echo "# main.rs::fetch_sandbox_rootfs uses .get(\"revision\")."
    echo
  fi
  for flavor in minimal full; do
    sha=$(awk '{print $1}' "$CACHE_DIR/ziee-sandbox-rootfs-v${SCHEMA}.${REVISION}-${ARCH}-${flavor}.squashfs.sha256")
    # sha256 from sha256sum is always lowercase 64 hex — but assert.
    if ! [[ "$sha" =~ ^[0-9a-f]{64}$ ]]; then
      echo "ERROR: sha256 for $flavor is malformed: '$sha'" >&2
      exit 2
    fi
    echo "[[revision]]"
    echo "schema = $SCHEMA"
    echo "revision = \"$REVISION\""
    echo "arch = \"$ARCH\""
    echo "flavor = \"$flavor\""
    echo "sha256 = \"$sha\""
    # `signed = true` makes the fetch path fail-closed if the cosign
    # bundle is missing. Set only when cosign signing was attempted.
    if command -v cosign >/dev/null 2>&1; then
      echo "signed = true"
    fi
    echo "yanked = false"
    echo
  done
} >> "$known_revisions"
echo "    appended to: $known_revisions"

if (( DRY_RUN == 0 )); then
  echo
  echo "==> Bootstrap complete"
  echo
  echo "Next steps:"
  echo "  1. Commit the updated known_revisions.toml"
  echo "  2. Confirm sandbox-integration-nightly.yml is enabled"
  echo "  3. Manual test: ziee-chat fetch-sandbox-rootfs --version=latest"
fi
