#!/usr/bin/env bash
# Build a ziee sandbox rootfs squashfs.
#
# Defaults:
#   --flavor full
#   --schema $(cat src-app/sandbox-rootfs/compat.toml | <current_schema>)
#   --revision r0
#   --arch    x86_64  (from `uname -m` — only override for cross-build)
#   --output  .ziee-cache/sandbox-rootfs/ziee-sandbox-rootfs-v{schema}.r{rev}-{arch}-{flavor}.squashfs
#
# Two backends, auto-detected:
#   1. docker (Dockerfile-based; portable)
#   2. mmdebstrap (planned; faster + more reproducible; not yet wired)
#
# Reproducibility:
#   SOURCE_DATE_EPOCH is exported (default: today's commit timestamp)
#   so the squashfs hash is stable across CI runs.

set -euo pipefail

# --------------------------------------------------------------------
# Argument parsing
# --------------------------------------------------------------------

FLAVOR="full"
SCHEMA=""
REVISION="r0"
ARCH="$(uname -m)"
OUTPUT=""
APT_CACHE=""
USE_DOCKER=1   # set to 0 to force mmdebstrap (not yet wired)

while [[ $# -gt 0 ]]; do
  case "$1" in
    --flavor)    FLAVOR="$2";    shift 2 ;;
    --schema)    SCHEMA="$2";    shift 2 ;;
    --revision)  REVISION="$2";  shift 2 ;;
    --arch)      ARCH="$2";      shift 2 ;;
    --output)    OUTPUT="$2";    shift 2 ;;
    --apt-cache) APT_CACHE="$2"; shift 2 ;;
    --no-docker) USE_DOCKER=0;   shift ;;
    -h|--help)
      grep '^#' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# --------------------------------------------------------------------
# Read schema version from compat.toml if not given.
# --------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [[ -z "$SCHEMA" ]]; then
  if [[ -f "$SCRIPT_DIR/compat.toml" ]]; then
    SCHEMA="$(awk -F'=' '/^current_schema/ {gsub(/[ "\047]/, "", $2); print $2; exit}' "$SCRIPT_DIR/compat.toml")"
  fi
  : "${SCHEMA:=1}"
fi

if [[ -z "$OUTPUT" ]]; then
  OUTPUT="$REPO_ROOT/.ziee-cache/sandbox-rootfs/ziee-sandbox-rootfs-v${SCHEMA}.${REVISION}-${ARCH}-${FLAVOR}.squashfs"
fi

mkdir -p "$(dirname "$OUTPUT")"

# --------------------------------------------------------------------
# Reproducibility env
# --------------------------------------------------------------------

if [[ -z "${SOURCE_DATE_EPOCH:-}" ]]; then
  # Use the last commit timestamp of the rootfs build tree as a
  # reasonable, stable default.
  SOURCE_DATE_EPOCH="$(git -C "$REPO_ROOT" log -1 --format=%ct -- "$SCRIPT_DIR" 2>/dev/null || date -u +%s)"
fi
export SOURCE_DATE_EPOCH

# --------------------------------------------------------------------
# Build via docker (default)
# --------------------------------------------------------------------

if [[ "$USE_DOCKER" == "1" ]]; then
  if ! command -v docker >/dev/null; then
    echo "build.sh: docker not found in PATH; install docker or pass --no-docker (mmdebstrap not yet wired)" >&2
    exit 1
  fi
  if ! command -v mksquashfs >/dev/null; then
    echo "build.sh: mksquashfs not found in PATH; apt install squashfs-tools" >&2
    exit 1
  fi

  IMAGE_TAG="ziee-sandbox-rootfs-v${SCHEMA}.${REVISION}-${FLAVOR}"
  STAGE_TAR="$(dirname "$OUTPUT")/.stage-${IMAGE_TAG}.tar"
  STAGE_DIR="$(dirname "$OUTPUT")/.stage-${IMAGE_TAG}"

  echo "==> docker build (flavor=$FLAVOR schema=$SCHEMA rev=$REVISION arch=$ARCH)"
  build_args=(
    --build-arg "ZIEE_SANDBOX_FLAVOR=$FLAVOR"
    --build-arg "ZIEE_SANDBOX_SCHEMA=$SCHEMA"
  )
  if [[ -n "$APT_CACHE" ]]; then
    build_args+=(--build-arg "http_proxy=$APT_CACHE" --build-arg "HTTP_PROXY=$APT_CACHE")
  fi
  docker build -f "$SCRIPT_DIR/Dockerfile" "${build_args[@]}" -t "$IMAGE_TAG" "$SCRIPT_DIR"

  echo "==> docker export → tar"
  CONTAINER_ID="$(docker create "$IMAGE_TAG")"
  trap 'docker rm -f "$CONTAINER_ID" >/dev/null 2>&1 || true' EXIT
  docker export "$CONTAINER_ID" -o "$STAGE_TAR"

  echo "==> extract tar → staging dir"
  rm -rf "$STAGE_DIR"
  mkdir -p "$STAGE_DIR"
  tar -xf "$STAGE_TAR" -C "$STAGE_DIR" --xattrs --acls --numeric-owner
  rm -f "$STAGE_TAR"

  echo "==> mksquashfs ($OUTPUT)"
  rm -f "$OUTPUT"
  mksquashfs "$STAGE_DIR" "$OUTPUT" \
    -comp zstd -Xcompression-level 19 \
    -no-xattrs \
    -all-time "$SOURCE_DATE_EPOCH" \
    -mkfs-time "$SOURCE_DATE_EPOCH" \
    -noappend -no-progress \
    -quiet

  rm -rf "$STAGE_DIR"

  size_h="$(du -h "$OUTPUT" | cut -f1)"
  sha="$(sha256sum "$OUTPUT" | cut -d' ' -f1)"
  echo "==> done: $OUTPUT ($size_h, sha256=$sha)"
else
  echo "build.sh: --no-docker (mmdebstrap backend) not yet implemented" >&2
  exit 1
fi
