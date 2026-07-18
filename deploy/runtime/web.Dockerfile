# ziee web (SPA) image — builds the Vite SPA and serves it via nginx, reverse-
# proxying /api to the SEPARATE API container (ziee-web:9000). This replaces the
# SPA half of the old fat docker/web/Dockerfile; the API half is now the thin
# root Dockerfile (host-compiled musl binary).
#
# SDK-model note: the UI is an npm workspace that now depends on the SDK
# workspace packages (@ziee/shell, @ziee/kit, @ziee/framework, …) under
# sdk/packages/* — so the build context MUST include the `sdk` submodule
# (init it first: git submodule update --init --recursive). The root
# package.json `workspaces` lists `sdk/packages/*`.
#
# Build context = REPO ROOT:
#   docker buildx build -f deploy/runtime/web.Dockerfile -t ziee-web-spa:local .

# ── Stage 1 — build the SPA ────────────────────────────────────────────────
FROM node:22-bookworm-slim AS ui-build
WORKDIR /app

# Restore workspace manifests first so `npm ci` caches across source edits. npm
# workspaces require EVERY workspace's package.json present to resolve the
# lockfile — that now includes all sdk/packages/*.
COPY package.json package-lock.json ./
COPY src-app/ui/package.json ./src-app/ui/package.json
COPY src-app/desktop/ui/package.json ./src-app/desktop/ui/package.json
COPY sdk/packages ./sdk/packages
RUN --mount=type=cache,target=/root/.npm npm ci

# The UI source (the SDK package source is already copied above via sdk/packages).
COPY src-app/ui ./src-app/ui
# `build` = `tsc && vite build`; vite's outDir is ../../dist/ui → src-app/dist/ui.
# Relocate to a canonical /webroot so the runtime COPY is unambiguous.
RUN --mount=type=cache,target=/root/.npm \
    npm run build --workspace @ziee/ui-core \
    && test -f /app/src-app/dist/ui/index.html \
    && mkdir -p /webroot \
    && cp -a /app/src-app/dist/ui/. /webroot/

# ── Stage 2 — nginx serving the SPA ────────────────────────────────────────
FROM nginx:1.27-alpine
COPY deploy/runtime/nginx.conf /etc/nginx/nginx.conf
COPY --from=ui-build /webroot /srv/www
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
  CMD wget -qO- http://127.0.0.1:8080/ >/dev/null 2>&1 || exit 1
