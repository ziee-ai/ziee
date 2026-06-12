#!/bin/sh
# Package preinstall: create the ziee system user/group BEFORE files are
# unpacked, so the config file (shipped as group `ziee`, mode 0640) resolves
# its group ownership at unpack time. postinstall re-runs this idempotently.
set -eu

if ! getent group ziee >/dev/null 2>&1; then
  groupadd --system ziee 2>/dev/null || addgroup -S ziee 2>/dev/null || true
fi
if ! getent passwd ziee >/dev/null 2>&1; then
  useradd --system --gid ziee --home-dir /var/lib/ziee --shell /usr/sbin/nologin ziee 2>/dev/null \
    || adduser -S -G ziee -H -h /var/lib/ziee ziee 2>/dev/null || true
fi

exit 0
