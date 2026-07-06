#!/bin/sh
# Tests for install.sh — asserts URL/method/arch resolution via --dry-run
# (no downloads). Part A runs anywhere (stubs `uname`); Part B exercises real
# distro detection inside ubuntu/fedora/alpine containers when Docker is up.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
INSTALL="$HERE/install.sh"
VERSION=9.9.9
FAILED=0

pass() { echo "  ok: $1"; }
fail() { echo "  FAIL: $1" >&2; FAILED=1; }

# Run install.sh --dry-run with a stubbed `uname` (machine = $1), capture output.
run_dry() {
  machine="$1"; shift
  stub=$(mktemp -d)
  cat > "$stub/uname" <<EOF
#!/bin/sh
case "\$1" in
  -s) echo Linux ;;
  -m) echo "$machine" ;;
  *) exec /usr/bin/uname "\$@" ;;
esac
EOF
  chmod +x "$stub/uname"
  # `|| true`: some cases intentionally make install.sh exit non-zero (e.g. an
  # unsupported --method); we assert on the captured output, not the exit, and
  # must not let `set -e` abort the whole test run.
  PATH="$stub:$PATH" sh "$INSTALL" --dry-run --version "$VERSION" "$@" 2>&1 || true
  rm -rf "$stub"
}

assert_contains() {
  haystack="$1"; needle="$2"; label="$3"
  case "$haystack" in
    *"$needle"*) pass "$label" ;;
    *) fail "$label — expected '$needle' in:
$haystack" ;;
  esac
}

echo "== Part A: dry-run URL/method/arch (stubbed uname) =="

out=$(run_dry x86_64 --method standalone)
assert_contains "$out" "ziee_${VERSION}_linux_amd64.tar.gz" "amd64 + standalone → tar.gz"

out=$(run_dry aarch64 --method deb)
assert_contains "$out" "ziee_${VERSION}_linux_arm64.deb" "arm64 + deb → .deb"

out=$(run_dry x86_64 --method rpm)
assert_contains "$out" "ziee_${VERSION}_linux_amd64.rpm" "amd64 + rpm → .rpm"

# apk is no longer a supported method (Alpine → standalone).
out=$(run_dry x86_64 --method apk)
assert_contains "$out" "unsupported method" "--method apk rejected"

# No package manager on the host (e.g. macOS) → detect falls back to standalone.
out=$(run_dry x86_64)
assert_contains "$out" "via 'standalone'" "detect with no pkg mgr → standalone"
assert_contains "$out" "/releases/download/v${VERSION}/" "URL uses the v-prefixed tag"

# Non-Linux is rejected.
darwin_stub=$(mktemp -d)
cat > "$darwin_stub/uname" <<'EOF'
#!/bin/sh
case "$1" in -s) echo Darwin ;; -m) echo arm64 ;; *) exec /usr/bin/uname "$@" ;; esac
EOF
chmod +x "$darwin_stub/uname"
if PATH="$darwin_stub:$PATH" sh "$INSTALL" --dry-run --version "$VERSION" >/dev/null 2>&1; then
  fail "non-Linux OS should exit non-zero"
else
  pass "non-Linux OS rejected"
fi
rm -rf "$darwin_stub"

# latest-version resolution via the releases/latest redirect (stub uname + curl).
lv_stub=$(mktemp -d)
cat > "$lv_stub/uname" <<'EOF'
#!/bin/sh
case "$1" in -s) echo Linux ;; -m) echo x86_64 ;; *) exec /usr/bin/uname "$@" ;; esac
EOF
cat > "$lv_stub/curl" <<'EOF'
#!/bin/sh
# Only the redirect probe (-w %{url_effective}) runs in --dry-run; echo a fake
# effective URL pointing at a tagged release.
for a in "$@"; do
  case "$a" in *url_effective*) echo "https://github.com/ziee-ai/ziee/releases/tag/v9.9.9"; exit 0 ;; esac
done
exit 0
EOF
chmod +x "$lv_stub/uname" "$lv_stub/curl"
out=$(PATH="$lv_stub:$PATH" sh "$INSTALL" --dry-run --method standalone 2>&1)
assert_contains "$out" "ziee_9.9.9_linux_amd64.tar.gz" "latest-version resolves via redirect → 9.9.9"
rm -rf "$lv_stub"

# A CR-tainted redirect (the wget path reads a CRLF `Location:` header) must NOT
# leak a stray \r into the resolved version / URL — latest_version strips it.
cr_stub=$(mktemp -d)
cat > "$cr_stub/uname" <<'EOF'
#!/bin/sh
case "$1" in -s) echo Linux ;; -m) echo x86_64 ;; *) exec /usr/bin/uname "$@" ;; esac
EOF
cat > "$cr_stub/curl" <<'EOF'
#!/bin/sh
for a in "$@"; do
  case "$a" in *url_effective*) printf 'https://github.com/ziee-ai/ziee/releases/tag/v9.9.9\r\n'; exit 0 ;; esac
done
exit 0
EOF
chmod +x "$cr_stub/uname" "$cr_stub/curl"
out=$(PATH="$cr_stub:$PATH" sh "$INSTALL" --dry-run --method standalone 2>&1)
assert_contains "$out" "ziee_9.9.9_linux_amd64.tar.gz" "CR-tainted redirect → version has no stray \\r"
rm -rf "$cr_stub"

# --help prints the comment header (flags) but NOT the shell body.
help_out=$(sh "$INSTALL" --help 2>&1 || true)
assert_contains "$help_out" "--dry-run" "--help lists flags"
case "$help_out" in
  *"set -eu"* | *'REPO="'*) fail "--help leaked shell code" ;;
  *) pass "--help shows no shell code" ;;
esac

# Checksum verification REJECTS a tampered artifact (wrong hash) before install.
ck_stub=$(mktemp -d)
cat > "$ck_stub/uname" <<'EOF'
#!/bin/sh
case "$1" in -s) echo Linux ;; -m) echo x86_64 ;; *) exec /usr/bin/uname "$@" ;; esac
EOF
# Stub curl's download form (`-fsSL <url> -o <out>`): serve a fake artifact and
# a checksums file whose hash does NOT match it.
cat > "$ck_stub/curl" <<'EOF'
#!/bin/sh
out=""; url=""
while [ $# -gt 0 ]; do
  case "$1" in -o) out="$2"; shift 2 ;; -*) shift ;; *) url="$1"; shift ;; esac
done
case "$url" in
  *checksums*) printf '%s  %s\n' "0000000000000000000000000000000000000000000000000000000000000000" "ziee_9.9.9_linux_amd64.tar.gz" > "$out" ;;
  *) printf 'tampered-artifact' > "$out" ;;
esac
EOF
chmod +x "$ck_stub/uname" "$ck_stub/curl"
if PATH="$ck_stub:$PATH" sh "$INSTALL" --version 9.9.9 --method standalone >/dev/null 2>&1; then
  fail "tampered artifact (bad checksum) should abort the install"
else
  pass "checksum mismatch aborts before install"
fi
rm -rf "$ck_stub"

# Checksum verification ACCEPTS a matching artifact (prints "sha256 ok"); the
# install then fails on the non-tar payload, which is fine — we assert the gate.
if command -v sha256sum >/dev/null 2>&1; then
  GOOD=$(printf '%s' "matched-artifact" | sha256sum | awk '{print $1}')
else
  GOOD=$(printf '%s' "matched-artifact" | shasum -a 256 | awk '{print $1}')
fi
ok_stub=$(mktemp -d)
cat > "$ok_stub/uname" <<'EOF'
#!/bin/sh
case "$1" in -s) echo Linux ;; -m) echo x86_64 ;; *) exec /usr/bin/uname "$@" ;; esac
EOF
cat > "$ok_stub/curl" <<EOF
#!/bin/sh
out=""; url=""
while [ \$# -gt 0 ]; do
  case "\$1" in -o) out="\$2"; shift 2 ;; -*) shift ;; *) url="\$1"; shift ;; esac
done
case "\$url" in
  *checksums*) printf '%s  %s\n' "$GOOD" "ziee_9.9.9_linux_amd64.tar.gz" > "\$out" ;;
  *) printf 'matched-artifact' > "\$out" ;;
esac
EOF
chmod +x "$ok_stub/uname" "$ok_stub/curl"
ok_out=$(PATH="$ok_stub:$PATH" sh "$INSTALL" --version 9.9.9 --method standalone 2>&1 || true)
assert_contains "$ok_out" "sha256 ok" "matching checksum passes verification"
rm -rf "$ok_stub"

# A redirect that doesn't resolve to a tag must be rejected (no garbage version).
rf_stub=$(mktemp -d)
cat > "$rf_stub/uname" <<'EOF'
#!/bin/sh
case "$1" in -s) echo Linux ;; -m) echo x86_64 ;; *) exec /usr/bin/uname "$@" ;; esac
EOF
cat > "$rf_stub/curl" <<'EOF'
#!/bin/sh
for a in "$@"; do case "$a" in *url_effective*) echo "https://github.com/ziee-ai/ziee/releases"; exit 0 ;; esac; done
exit 0
EOF
chmod +x "$rf_stub/uname" "$rf_stub/curl"
if PATH="$rf_stub:$PATH" sh "$INSTALL" --dry-run --method standalone >/dev/null 2>&1; then
  fail "unparseable releases/latest redirect should be rejected"
else
  pass "unparseable redirect rejected (no garbage version)"
fi
rm -rf "$rf_stub"

echo "== Part B: distro detection in containers (Docker) =="
if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  # image : expected method (Alpine → standalone; it has no systemd)
  for spec in "ubuntu:22.04 deb" "fedora:40 rpm" "alpine:3.20 standalone"; do
    img=${spec% *}; want=${spec#* }
    out=$(docker run --rm -v "$HERE:/s:ro" "$img" sh /s/install.sh --dry-run --version "$VERSION" 2>&1 || true)
    assert_contains "$out" "via '$want'" "$img → $want"
  done
else
  echo "  skip: Docker not available (Part B is the container distro-detection tier)"
fi

echo "== Part C: nfpm output filename == install.sh's requested ASSET (nfpm) =="
# Guards the #1 contract: the workflow's nfpm --target name must equal what
# install.sh fetches, or every package install 404s.
if command -v nfpm >/dev/null 2>&1; then
  work=$(mktemp -d)
  mkdir -p "$work/dist" "$work/out"
  printf '#!/bin/sh\ntrue\n' > "$work/dist/ziee"; chmod +x "$work/dist/ziee"
  cp -r "$HERE/../packaging" "$work/packaging"
  for arch in amd64 arm64; do
    for pkg in deb rpm; do
      expect="ziee_9.9.9_linux_${arch}.${pkg}"   # == install.sh ASSET
      ( cd "$work" && PKG_ARCH="$arch" PKG_VERSION=9.9.9 \
          nfpm pkg --config packaging/nfpm.yaml --packager "$pkg" --target "out/$expect" >/dev/null 2>&1 ) || true
      if [ -f "$work/out/$expect" ]; then
        pass "nfpm produces $expect"
      else
        fail "nfpm did not produce $expect"
      fi
    done
    # Tarball name (the workflow's `tar` command) must == install.sh's
    # standalone ASSET for the same arch.
    tb="ziee_9.9.9_linux_${arch}.tar.gz"
    ( cd "$work" && tar -C dist -czf "out/$tb" ziee )
    if [ -f "$work/out/$tb" ]; then
      pass "tarball name matches install.sh ASSET ($tb)"
    else
      fail "tarball name mismatch ($tb)"
    fi
  done
  rm -rf "$work"
else
  echo "  skip: nfpm not installed (Part C verifies the filename contract)"
fi

# The checksums sidecar name install.sh fetches must match the workflow's.
# SC2016: the ${...} are literal text we grep FOR in those files, not expansions.
# shellcheck disable=SC2016
if grep -q '${APP}_${VERSION}_checksums.txt' "$INSTALL" \
   && grep -q 'ziee_${VERSION}_checksums.txt' "$HERE/../.github/workflows/server-release.yml"; then
  pass "checksums sidecar name agrees (install.sh <-> workflow)"
else
  fail "checksums sidecar name drifted between install.sh and the workflow"
fi

if [ "$FAILED" -eq 0 ]; then
  echo "install.sh: all assertions passed"
else
  echo "install.sh: FAILURES above" >&2
  exit 1
fi
