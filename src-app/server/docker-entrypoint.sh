#!/usr/bin/env bash
# Entrypoint for ziee/server-with-sandbox container.
#
# Responsibilities:
#   1. Mount the baked-in squashfs at the path the server expects.
#   2. Set up the per-call cgroup parent if cgroup delegation is
#      available (otherwise fall back to rlimits-only mode silently).
#   3. exec the server (replaces $0 so PID 1 stays tini).
set -euo pipefail

ROOTFS_SQUASHFS="${ROOTFS_SQUASHFS:-/var/lib/ziee/sandbox-rootfs/current.squashfs}"
ROOTFS_MOUNT="${CODE_SANDBOX_ROOTFS_PATH:-/var/lib/ziee/sandbox-rootfs/current}"

# Mount the rootfs (idempotent — skip if already mounted, e.g. on
# container restart with a persistent volume).
if [ -f "$ROOTFS_SQUASHFS" ] && ! mountpoint -q "$ROOTFS_MOUNT" 2>/dev/null; then
    mkdir -p "$ROOTFS_MOUNT"
    if ! squashfuse "$ROOTFS_SQUASHFS" "$ROOTFS_MOUNT"; then
        echo "ERROR: squashfuse failed to mount $ROOTFS_SQUASHFS — code_sandbox will refuse to register." >&2
        echo "       Common cause: container is missing /dev/fuse or SYS_ADMIN cap." >&2
        echo "       See src-app/docker-compose.prod.yaml for required runtime options." >&2
    fi
fi

# Cgroup v2 delegation setup. If a delegated parent slice is mounted
# at the documented path, ensure the controllers we need are enabled.
# Best-effort — failures here mean the server falls back to
# rlimits-only mode (still safe, just less defense in depth).
CGROUP_PARENT="${CODE_SANDBOX_CGROUP_PARENT:-/sys/fs/cgroup/ziee-sandbox.slice}"
if [ -d "$CGROUP_PARENT" ] && [ -w "$CGROUP_PARENT/cgroup.subtree_control" ]; then
    # Enable the controllers we use per-call. Idempotent.
    echo "+memory +pids +cpu" > "$CGROUP_PARENT/cgroup.subtree_control" 2>/dev/null || true
fi

exec "$@"
