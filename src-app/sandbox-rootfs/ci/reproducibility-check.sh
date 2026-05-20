#!/usr/bin/env bash
# Build the rootfs twice with the same inputs and assert identical
# sha256. Called from .github/workflows/sandbox-rootfs-pr.yml +
# sandbox-rootfs-release.yml; can also be run locally.
#
# Usage:  src-app/sandbox-rootfs/ci/reproducibility-check.sh [flavor]
#         flavor defaults to "minimal".

set -euo pipefail

FLAVOR="${1:-minimal}"
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

cleanup() {
    rm -rf .ziee-cache/sandbox-rootfs/ziee-sandbox-rootfs-*-${FLAVOR}.squashfs 2>/dev/null || true
}
trap cleanup EXIT

echo "=== build #1 ==="
"$SCRIPT_DIR/build.sh" --flavor "$FLAVOR"
first=$(ls -t .ziee-cache/sandbox-rootfs/*-${FLAVOR}.squashfs | head -1)
sha1=$(sha256sum "$first" | cut -d' ' -f1)
size1=$(stat -c%s "$first")
mv "$first" "${first}.first"

echo "=== build #2 ==="
"$SCRIPT_DIR/build.sh" --flavor "$FLAVOR"
second=$(ls -t .ziee-cache/sandbox-rootfs/*-${FLAVOR}.squashfs | head -1)
sha2=$(sha256sum "$second" | cut -d' ' -f1)
size2=$(stat -c%s "$second")

echo
echo "build #1: $sha1 ($size1 bytes)"
echo "build #2: $sha2 ($size2 bytes)"

if [ "$sha1" = "$sha2" ]; then
    echo "OK: reproducible"
    exit 0
else
    echo "FAIL: two builds produced different bytes" >&2
    exit 1
fi
