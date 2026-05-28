#!/usr/bin/env bash
# Cross-compile sandbox-guest-agent to x86_64-unknown-linux-musl via Docker.
#
# Output: src-app/target/debug/deps/ziee-sandbox-agent (host path)
#
# Why: the agent runs INSIDE the WSL2 distro (Linux). Building from
# Windows requires cross-compilation; the cleanest path is Docker with
# the same rust:1.90-alpine3.20 image the macOS sandbox-runtime build
# uses (see src-app/server/build_helper/sandbox_runtime.rs:41).
#
# The agent links libseccomp statically. The musl crate in Alpine has
# libseccomp-dev + libseccomp-static, which is exactly what we need.
#
# Idempotent: skip if cached. `--force` to rebuild.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Destination: `wsl2.rs::agent_host_path()` looks for the agent as a
# sibling of the running exe. For the production server that's
# `target/debug/ziee.exe` → sibling `target/debug/ziee-sandbox-agent`.
# The Tier 4 test path (`exec_raw_argv`) reads via `current_exe()` of
# `target/debug/deps/integration_tests-*.exe` → sibling
# `target/debug/deps/ziee-sandbox-agent`. We write to BOTH so either
# path resolves without env overrides.
DEST="$REPO_ROOT/src-app/target/debug/ziee-sandbox-agent"
DEST_DEPS="$REPO_ROOT/src-app/target/debug/deps/ziee-sandbox-agent"

FORCE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --force) FORCE=1; shift ;;
    -h|--help)
      cat <<EOF
build-sandbox-agent-linux.sh — Cross-compile the Linux sandbox-guest-agent.

Usage:
  $(basename "$0") [--force]

Output:
  $DEST   (placed where wsl2.rs::agent_host_path() looks first)
EOF
      exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

mkdir -p "$(dirname "$DEST")" "$(dirname "$DEST_DEPS")"

if [[ "$FORCE" -eq 0 && -f "$DEST" && -f "$DEST_DEPS" ]]; then
  echo "✓ sandbox-guest-agent already built:"
  echo "    $DEST ($(du -h "$DEST" | cut -f1))"
  echo "    $DEST_DEPS ($(du -h "$DEST_DEPS" | cut -f1))"
  echo "  use --force to rebuild"
  exit 0
fi

# Build in a workspace-aware Alpine container. Bind-mount the entire
# src-app/ workspace + a separate target dir so cargo can resolve all
# path = ".." crates AND the Linux artifacts don't collide with the
# host's Windows artifacts.
DOCKER_TARGET="$REPO_ROOT/src-app/target/linux-musl"
mkdir -p "$DOCKER_TARGET"

echo "==> cross-compile sandbox-guest-agent (rust:1.90-alpine3.20 / x86_64-unknown-linux-musl)"
docker run --rm \
  --platform linux/amd64 \
  -v "$REPO_ROOT/src-app:/work" \
  -v "$DOCKER_TARGET:/cargo-target" \
  -w /work/sandbox-guest-agent \
  -e CARGO_TARGET_DIR=/cargo-target \
  -e CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS='-C target-feature=+crt-static' \
  -e LIBSECCOMP_LINK_TYPE=static \
  -e LIBSECCOMP_LIB_PATH=/usr/lib \
  rust:1.90-alpine3.20 \
  sh -c '
    set -eu
    apk add --quiet --no-cache musl-dev libseccomp-dev libseccomp-static gcc make pkgconf >/dev/null
    cargo build --release --target x86_64-unknown-linux-musl
  '

BUILT="$DOCKER_TARGET/x86_64-unknown-linux-musl/release/ziee-sandbox-agent"
if [[ ! -f "$BUILT" ]]; then
  echo "ERROR: agent build completed but $BUILT is missing" >&2
  exit 1
fi
cp "$BUILT" "$DEST"
chmod +x "$DEST"
cp "$BUILT" "$DEST_DEPS"
chmod +x "$DEST_DEPS"
echo "✓ wrote $DEST ($(du -h "$DEST" | cut -f1))"
echo "✓ wrote $DEST_DEPS ($(du -h "$DEST_DEPS" | cut -f1))"
