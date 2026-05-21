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
#   1. mmdebstrap (primary; reproducible-by-design, no daemon)
#   2. docker  (fallback when mmdebstrap unavailable)
#   Override with --backend={mmdebstrap|docker}.
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
BACKEND="auto"   # auto | mmdebstrap | docker

while [[ $# -gt 0 ]]; do
  case "$1" in
    --flavor)    FLAVOR="$2";    shift 2 ;;
    --schema)    SCHEMA="$2";    shift 2 ;;
    --revision)  REVISION="$2";  shift 2 ;;
    --arch)      ARCH="$2";      shift 2 ;;
    --output)    OUTPUT="$2";    shift 2 ;;
    --apt-cache) APT_CACHE="$2"; shift 2 ;;
    --backend)   BACKEND="$2";   shift 2 ;;
    --no-docker) BACKEND="mmdebstrap"; shift ;;
    -h|--help)
      grep '^#' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# --backend=auto: prefer mmdebstrap if installed, else docker.
if [[ "$BACKEND" == "auto" ]]; then
  if command -v mmdebstrap >/dev/null; then
    BACKEND="mmdebstrap"
  elif command -v docker >/dev/null; then
    BACKEND="docker"
  else
    echo "build.sh: neither mmdebstrap nor docker found in PATH" >&2
    echo "  apt install mmdebstrap   # primary, faster, reproducible-by-design" >&2
    echo "  OR install docker         # fallback" >&2
    exit 1
  fi
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
# Backend: mmdebstrap (primary)
# --------------------------------------------------------------------

build_mmdebstrap() {
  echo "==> mmdebstrap (flavor=$FLAVOR schema=$SCHEMA rev=$REVISION arch=$ARCH)"
  local apt_snapshot
  apt_snapshot="$(cat "$SCRIPT_DIR/pins/apt-snapshot" 2>/dev/null | grep -v '^#' | head -1 | tr -d '[:space:]')"
  apt_snapshot="${apt_snapshot:-current}"
  local mirror="http://snapshot.ubuntu.com/ubuntu/${apt_snapshot}"

  local pkgs="bash,coreutils,util-linux,ca-certificates,curl,wget,bzip2,xz-utils,unzip,locales,tzdata,python3,python3-pip,python3-venv"
  if [[ "$FLAVOR" == "full" ]]; then
    pkgs+=",build-essential,gfortran,git,git-lfs,libffi-dev,libssl-dev,zlib1g-dev,vim,jq,ripgrep,fd-find,tree,net-tools,dnsutils,iputils-ping,gnupg,lsb-release,apt-transport-https,r-base,r-base-dev"
  fi

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

  # Layer 2: pip packages for full flavor (mmdebstrap can't reach
  # PyPI directly; use chroot pip after bootstrap).
  if [[ "$FLAVOR" == "full" ]]; then
    echo "==> chroot pip install (full flavor Python stack)"
    # Bind /etc/resolv.conf so pip can resolve pypi.
    sudo systemd-nspawn --quiet -D "$STAGE_DIR" \
      --bind-ro=/etc/resolv.conf \
      /bin/bash -c "
        pip3 install --no-cache-dir --break-system-packages \
          numpy pandas matplotlib scipy scikit-learn \
          seaborn plotly statsmodels sympy \
          requests httpx beautifulsoup4 \
          ipython jupyter pillow openpyxl xlrd pyarrow && \
        pip3 install --no-cache-dir --break-system-packages \
          torch torchvision --extra-index-url https://download.pytorch.org/whl/cpu
      " 2>&1 | tail -20

    echo "==> chroot R + Node install"
    sudo systemd-nspawn --quiet -D "$STAGE_DIR" \
      --bind-ro=/etc/resolv.conf \
      /bin/bash -c "
        Rscript -e \"install.packages(c('ggplot2','dplyr','tidyr','readr','stringr','lubridate','purrr','tibble','jsonlite','httr','data.table','caret','forecast'), repos='https://cloud.r-project.org', Ncpus=parallel::detectCores())\" && \
        curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && \
        apt-get install -y --no-install-recommends nodejs && \
        npm install -g typescript ts-node
      " 2>&1 | tail -20
  fi

  # Write the schema sentinel.
  echo "$SCHEMA" | sudo tee "$STAGE_DIR/.ziee-sandbox-rootfs-schema" >/dev/null

  # Strip setuid bits (defense in depth).
  sudo find "$STAGE_DIR" -xdev \( -perm /u+s -o -perm /g+s \) -type f \
    -exec chmod u-s,g-s {} \; 2>/dev/null || true
}

# --------------------------------------------------------------------
# Backend: docker (fallback)
# --------------------------------------------------------------------

build_docker() {
  if ! command -v docker >/dev/null; then
    echo "build.sh: docker not found; install docker or use mmdebstrap" >&2
    exit 1
  fi

  local image_tag="ziee-sandbox-rootfs-v${SCHEMA}.${REVISION}-${FLAVOR}"
  local stage_tar="$(dirname "$OUTPUT")/.stage-${image_tag}.tar"

  echo "==> docker build (flavor=$FLAVOR schema=$SCHEMA rev=$REVISION arch=$ARCH)"
  build_args=(
    --build-arg "ZIEE_SANDBOX_FLAVOR=$FLAVOR"
    --build-arg "ZIEE_SANDBOX_SCHEMA=$SCHEMA"
  )
  if [[ -n "$APT_CACHE" ]]; then
    build_args+=(--build-arg "http_proxy=$APT_CACHE" --build-arg "HTTP_PROXY=$APT_CACHE")
  fi
  docker build -f "$SCRIPT_DIR/Dockerfile" "${build_args[@]}" -t "$image_tag" "$SCRIPT_DIR"

  echo "==> docker export → tar"
  local container_id
  container_id="$(docker create "$image_tag")"
  trap 'docker rm -f "$container_id" >/dev/null 2>&1 || true; rm -rf "$STAGE_DIR"' EXIT
  docker export "$container_id" -o "$stage_tar"

  echo "==> extract tar → staging dir"
  tar -xf "$stage_tar" -C "$STAGE_DIR" --xattrs --acls --numeric-owner
  rm -f "$stage_tar"
}

# --------------------------------------------------------------------
# Dispatch + finalize
# --------------------------------------------------------------------

case "$BACKEND" in
  mmdebstrap) build_mmdebstrap ;;
  docker)     build_docker ;;
  *) echo "build.sh: unknown backend '$BACKEND' (want mmdebstrap|docker|auto)" >&2; exit 2 ;;
esac

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
