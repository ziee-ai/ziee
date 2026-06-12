#!/bin/sh
# Package preremove: stop + disable the service ONLY on a true removal, NOT on
# an upgrade (deb passes "upgrade"/"remove"; rpm passes "1"=upgrade/"0"=remove).
# Disabling on upgrade would leave the service stopped after every update.
# Leaves /var/lib/ziee (data) intact on purpose.
set -eu

case "${1:-}" in
  remove | purge | 0)
    if command -v systemctl >/dev/null 2>&1; then
      systemctl disable --now ziee 2>/dev/null || true
    fi
    ;;
esac

exit 0
