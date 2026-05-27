#!/usr/bin/env bash
# Build a minimal test rootfs squashfs for tier 4/6 sandbox tests.
#
# Output: .ziee-cache/sandbox-rootfs/test-minimal.squashfs
#
# This is NOT the production rootfs (which is built by
# `src-app/sandbox-rootfs/build.sh` via mmdebstrap + systemd-nspawn,
# requires CAP_SYS_ADMIN, ~15 min). Instead it's a stripped-down
# Alpine-based rootfs that provides JUST enough userland for tier 4/6
# tests to exercise the bwrap hardening primitives:
#
#   - busybox + GNU coreutils  (echo, whoami, cat, id, mount, …)
#   - bash (some tests assume bash semantics — `bash -lc '...'`)
#   - basic /etc/passwd + /etc/group (sandbox masks these per-call)
#   - the .ziee-sandbox-rootfs-schema sentinel so the boot probe passes
#
# Built via Docker on Mac (no host requirements beyond Docker); built
# directly on Linux (no Docker needed).
#
# Idempotent: skip if cached. `--force` to rebuild.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CACHE_DIR="$REPO_ROOT/.ziee-cache/sandbox-rootfs"
OUTPUT="$CACHE_DIR/test-minimal.squashfs"
SCHEMA="$(grep -E '^current_schema' "$REPO_ROOT/src-app/sandbox-rootfs/compat.toml" | awk '{print $3}')"

FORCE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --force) FORCE=1; shift ;;
    -h|--help)
      cat <<EOF
build-test-rootfs.sh — Build the minimal test sandbox squashfs.

Usage:
  $(basename "$0") [--force]

Output:
  $OUTPUT
EOF
      exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

mkdir -p "$CACHE_DIR"

if [[ "$FORCE" -eq 0 && -f "$OUTPUT" ]]; then
  echo "✓ test rootfs already cached: $OUTPUT ($(du -h "$OUTPUT" | cut -f1))"
  echo "  use --force to rebuild"
  exit 0
fi

# The arch the Mac libkrun guest uses is aarch64; on Linux the host
# arch is whatever uname says. Build matches that so tier 4/6 can
# actually run the binaries in this rootfs.
case "$(uname -m)" in
  arm64|aarch64) ARCH=arm64 ;;
  x86_64|amd64) ARCH=amd64 ;;
  *) echo "unsupported arch: $(uname -m)" >&2; exit 1 ;;
esac

# Assemble Alpine rootfs in a Docker container; mksquashfs in the same
# container. Output volume-mounted back to the host. The container needs:
# alpine-keys (for apk trust), squashfs-tools (for mksquashfs).
docker run --rm \
  --platform "linux/$ARCH" \
  -v "$CACHE_DIR:/out" \
  alpine:3.20 \
  sh -c "
    set -euo pipefail
    # squashfs-tools for mksquashfs; python3 for the symlink-rewrite pass below.
    apk add --quiet --no-cache squashfs-tools python3 >/dev/null
    STAGE=/stage
    mkdir -p \$STAGE/etc/apk
    cp /etc/apk/repositories \$STAGE/etc/apk/
    # Two-step install: keys first (so signature trust works), then payload.
    apk add --quiet --no-cache --initdb --root \$STAGE --allow-untrusted alpine-keys >/dev/null
    # util-linux-misc is required for /usr/bin/prlimit which the
    # production sandbox argv wraps user commands in (rlimits
    # defense-in-depth). libsmartcols is its runtime dep that apk
    # doesn't auto-pull because util-linux-misc lists it as optional;
    # without it prlimit fails with a misleading 'execvp: No such file'
    # (the dynamic linker can't find libsmartcols.so.1).
    # python3 is needed by several tier-6 hardening tests that exercise
    # memory caps via Python's bytearray alloc.
    apk add --quiet --no-cache --root \$STAGE \
      alpine-baselayout busybox musl bash coreutils \
      util-linux util-linux-misc libsmartcols procps python3 >/dev/null
    # usr-merge: production sandbox argv does '--symlink usr/lib /lib'
    # (assumes Debian usrmerged layout). Alpine keeps /lib + /usr/lib
    # separate, so libsmartcols.so.1 (needed by /usr/bin/prlimit) ends
    # up in /lib and is invisible after the symlink shim. Move all /lib,
    # /lib64, /bin, /sbin files into /usr/* and replace the originals
    # with symlinks — matches what bwrap's argv expects.
    #
    # Important: Alpine ships relative symlinks like /bin/sleep ->
    # ../usr/bin/coreutils. Naive 'cp -a' to /usr/bin makes the target
    # resolve to /usr/usr/bin/coreutils (broken). Rewrite relative
    # symlinks to absolute (rootfs-anchored) paths first, then move.
    python3 - <<PYEOF
import os, posixpath
stage = '\$STAGE'
for sub in ('lib', 'lib64', 'bin', 'sbin'):
    d = os.path.join(stage, sub)
    if not os.path.isdir(d) or os.path.islink(d):
        continue
    for root, dirs, files in os.walk(d):
        for name in files + dirs:
            p = os.path.join(root, name)
            if not os.path.islink(p):
                continue
            tgt = os.readlink(p)
            if tgt.startswith('/'):
                continue
            # Resolve relative target to a rootfs-anchored absolute path
            link_dir_in_rootfs = '/' + os.path.relpath(os.path.dirname(p), stage)
            abs_in_rootfs = posixpath.normpath(posixpath.join(link_dir_in_rootfs, tgt))
            os.remove(p)
            os.symlink(abs_in_rootfs, p)
PYEOF
    for d in lib lib64 bin sbin; do
      if [ -d \$STAGE/\$d ] && [ ! -L \$STAGE/\$d ]; then
        mkdir -p \$STAGE/usr/\$d
        cp -a \$STAGE/\$d/. \$STAGE/usr/\$d/ 2>/dev/null || true
        rm -rf \$STAGE/\$d
        ln -s usr/\$d \$STAGE/\$d
      fi
    done
    # The schema sentinel the boot probe reads.
    echo '$SCHEMA' > \$STAGE/.ziee-sandbox-rootfs-schema
    # Pre-create the mount points sandbox bind-binds into (best-effort, agent
    # also tries to create them).
    mkdir -p \$STAGE/proc \$STAGE/sys \$STAGE/dev \$STAGE/tmp \$STAGE/workspace
    # Skip device nodes that mksquashfs would warn on for un-rooted Docker.
    rm -f \$STAGE/dev/* 2>/dev/null || true
    # Strip setuid (defense-in-depth; same as the production build.sh).
    find \$STAGE -xdev \\( -perm /u+s -o -perm /g+s \\) -type f \
      -exec chmod u-s,g-s {} \\; 2>/dev/null || true
    echo '==> mksquashfs (gzip; xz fails to mount with libkrun bundled kernel)'
    mksquashfs \$STAGE /out/test-minimal.squashfs.tmp \
      -quiet -no-progress -comp gzip -all-root
    mv /out/test-minimal.squashfs.tmp /out/test-minimal.squashfs
    echo '==> wrote /out/test-minimal.squashfs (' \$(du -h /out/test-minimal.squashfs | cut -f1) ')'
  "

if [[ ! -f "$OUTPUT" ]]; then
  echo "ERROR: build completed but $OUTPUT is missing" >&2
  exit 1
fi
echo "✓ wrote $OUTPUT ($(du -h "$OUTPUT" | cut -f1))"
