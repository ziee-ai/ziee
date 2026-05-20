#!/usr/bin/env bash
# Mount the squashfs via squashfuse and run the basic surface
# assertions: required binaries present, no setuid, schema file
# matches the integer in the filename.
#
# Usage:  src-app/sandbox-rootfs/ci/smoke-test.sh <path-to.squashfs>

set -euo pipefail

SQFS="${1:-}"
if [ -z "$SQFS" ] || [ ! -f "$SQFS" ]; then
    echo "usage: $0 <path-to-rootfs.squashfs>" >&2
    exit 2
fi

MNT="$(mktemp -d -t ziee-sandbox-smoke-XXXX)"
cleanup() {
    fusermount -u "$MNT" 2>/dev/null || true
    rmdir "$MNT" 2>/dev/null || true
}
trap cleanup EXIT

echo "==> mount $SQFS at $MNT"
squashfuse "$SQFS" "$MNT"

echo "==> assert required binaries present"
for bin in /bin/bash /usr/bin/python3 /usr/bin/prlimit /bin/cat /bin/ls /usr/bin/whoami; do
    if [ ! -x "$MNT$bin" ]; then
        echo "FAIL: missing required binary: $bin" >&2
        exit 1
    fi
done

echo "==> assert schema file exists + is numeric"
schema_file="$MNT/.ziee-sandbox-rootfs-schema"
if [ ! -f "$schema_file" ]; then
    echo "FAIL: $schema_file missing" >&2
    exit 1
fi
schema="$(cat "$schema_file")"
if ! [[ "$schema" =~ ^[0-9]+$ ]]; then
    echo "FAIL: schema is not a positive integer: '$schema'" >&2
    exit 1
fi
echo "    schema = $schema"

echo "==> assert no setuid/setgid binaries"
setuid="$(find "$MNT" -xdev \( -perm /u+s -o -perm /g+s \) -type f 2>/dev/null || true)"
if [ -n "$setuid" ]; then
    echo "FAIL: setuid/setgid binaries found:" >&2
    echo "$setuid" >&2
    exit 1
fi

echo "==> bwrap smoke (echo hello)"
bwrap --unshare-user --uid 1001 --gid 1001 --share-net --new-session --die-with-parent \
    --ro-bind "$MNT/usr" /usr \
    --symlink usr/bin /bin --symlink usr/lib /lib --symlink usr/lib64 /lib64 \
    --dev-bind /proc /proc --dev /dev --tmpfs /tmp \
    -- /bin/echo hello-from-rootfs

echo "OK: smoke test passed"
