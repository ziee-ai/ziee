#!/usr/bin/env bash
# Stand up a throwaway pgvector Postgres for SQLx compile-time verification.
#
# The ziee server has NO .sqlx offline cache: src-app/server/build.rs always
# connects to a live Postgres (default 127.0.0.1:54321), drops+recreates the
# public schema, applies every migration (which run `CREATE EXTENSION vector` /
# `pgcrypto`), and points the sqlx query macros at it. So `cargo build` needs a
# reachable pgvector-capable Postgres. We run one INSIDE the build layer so the
# whole image builds with a plain `docker build` — no external DB, no host
# state, no buildx networking.
#
# Postgres refuses to run as root, so the cluster runs as the `postgres` system
# user created by the postgresql-17 package. `trust` auth on loopback means the
# password in DATABASE_URL is ignored. This DB is discarded when the RUN layer
# ends; nothing from it lands in the final image.
set -euo pipefail

PGBIN="${PG_BUILD_BINDIR:-/usr/lib/postgresql/17/bin}"
PGDATA=/tmp/ziee-build-pgdata
PGPORT=54321

mkdir -p "$PGDATA"
chown -R postgres:postgres "$PGDATA"

su postgres -c "$PGBIN/initdb -D $PGDATA -U postgres --auth-local=trust --auth-host=trust --encoding=UTF8"

# Loopback-only, trust auth — matches the committed DATABASE_URL sentinel
# (postgresql://postgres:password@127.0.0.1:54321/postgres).
{
  echo "listen_addresses = '127.0.0.1'"
  echo "port = $PGPORT"
  echo "fsync = off"
  echo "synchronous_commit = off"
  echo "full_page_writes = off"
} >> "$PGDATA/postgresql.conf"
echo "host all all 127.0.0.1/32 trust" >> "$PGDATA/pg_hba.conf"

su postgres -c "$PGBIN/pg_ctl -D $PGDATA -w -t 60 start"

# Belt-and-suspenders: wait until it actually accepts connections.
for _ in $(seq 1 30); do
  if "$PGBIN/pg_isready" -h 127.0.0.1 -p "$PGPORT" -U postgres >/dev/null 2>&1; then
    echo "build DB ready on 127.0.0.1:$PGPORT"
    exit 0
  fi
  sleep 1
done

echo "build DB failed to become ready on 127.0.0.1:$PGPORT" >&2
exit 1
