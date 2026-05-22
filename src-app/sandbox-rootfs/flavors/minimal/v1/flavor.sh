# Recipe for the "minimal" rootfs flavor (schema 1). Sourced by build.sh.
#
# Pure-apt: shell + coreutils + curl + jq + git + python3 interpreter.
# No `provision` function → build.sh skips the chroot layer entirely.

DESCRIPTION="Shell + coreutils + curl + jq + git + python3 (interpreter only)."
APPROX_SIZE_MB=57

# snapshot.ubuntu.com date for reproducible apt installs. Bump deliberately;
# CI's reproducibility check will catch silent drift.
APT_SNAPSHOT="20250115T000000Z"

# Whitespace/newline separated; build.sh collapses to mmdebstrap's comma list.
APT_PACKAGES="
  bash coreutils util-linux ca-certificates curl wget bzip2 xz-utils unzip
  locales tzdata python3 python3-pip python3-venv
"
