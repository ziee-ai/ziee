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
#
# If the squashfs exists but squashfuse fails, exit non-zero.
# Letting the server boot without the rootfs would silently disable
# the entire code_sandbox feature; k8s liveness probes would report
# green while operators wonder why no tool calls work. Better to
# fail loudly so the container restart loop surfaces the problem.
if [ -f "$ROOTFS_SQUASHFS" ] && ! mountpoint -q "$ROOTFS_MOUNT" 2>/dev/null; then
    mkdir -p "$ROOTFS_MOUNT"
    if ! squashfuse "$ROOTFS_SQUASHFS" "$ROOTFS_MOUNT"; then
        echo "FATAL: squashfuse failed to mount $ROOTFS_SQUASHFS" >&2
        echo "       The sandbox feature requires the rootfs to be mounted." >&2
        echo "       Common causes:" >&2
        echo "         - missing --device /dev/fuse" >&2
        echo "         - missing --cap-add SYS_ADMIN (or --privileged)" >&2
        echo "         - missing --security-opt seccomp=unconfined" >&2
        echo "         - missing --security-opt apparmor=unconfined" >&2
        echo "       See src-app/docker-compose.prod.yaml + DEPLOYMENT.md." >&2
        exit 1
    fi
    # Verify the schema sentinel is readable so the server's boot
    # probe doesn't silently disable sandbox a few seconds later.
    if [ ! -r "$ROOTFS_MOUNT/.ziee-sandbox-rootfs-schema" ]; then
        echo "FATAL: $ROOTFS_MOUNT/.ziee-sandbox-rootfs-schema not readable post-mount." >&2
        echo "       Either the squashfs is corrupt or the image was built without" >&2
        echo "       the schema sentinel. Re-build the rootfs from a clean source." >&2
        fusermount -u "$ROOTFS_MOUNT" 2>/dev/null || true
        exit 1
    fi
fi

# Cgroup v2 delegation setup. If a delegated parent slice is mounted
# at the documented path, ensure the controllers we need are enabled.
# Best-effort — failures here mean the server falls back to
# rlimits-only mode (still safe, just less defense in depth) — but
# log loudly so ops can see the degradation.
CGROUP_PARENT="${CODE_SANDBOX_CGROUP_PARENT:-/sys/fs/cgroup/ziee-sandbox.slice}"
cgroup_state="off"
if [ -d "$CGROUP_PARENT" ] && [ -w "$CGROUP_PARENT/cgroup.subtree_control" ]; then
    if echo "+memory +pids +cpu" > "$CGROUP_PARENT/cgroup.subtree_control" 2>/dev/null; then
        cgroup_state="on (delegated)"
    else
        cgroup_state="off (subtree_control write failed)"
    fi
elif [ -d "$CGROUP_PARENT" ]; then
    cgroup_state="off (subtree_control not writable)"
fi
echo "ziee-entrypoint: cgroup_v2 = $cgroup_state" >&2

# Sweep stale cgroup scopes from prior crashed runs (tini reaps
# processes; it does NOT clean up the per-call cgroup mkdirs we
# leave behind under $CGROUP_PARENT/sandbox-*).
if [ -d "$CGROUP_PARENT" ]; then
    for d in "$CGROUP_PARENT"/sandbox-*; do
        [ -d "$d" ] && rmdir "$d" 2>/dev/null || true
    done
fi

exec "$@"
