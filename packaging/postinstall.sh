#!/bin/sh
# Package postinstall: create the ziee system user + data dir, generate a
# random JWT secret on first install, and print the systemd next steps.
set -eu

# 1. System user/group (no login, no home beyond the state dir).
if ! getent group ziee >/dev/null 2>&1; then
  groupadd --system ziee 2>/dev/null || addgroup -S ziee 2>/dev/null || true
fi
if ! getent passwd ziee >/dev/null 2>&1; then
  useradd --system --gid ziee --home-dir /var/lib/ziee --shell /usr/sbin/nologin ziee 2>/dev/null \
    || adduser -S -G ziee -H -h /var/lib/ziee ziee 2>/dev/null || true
fi

# 2. Data dir + config perms (the config holds the jwt secret → not world-readable).
mkdir -p /var/lib/ziee/data
chown -R ziee:ziee /var/lib/ziee
chmod 0750 /var/lib/ziee
if [ -d /etc/ziee ]; then
  chown -R ziee:ziee /etc/ziee
  chmod 0750 /etc/ziee
  [ -f /etc/ziee/config.yaml ] && chmod 0640 /etc/ziee/config.yaml
fi

# 3. Generate random secrets on first install (placeholders → real values).
CONF=/etc/ziee/config.yaml
if [ -f "$CONF" ] && grep -q "REPLACE_ME_GENERATED_AT_INSTALL" "$CONF"; then
  SECRET=$(head -c 48 /dev/urandom | od -An -tx1 | tr -d ' \n')
  if [ "${#SECRET}" -lt 32 ]; then
    echo "ziee: failed to generate a jwt secret (got ${#SECRET} chars)" >&2
    exit 1
  fi
  sed -i "s/REPLACE_ME_GENERATED_AT_INSTALL/${SECRET}/" "$CONF"
  echo "ziee: generated a random jwt.secret in $CONF"
fi
if [ -f "$CONF" ] && grep -q "REPLACE_ME_PG_PASSWORD_AT_INSTALL" "$CONF"; then
  PGPW=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
  if [ "${#PGPW}" -lt 16 ]; then
    echo "ziee: failed to generate a postgres password (got ${#PGPW} chars)" >&2
    exit 1
  fi
  sed -i "s/REPLACE_ME_PG_PASSWORD_AT_INSTALL/${PGPW}/" "$CONF"
  echo "ziee: generated a random embedded-postgres password in $CONF"
fi

# 4. systemd.
if command -v systemctl >/dev/null 2>&1; then
  systemctl daemon-reload 2>/dev/null || true
  echo ""
  echo "Ziee installed. Enable + start the service:"
  echo "  sudo systemctl enable --now ziee"
  echo "  journalctl -u ziee -f"
fi

exit 0
