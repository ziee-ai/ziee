#!/bin/sh
# Ziee server installer (Linux). Modeled on coder/coder's install.sh.
#
#   curl -fsSL https://github.com/phibya/ziee-chat-new/releases/latest/download/install.sh | sh
#
# Detects your CPU arch + distro and installs ziee via the native package
# manager (.deb / .rpm) or a standalone musl binary to /usr/local/bin (Alpine
# uses the tarball — it has no systemd), then prints how to enable the systemd
# service. Re-run to UPDATE. Artifacts are sha256-verified before install; for
# the strongest check, verify the Sigstore attestation (see the runbook).
#
# Flags:
#   --version X.Y.Z     install a specific version (default: latest release)
#   --method M          detect | deb | rpm | standalone (default: detect)
#   --prefix DIR        standalone install prefix (default: /usr/local)
#   --dry-run           print what would happen; download/install nothing
#   --help              show this help
#
# The server is Linux-only. On macOS/Windows use the desktop app instead.
set -eu

REPO="phibya/ziee-chat-new"
APP="ziee"
GH="https://github.com"

VERSION=""
METHOD="detect"
PREFIX="/usr/local"
DRY_RUN=0

main() {
  parse_args "$@"

  OS=$(os)
  if [ "$OS" != "linux" ]; then
    echoerr "The $APP server is Linux-only (detected: $OS)."
    echoerr "On macOS/Windows, install the Ziee desktop app instead."
    exit 1
  fi
  ARCH=$(arch) || exit 1

  if [ -z "$VERSION" ]; then
    VERSION=$(latest_version)
    # The redirect parse must yield something version-like (starts with a digit),
    # not a stray URL / empty string.
    case "$VERSION" in
      [0-9]*) ;;
      # Resolution can fail without curl (BusyBox wget on Alpine doesn't expose
      # the final redirect URL the same way) — tell the user to pass --version.
      *) echoerr "could not resolve the latest version; pass --version X.Y.Z explicitly"; exit 1 ;;
    esac
  fi

  if [ "$METHOD" = "detect" ]; then
    METHOD=$(detect_method)
  fi

  EXT=$(ext_for_method "$METHOD")
  ASSET="${APP}_${VERSION}_linux_${ARCH}.${EXT}"
  URL="$GH/$REPO/releases/download/v${VERSION}/${ASSET}"

  echoh "Installing $APP v$VERSION ($ARCH) via '$METHOD'"
  echoh "  source: $URL"

  if [ "$DRY_RUN" = "1" ]; then
    echoh "  target: $(target_for_method "$METHOD")"
    echoh "[dry-run] nothing downloaded or installed"
    return 0
  fi

  TMP=$(mktemp -d)
  trap 'rm -rf "$TMP"' EXIT
  fetch "$URL" "$TMP/$ASSET"
  verify_checksum "$TMP/$ASSET" "$ASSET"

  case "$METHOD" in
    deb)        install_deb "$TMP/$ASSET" ;;
    rpm)        install_rpm "$TMP/$ASSET" ;;
    standalone) install_standalone "$TMP/$ASSET" ;;
    *)          echoerr "unknown method: $METHOD"; exit 1 ;;
  esac

  postinstall
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --version) VERSION="${2#v}"; shift 2 ;;
      --version=*) VERSION="${1#*=}"; VERSION="${VERSION#v}"; shift ;;
      --method) METHOD="$2"; shift 2 ;;
      --method=*) METHOD="${1#*=}"; shift ;;
      --prefix) PREFIX="$2"; shift 2 ;;
      --prefix=*) PREFIX="${1#*=}"; shift ;;
      --dry-run) DRY_RUN=1; shift ;;
      --help|-h) usage; exit 0 ;;
      *) echoerr "unknown flag: $1"; usage; exit 1 ;;
    esac
  done
}

usage() {
  # Print the comment header (every comment line after the shebang, up to the
  # first non-comment line), stripping the leading '# '. Robust to header edits.
  awk 'NR==1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "$0"
}

# linux | darwin | freebsd | ...
os() { uname -s | tr '[:upper:]' '[:lower:]'; }

# amd64 | arm64 (other arches are unsupported for the server)
arch() {
  case "$(uname -m)" in
    x86_64|amd64) echo amd64 ;;
    aarch64|arm64) echo arm64 ;;
    *) echoerr "unsupported architecture: $(uname -m) (server ships amd64 + arm64)"; return 1 ;;
  esac
}

# Read ID + ID_LIKE from /etc/os-release.
distro() {
  if [ -f /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    echo "${ID:-} ${ID_LIKE:-}"
  fi
}

detect_method() {
  # Alpine routes to the standalone tarball: it uses OpenRC, not systemd, so the
  # packaged systemd unit would be inert. (deb/rpm hosts get the native package
  # + a working systemd unit.)
  case " $(distro) " in
    *" debian "*|*" ubuntu "*) command -v dpkg >/dev/null 2>&1 && { echo deb; return; } ;;
    *" fedora "*|*" rhel "*|*" centos "*|*" suse "*|*" opensuse "*) command -v rpm >/dev/null 2>&1 && { echo rpm; return; } ;;
  esac
  echo standalone
}

ext_for_method() {
  case "$1" in
    deb) echo deb ;; rpm) echo rpm ;; standalone) echo tar.gz ;;
    *) echoerr "unsupported method: $1 (use deb | rpm | standalone)"; exit 1 ;;
  esac
}

target_for_method() {
  case "$1" in
    deb|rpm)    echo "/usr/bin/$APP (via package manager) + systemd unit" ;;
    standalone) echo "$PREFIX/bin/$APP" ;;
  esac
}

# Follow the releases/latest redirect and take the trailing tag (no API token).
# `tr -d '\r'`: the wget path reads a CRLF-terminated `Location:` header, so the
# captured value carries a trailing CR — strip it here so the version (and the
# URL built from it) never contains a stray \r that would 404 the download.
latest_version() {
  redirect=$(fetch_redirect "$GH/$REPO/releases/latest")
  echo "$redirect" | sed -E 's#.*/tag/v?##' | tr -d '\r'
}

install_deb() { echoh "  dpkg -i"; sudo_run dpkg -i "$1" || sudo_run apt-get install -f -y; }
install_rpm() { echoh "  rpm -U"; sudo_run rpm -U --replacepkgs "$1"; }

install_standalone() {
  echoh "  extracting to $PREFIX/bin"
  # Extract under the trapped $TMP (cleaned on EXIT) so a failure path can't leak.
  tmp="$TMP/extract"; mkdir -p "$tmp"
  tar -xzf "$1" -C "$tmp"
  bin=$(find "$tmp" -name "$APP" -type f | head -1)
  [ -n "$bin" ] || { echoerr "binary '$APP' not found in archive"; exit 1; }
  # -D creates the leading dirs so a fresh custom --prefix (e.g. /opt/ziee) works.
  sudo_run install -D -m 0755 "$bin" "$PREFIX/bin/$APP"
}

postinstall() {
  case "$METHOD" in
    deb|rpm)
      echoh ""
      echoh "Installed. Enable + start the service:"
      echoh "  sudo systemctl enable --now $APP"
      echoh "  journalctl -u $APP -f"
      echoh "Config: /etc/ziee/config.yaml   Data: /var/lib/ziee" ;;
    standalone)
      echoh ""
      echoh "Installed $APP to $PREFIX/bin/$APP (binary only — no service/config set up)."
      echoh "Get a starter config:"
      echoh "  curl -fsSLO $GH/$REPO/releases/download/v${VERSION}/config.default.yaml"
      echoh "Run with:  $APP --config-file ./config.default.yaml"
      echoh "For a managed service, set up systemd or (on Alpine) an OpenRC unit." ;;
  esac
  echoh ""
  case "$METHOD" in
    deb) sandbox_deps="sudo apt install bubblewrap squashfuse fuse3" ;;
    rpm) sandbox_deps="sudo dnf install bubblewrap squashfuse fuse3" ;;
    *)   sandbox_deps="install bubblewrap + squashfuse + fuse3 via your package manager" ;;
  esac
  echoh "Optional (code sandbox): $sandbox_deps"
}

# ---- helpers ---------------------------------------------------------------

fetch() {
  url="$1"; out="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$out"
  else
    echoerr "need curl or wget"; exit 1
  fi
}

# Verify a downloaded artifact against the release's sha256 checksums sidecar
# BEFORE installing it as root. Aborts on a missing or mismatched checksum.
verify_checksum() {
  file="$1"; name="$2"
  sums_url="$GH/$REPO/releases/download/v${VERSION}/${APP}_${VERSION}_checksums.txt"
  echoh "  verifying sha256..."
  if ! fetch "$sums_url" "$TMP/checksums.txt"; then
    echoerr "could not fetch checksums ($sums_url) — refusing to install unverified"
    exit 1
  fi
  expected=$(awk -v n="$name" '$2 == n { print $1 }' "$TMP/checksums.txt")
  if [ -z "$expected" ]; then
    echoerr "no checksum for $name — refusing to install unverified"
    exit 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$file" | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$file" | awk '{print $1}')
  else
    echoerr "no sha256sum/shasum available to verify the download"; exit 1
  fi
  if [ "$expected" != "$actual" ]; then
    echoerr "CHECKSUM MISMATCH for $name (expected $expected, got $actual)"
    exit 1
  fi
  echoh "  sha256 ok"
}

fetch_redirect() {
  url="$1"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSLI -o /dev/null -w '%{url_effective}' "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -S --max-redirect=10 -O /dev/null "$url" 2>&1 | awk '/^  Location: /{u=$2} END{sub(/\r$/,"",u); print u}'
  else
    echoerr "need curl or wget"; exit 1
  fi
}

sudo_run() {
  if [ "$(id -u)" -eq 0 ]; then "$@"; else sudo "$@"; fi
}

echoh() { echo "$@"; }
echoerr() { echo "$APP: $*" >&2; }

main "$@"
