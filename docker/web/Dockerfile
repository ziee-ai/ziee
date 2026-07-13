# syntax=docker/dockerfile:1.7
#
# ziee-web — single self-contained web image.
#
# Serves the built React SPA (src-app/ui) via nginx and reverse-proxies /api to
# the Rust `ziee` API server running on loopback in the same container. One
# artifact for CI to build (`docker build`) and push — no GitHub Actions.
#
# Three stages:
#   ui-build     — node: `npm ci` + `vite build` → dist/ui
#   server-build — rust (glibc): stands up an EPHEMERAL pgvector Postgres in the
#                  build layer (SQLx compile-time verification needs a live DB —
#                  build.rs wipes+migrates it; there is no .sqlx offline cache),
#                  then `cargo build --release`.
#   runtime      — debian-slim + nginx + the binary; non-root; SSE-safe proxy.
#
# No build secret is required: the hub seed is tracked in-repo
# (src-app/server/resources/hub-seed/) — the old GITHUB_TOKEN/hub-seed network
# fetch was removed. The builder DOES need plain (unauthenticated) public
# network egress:
#   - crates.io + npm registry (deps)
#   - github.com: the pinned pgvector source clone
#   - theseus-rs release binaries: matching PostgreSQL (for pgvector) + the
#     embedded-PG bundled into the binary
#   - github.com releases: pandoc, typst, pdfium, uv, bun, biomcp — fetched by
#     build.rs and WARN-AND-CONTINUE (stub on failure). If those hosts are
#     blocked the build still SUCCEEDS but ships without doc-conversion /
#     stdio-MCP tooling / biomcp. Only hub-seed + pgvector are hard failures.

# ─────────────────────────────────────────────────────────────────────────────
# Stage 1 — build the web SPA
# ─────────────────────────────────────────────────────────────────────────────
FROM node:22-bookworm-slim AS ui-build
WORKDIR /app

# Restore just the manifests first so `npm ci` caches across source edits.
# npm workspaces (root package.json lists both UI workspaces) require every
# workspace's package.json to be present for `npm ci` to resolve the lockfile.
COPY package.json package-lock.json ./
COPY src-app/ui/package.json ./src-app/ui/package.json
COPY src-app/desktop/ui/package.json ./src-app/desktop/ui/package.json
RUN --mount=type=cache,target=/root/.npm npm ci

# Only the web UI is needed (the desktop workspace is a separate app).
COPY src-app/ui ./src-app/ui
# `build` = `tsc && vite build`. vite resolves `outDir: ../../dist/ui` relative
# to its project root (`root: 'src'` → src-app/ui/src), so the bundle lands at
# src-app/dist/ui — NOT the repo-root dist/ui. Relocate it to a canonical
# /webroot so the runtime COPY is unambiguous.
RUN set -eux; \
    npm run build --workspace @ziee/ui-core; \
    test -f /app/src-app/dist/ui/index.html; \
    mkdir -p /webroot; \
    cp -a /app/src-app/dist/ui/. /webroot/
# → /webroot (built SPA)

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2 — build the ziee server binary (glibc, native)
# ─────────────────────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS server-build
WORKDIR /app

# Build deps:
#   build-essential + perl  → vendored openssl/libgit2 (git2) + pgvector make
#   git                     → clone the pgvector source (submodule can't init;
#                             .dockerignore excludes .git from the build context)
#   ocl-icd-opencl-dev / libvulkan-dev → link deps for the `gpu-detect` feature
#   postgresql-17 + pgvector (PGDG) → the ephemeral SQLx build DB
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        build-essential perl pkg-config git ca-certificates gnupg wget \
        ocl-icd-opencl-dev libvulkan-dev; \
    install -d /usr/share/postgresql-common/pgdg; \
    wget -q -O /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc \
        https://www.postgresql.org/media/keys/ACCC4CF8.asc; \
    echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] http://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" \
        > /etc/apt/sources.list.d/pgdg.list; \
    apt-get update; \
    apt-get install -y --no-install-recommends postgresql-17 postgresql-17-pgvector; \
    rm -rf /var/lib/apt/lists/*

# Build-time DB isolation off: use the committed DATABASE_URL sentinel
# (127.0.0.1:54321) directly instead of the per-worktree keyed schema.
ENV ZIEE_BUILD_DB_PERWORKTREE=0
ENV PG_BUILD_BINDIR=/usr/lib/postgresql/17/bin
# Pin the PG version + build DATABASE_URL explicitly (backstops src-app/.cargo/
# config.toml's [env]; identical values). The URL points at build-db-init.sh's
# ephemeral cluster.
ENV POSTGRESQL_VERSION="=18.3.0" \
    ZIEE_POSTGRES_VERSION="18.3.0" \
    DATABASE_URL="postgresql://postgres:password@127.0.0.1:54321/postgres"

# Whole repo (trimmed by .dockerignore). src-app/.cargo/config.toml and
# src-app/server/resources/hub-seed/ are preserved by .dockerignore.
COPY . .

# The pgvector submodule is uninitialized in the context (.git is excluded), so
# clone the pinned tag directly. build_helper/pgvector.rs runs `make` against it.
RUN set -eux; \
    rm -rf src-app/server/vendor/pgvector; \
    git clone --depth 1 --branch v0.7.4 \
        https://github.com/pgvector/pgvector.git src-app/server/vendor/pgvector

COPY docker/web/build-db-init.sh /usr/local/bin/build-db-init.sh
RUN chmod +x /usr/local/bin/build-db-init.sh

# Start the throwaway pgvector Postgres on :54321 and build in the same layer
# (background daemons don't survive across RUN layers). build.rs connects,
# drops+recreates public, applies server + desktop migrations, and points the
# sqlx query macros at it. Release feature set mirrors the CI release build:
# seccomp OFF (--no-default-features), gpu-detect ON.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    set -eux; \
    /usr/local/bin/build-db-init.sh; \
    cd src-app/server; \
    cargo build --release --no-default-features --features gpu-detect; \
    cp /app/src-app/target/release/ziee /usr/local/bin/ziee
# → /usr/local/bin/ziee

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2b — validate nginx.conf keeps the SSE-critical directives
#   The runtime COPYs nginx.conf FROM this stage, so the build graph depends on
#   it and the guard runs on every image build (an unreferenced stage would be
#   skipped). If `check-sse-headers.mjs` fails — proxy_buffering off or
#   X-Accel-Buffering no missing from `location /api` — the image build fails,
#   so a config edit that re-breaks SSE through a buffering edge (the Coder
#   published URL) can never ship. Reuses the node base already pulled for
#   ui-build; copies only the two files it needs.
# ─────────────────────────────────────────────────────────────────────────────
FROM node:22-bookworm-slim AS config-check
WORKDIR /app
COPY docker/web/nginx.conf docker/web/check-sse-headers.mjs ./docker/web/
RUN node docker/web/check-sse-headers.mjs

# ─────────────────────────────────────────────────────────────────────────────
# Stage 3 — runtime
# ─────────────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# ocl-icd-libopencl1 + libvulkan1: the `gpu-detect` feature links OpenCL and
# loads Vulkan; without the OpenCL ICD loader (libOpenCL.so.1) the dynamically
# linked binary would fail to exec on this slim base. With no real GPU/ICD
# present, detection just returns empty — the loader lib only needs to resolve.
#
# bubblewrap + squashfuse + fuse3: the OPTIONAL code_sandbox host deps.
# `bubblewrap`→bwrap (the boot probe requires it on PATH), `squashfuse`→mount the
# rootfs, `fuse3`→fusermount3 AND a `/usr/bin/fusermount` compat symlink (which
# runtime_mount.rs calls by that exact name). Always shipped, but inert unless
# ZIEE_CODE_SANDBOX_ENABLED=true (see docker-compose.sandbox.yaml) — with the
# sandbox off they add ~a few MB and are never exec'd. (This is the same set the
# CLAUDE.md operator runtime deps list.)
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        nginx ca-certificates tini gettext-base wget \
        ocl-icd-libopencl1 libvulkan1 \
        bubblewrap squashfuse fuse3; \
    rm -rf /var/lib/apt/lists/*; \
    groupadd -r ziee; \
    useradd -r -g ziee -d /var/lib/ziee -s /usr/sbin/nologin ziee; \
    mkdir -p /var/lib/ziee/data /srv/www /etc/ziee \
             /tmp/ziee /var/lib/nginx /var/lib/nginx/body; \
    chown -R ziee:ziee /var/lib/ziee /tmp/ziee /var/lib/nginx

COPY --from=server-build /usr/local/bin/ziee /usr/local/bin/ziee
COPY --from=ui-build     /webroot            /srv/www
COPY --from=config-check /app/docker/web/nginx.conf /etc/nginx/nginx.conf
COPY docker/web/config.template.yaml  /etc/ziee/config.template.yaml
COPY docker/web/entrypoint.sh         /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/ziee

# Config defaults (override at `docker run`/compose time). DB creds default to
# the bundled compose Postgres; point ZIEE_DB_* at a managed Postgres to deploy
# the SAME image against an external DB. jwt/storage are CHANGE-ME dev defaults.
ENV HOME=/var/lib/ziee \
    ZIEE_DB_HOST=postgres \
    ZIEE_DB_PORT=5432 \
    ZIEE_DB_USER=ziee \
    ZIEE_DB_PASSWORD=ziee \
    ZIEE_DB_NAME=ziee \
    ZIEE_JWT_SECRET=ziee-local-dev-jwt-secret-change-me-at-least-32-chars \
    ZIEE_STORAGE_KEY=ziee-local-dev-storage-key-change-me-at-least-32-chars \
    ZIEE_CORS_ALLOW_ORIGIN=http://localhost:8080 \
    ZIEE_UPDATE_CHECK=false \
    ZIEE_CODE_SANDBOX_ENABLED=false \
    ZIEE_LOG_LEVEL=info \
    ZIEE_LOG_FORMAT=json \
    ZIEE_MAX_FILE_UPLOAD_MB=128

USER ziee
WORKDIR /var/lib/ziee
VOLUME ["/var/lib/ziee"]
EXPOSE 8080

# Probe through nginx → proves the SPA host AND the /api proxy AND the server.
HEALTHCHECK --interval=30s --timeout=5s --start-period=90s --retries=5 \
    CMD wget -qO- http://127.0.0.1:8080/api/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
