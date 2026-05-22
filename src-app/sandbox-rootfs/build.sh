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
# Backend: mmdebstrap (reproducible-by-design, no daemon). Install it with
#   apt install mmdebstrap squashfs-tools
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

while [[ $# -gt 0 ]]; do
  case "$1" in
    --flavor)    FLAVOR="$2";    shift 2 ;;
    --schema)    SCHEMA="$2";    shift 2 ;;
    --revision)  REVISION="$2";  shift 2 ;;
    --arch)      ARCH="$2";      shift 2 ;;
    --output)    OUTPUT="$2";    shift 2 ;;
    -h|--help)
      grep '^#' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

if ! command -v mmdebstrap >/dev/null; then
  echo "build.sh: mmdebstrap not found in PATH" >&2
  echo "  apt install mmdebstrap squashfs-tools" >&2
  exit 1
fi

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

# --------------------------------------------------------------------
# Resolve + source the flavor recipe: flavors/<flavor>/v<schema>/flavor.sh
# Each recipe is self-contained: APT_SNAPSHOT, APT_PACKAGES, and an
# optional provision() function. Adding a flavor = drop in a new dir.
# --------------------------------------------------------------------

RECIPE="$SCRIPT_DIR/flavors/$FLAVOR/v$SCHEMA/flavor.sh"
if [[ ! -f "$RECIPE" ]]; then
  echo "build.sh: no recipe at $RECIPE" >&2
  echo "  available flavors for schema v$SCHEMA:" >&2
  for f in "$SCRIPT_DIR"/flavors/*/v"$SCHEMA"/flavor.sh; do
    [[ -f "$f" ]] && echo "    - $(basename "$(dirname "$(dirname "$f")")")" >&2
  done
  exit 1
fi
# shellcheck source=/dev/null
source "$RECIPE"
: "${APT_SNAPSHOT:?recipe $RECIPE must set APT_SNAPSHOT}"
: "${APT_PACKAGES:?recipe $RECIPE must set APT_PACKAGES}"

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
# Common: mksquashfs check
# --------------------------------------------------------------------

if ! command -v mksquashfs >/dev/null; then
  echo "build.sh: mksquashfs not found in PATH; apt install squashfs-tools" >&2
  exit 1
fi

STAGE_DIR="$(dirname "$OUTPUT")/.stage-v${SCHEMA}.${REVISION}-${FLAVOR}"
# Cleanup needs sudo on platforms where mmdebstrap ran in root mode
# (the stage dir then contains root-owned files like /var/log/wtmp,
# /boot, /var/cache/ldconfig that a plain `rm -rf` can't remove).
cleanup_stage() {
  if [[ -d "$STAGE_DIR" ]]; then
    if command -v sudo >/dev/null && sudo -n true 2>/dev/null; then
      sudo rm -rf "$STAGE_DIR" 2>/dev/null || rm -rf "$STAGE_DIR" 2>/dev/null
    else
      rm -rf "$STAGE_DIR" 2>/dev/null
    fi
  fi
}
cleanup_stage
mkdir -p "$STAGE_DIR"

trap cleanup_stage EXIT

# --------------------------------------------------------------------
# Build: mmdebstrap bootstrap → chroot installs
# --------------------------------------------------------------------

build_mmdebstrap() {
  echo "==> mmdebstrap (flavor=$FLAVOR schema=$SCHEMA rev=$REVISION arch=$ARCH)"
  local mirror="http://snapshot.ubuntu.com/ubuntu/${APT_SNAPSHOT}"
  # Collapse the recipe's whitespace/newline package list to a comma list.
  local pkgs
  pkgs="$(echo "$APT_PACKAGES" | tr -s '[:space:]' ',' | sed 's/^,//; s/,$//')"

  # mmdebstrap does the bootstrap directly into the staging dir.
  # Mode selection:
  #   - root (preferred): we have sudo and a proper subuid map, OR we're
  #     running as root. Cleanest and most accurate file ownership.
  #   - fakechroot: unprivileged fallback when subuid is empty (common on
  #     personal workstations). Requires `fakechroot fakeroot` installed.
  local mode="fakechroot"
  if [[ "$EUID" -eq 0 ]] || (command -v sudo >/dev/null && sudo -n true 2>/dev/null); then
    mode="root"
  fi
  echo "    (mmdebstrap mode=$mode)"
  local mmd=(mmdebstrap
    --variant=minbase
    --mode="$mode"
    --components=main,universe
    --include="$pkgs"
    noble
    "$STAGE_DIR"
    "$mirror")
  if [[ "$mode" == "root" && "$EUID" -ne 0 ]]; then
    sudo -E "${mmd[@]}" 2>&1 | grep -vE "^I:" || true
  else
    "${mmd[@]}" 2>&1 | grep -vE "^I:" || true
  fi

  # Post-bootstrap provisioning, if the recipe defines it (pip/R/Node etc.
  # — mmdebstrap can't reach PyPI/CRAN/npm directly). Runs inside the chroot
  # via systemd-nspawn with /etc/resolv.conf bound. The recipe's `provision`
  # function is shipped in verbatim via `declare -f` (no quoting-hell).
  if declare -f provision >/dev/null; then
    echo "==> chroot provision (recipe provision function)"
    local prov="$STAGE_DIR/tmp/ziee-provision.sh"
    { echo "set -euo pipefail"; declare -f provision; echo "provision"; } \
      | sudo tee "$prov" >/dev/null
    sudo systemd-nspawn --quiet -D "$STAGE_DIR" \
      --bind-ro=/etc/resolv.conf \
      /bin/bash /tmp/ziee-provision.sh 2>&1 | tail -30
    sudo rm -f "$prov"
  fi

  # Write the schema sentinel.
  echo "$SCHEMA" | sudo tee "$STAGE_DIR/.ziee-sandbox-rootfs-schema" >/dev/null

  # Strip setuid bits (defense in depth).
  sudo find "$STAGE_DIR" -xdev \( -perm /u+s -o -perm /g+s \) -type f \
    -exec chmod u-s,g-s {} \; 2>/dev/null || true
}

# --------------------------------------------------------------------
# Build + finalize
# --------------------------------------------------------------------

build_mmdebstrap

echo "==> mksquashfs ($OUTPUT)"
rm -f "$OUTPUT"
# squashfs-tools >=4.6 errors if BOTH the SOURCE_DATE_EPOCH env var AND
# the explicit -all-time/-mkfs-time flags are set. Unset the env var
# only for this invocation; we still pass the value via flags so the
# output is bit-reproducible.
sde="$SOURCE_DATE_EPOCH"
env -u SOURCE_DATE_EPOCH \
mksquashfs "$STAGE_DIR" "$OUTPUT" \
  -comp zstd -Xcompression-level 19 \
  -no-xattrs \
  -all-time "$sde" \
  -mkfs-time "$sde" \
  -noappend -no-progress \
  -quiet

size_h="$(du -h "$OUTPUT" | cut -f1)"
sha="$(sha256sum "$OUTPUT" | cut -d' ' -f1)"
echo "==> done: $OUTPUT ($size_h, sha256=$sha)"
