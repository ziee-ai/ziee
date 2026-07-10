#!/usr/bin/env bash
# ziee-web entrypoint: render the server config from env, then run BOTH the
# ziee API server (loopback:9000) and nginx (public:8080) under tini (PID 1),
# tearing the container down if either process exits.
set -euo pipefail

MOUNTED_CONFIG=/etc/ziee/config.yaml
TEMPLATE=/etc/ziee/config.template.yaml
RENDERED=/tmp/ziee/config.yaml

mkdir -p /tmp/ziee /tmp/nginx-client /tmp/nginx-proxy /tmp/nginx-fastcgi \
         /tmp/nginx-uwsgi /tmp/nginx-scgi

# Loudly warn if the image's baked CHANGE-ME secret defaults are still in use —
# they are public, so a session token forged with them would validate. Fine for
# local testing; never for a real deployment (set ZIEE_JWT_SECRET / ZIEE_STORAGE_KEY).
case "${ZIEE_JWT_SECRET:-}" in *change-me*) echo "ziee-web: WARNING — using the built-in default ZIEE_JWT_SECRET; set a real one for anything but local testing." >&2 ;; esac
case "${ZIEE_STORAGE_KEY:-}" in *change-me*) echo "ziee-web: WARNING — using the built-in default ZIEE_STORAGE_KEY; set a real one (losing it makes stored secrets unrecoverable)." >&2 ;; esac

if [ -f "$MOUNTED_CONFIG" ]; then
    echo "ziee-web: using mounted config $MOUNTED_CONFIG"
    CONFIG="$MOUNTED_CONFIG"
else
    echo "ziee-web: rendering $TEMPLATE -> $RENDERED"
    # Substitute ONLY our known vars so incidental '$' in values is left intact.
    envsubst '${ZIEE_DB_HOST} ${ZIEE_DB_PORT} ${ZIEE_DB_USER} ${ZIEE_DB_PASSWORD} ${ZIEE_DB_NAME} ${ZIEE_JWT_SECRET} ${ZIEE_STORAGE_KEY} ${ZIEE_CORS_ALLOW_ORIGIN} ${ZIEE_UPDATE_CHECK} ${ZIEE_LOG_LEVEL} ${ZIEE_LOG_FORMAT} ${ZIEE_CODE_SANDBOX_ENABLED} ${ZIEE_MAX_FILE_UPLOAD_MB}' \
        < "$TEMPLATE" > "$RENDERED"
    CONFIG="$RENDERED"
fi

ziee_pid=""
nginx_pid=""

shutdown() {
    trap - TERM INT
    [ -n "$ziee_pid" ]  && kill -TERM "$ziee_pid"  2>/dev/null || true
    [ -n "$nginx_pid" ] && kill -TERM "$nginx_pid" 2>/dev/null || true
    wait 2>/dev/null || true
}
trap shutdown TERM INT

echo "ziee-web: starting ziee server (loopback:9000)"
/usr/local/bin/ziee --config-file "$CONFIG" &
ziee_pid=$!

echo "ziee-web: starting nginx (0.0.0.0:8080)"
nginx -c /etc/nginx/nginx.conf &
nginx_pid=$!

# Exit as soon as EITHER process dies, propagating its status so Docker restarts
# the container (and the healthcheck flips unhealthy). `|| true` keeps `set -e`
# from short-circuiting the graceful sibling-kill below when a child crashes.
wait -n "$ziee_pid" "$nginx_pid" || true
status=$?
echo "ziee-web: a supervised process exited (status $status) — shutting down"
shutdown
exit "$status"
