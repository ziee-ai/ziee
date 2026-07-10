# ziee-web — single deployable web image

A single, self-contained Docker image that serves the ziee web app: the Rust
`ziee` API server **and** the built React SPA in one container. Inside the
container **nginx** serves the SPA on `:8080` and reverse-proxies `/api` (incl.
all SSE/streaming endpoints) to the `ziee` server on loopback `:9000`.

- **Multi-stage build** (`docker/web/Dockerfile`): builds the SPA (`node`),
  builds the server binary (`rust`, glibc), assembles a small `debian-slim`
  runtime.
- **No build secret required.** The hub seed is tracked in-repo, so the old
  `GITHUB_TOKEN` / hub-seed network fetch is gone. The builder needs only plain
  public network egress (crates, npm, the pinned pgvector source, and the
  theseus PostgreSQL binaries).
- **Bundled Postgres for local testing**, or point the same image at an
  **external managed Postgres** by changing env only.

> On this host the Docker daemon is reachable only via `sudo` (the shell user is
> not in the `docker` group). Prefix the commands below with `sudo` as needed.

---

## Quick start (bundled Postgres)

```bash
# from the repo root
docker compose up --build
```

Then open <http://localhost:8080>. First boot runs DB migrations, so give the
`ziee-web` healthcheck up to ~90s to go healthy.

- Website: <http://localhost:8080>
- API (proxied): <http://localhost:8080/api/health> → `{"status":"ok"}`

Tear down (keep data): `docker compose down` — or `docker compose down -v` to
also drop the Postgres + app volumes.

---

## Ports

| Port | Where | What |
|------|-------|------|
| `8080` | published | nginx: SPA + `/api` reverse-proxy (the only exposed port) |
| `9000` | container-internal | `ziee` API server (loopback only, never published) |
| `5432` | postgres service (compose network) | bundled Postgres |

---

## Configuration

The server reads config **only** from a YAML file — there are no per-key env
overrides in the server itself. The image ships `config.template.yaml` and
renders it at start (via `envsubst`) from these env vars:

| Env var | Default | Meaning |
|---|---|---|
| `ZIEE_DB_HOST` | `postgres` | external Postgres host |
| `ZIEE_DB_PORT` | `5432` | external Postgres port |
| `ZIEE_DB_USER` | `ziee` | DB user |
| `ZIEE_DB_PASSWORD` | `ziee` | DB password |
| `ZIEE_DB_NAME` | `ziee` | DB name |
| `ZIEE_JWT_SECRET` | dev default | JWT signing secret (**set >=32 chars in prod**) |
| `ZIEE_STORAGE_KEY` | dev default | at-rest secret encryption key (**set >=32 chars in prod**) |
| `ZIEE_UPDATE_CHECK` | `false` | outbound update-check toggle |
| `ZIEE_CODE_SANDBOX_ENABLED` | `false` | enable the bwrap code sandbox (see "Code sandbox (opt-in)" below) |
| `ZIEE_LOG_LEVEL` / `ZIEE_LOG_FORMAT` | `info` / `json` | logging |

The target Postgres **must** have the `vector` (pgvector) and `pgcrypto`
extensions available — migrations run at boot and create them. Use a
pgvector-capable image (the compose stack uses `pgvector/pgvector:pg18`).

> **Managed-DB note:** `pgcrypto` is not a "trusted" extension, so the boot
> migration's `CREATE EXTENSION vector` / `CREATE EXTENSION pgcrypto` needs a
> role that can create extensions (a superuser, or `rds_superuser` /
> `cloudsqlsuperuser` on RDS / Cloud SQL). On a managed DB where the app role
> is not privileged, have an admin pre-run `CREATE EXTENSION vector;
> CREATE EXTENSION pgcrypto;` once — the `IF NOT EXISTS` migrations then no-op.
> (The bundled compose Postgres runs `ziee` as a superuser, so it just works.)

Values with a `"`, backslash, or newline can break the rendered YAML — for
secrets containing such characters, bind-mount a config file instead (below).

**Full control:** bind-mount your own YAML at `/etc/ziee/config.yaml` and it is
used verbatim (the template/env path is skipped). Base it on
`src-app/server/config/prod.example.yaml`.

---

## External / managed Postgres (the deploy shape)

The same image runs against any external Postgres by changing env only:

```bash
export ZIEE_DB_HOST=db.internal ZIEE_DB_USER=ziee ZIEE_DB_PASSWORD=secret ZIEE_DB_NAME=ziee
export ZIEE_JWT_SECRET='a-long-random-secret-at-least-32-characters'
export ZIEE_STORAGE_KEY='a-long-random-storage-key-at-least-32-characters'

docker compose -f docker-compose.external-db.yml up --build
```

or with a plain `docker run` against a prebuilt image:

```bash
docker run --rm -p 8080:8080 \
  -e ZIEE_DB_HOST=db.internal -e ZIEE_DB_USER=ziee -e ZIEE_DB_PASSWORD=secret -e ZIEE_DB_NAME=ziee \
  -e ZIEE_JWT_SECRET='a-long-random-secret-at-least-32-characters' \
  -e ZIEE_STORAGE_KEY='a-long-random-storage-key-at-least-32-characters' \
  -v ziee-data:/var/lib/ziee \
  ziee-web:local
```

---

## Building the image directly (TeamCity)

The whole thing builds with a plain `docker build` — **no GitHub Actions, no
build secret**:

```bash
docker build -f docker/web/Dockerfile -t <registry>/ziee-web:<tag> .
docker push <registry>/ziee-web:<tag>
```

TeamCity build step notes:

- Context is the **repo root** (`.`); the Dockerfile is `docker/web/Dockerfile`.
- Needs BuildKit (Docker >= 23 / buildx). The build clones the pinned pgvector
  source and downloads the theseus PostgreSQL binaries, so the build agent needs
  **outbound network** to `github.com`, `crates.io`, and the npm registry.
- No `--secret` / `--build-arg` is required. (The `GITHUB_TOKEN` the old runbook
  mentioned is obsolete — the hub seed is tracked in-repo.)
- Enable layer caching (`--cache-to`/`--cache-from` or a persistent buildx
  builder) to avoid recompiling Rust on every run.

---

## How it works

- **`docker/web/Dockerfile`** — 3 stages:
  - `ui-build` (`node:22`) — `npm ci` (workspaces) + `vite build` → `dist/ui`.
  - `server-build` (`rust:1-bookworm`) — SQLx compile-time verification needs a
    live pgvector Postgres (there is no `.sqlx` offline cache and `build.rs`
    actively migrates a DB), so `build-db-init.sh` starts an **ephemeral pgvector
    Postgres in the build layer** on `:54321`, then `cargo build --release
    --no-default-features --features gpu-detect` (seccomp off, matching the
    release build).
  - `runtime` (`debian:bookworm-slim`) — nginx + the binary + `tini`; runs as
    non-root `ziee`.
- **`nginx.conf`** — SPA with history-API fallback (`try_files … /index.html`)
  for BrowserRouter; `location /api` proxies to `127.0.0.1:9000` with buffering
  **off** so SSE frames flush immediately, and forwards `X-Forwarded-*`
  (incl. `X-Forwarded-Host` for OAuth). It also re-emits `X-Accel-Buffering: no`
  via `add_header` so an **outer** reverse proxy (e.g. a Coder / ingress `nginx`
  published in front) also streams `/api` SSE un-buffered — nginx consumes the
  axum-set copy, so it must be re-emitted here to reach the edge.
  `check-sse-headers.mjs` (`node docker/web/check-sse-headers.mjs`) guards both
  directives against regression.
- **`entrypoint.sh`** — renders the config, then supervises `ziee` + `nginx`
  under `tini`; if either exits the container exits (Docker restarts it).
- **`config.template.yaml`** — external Postgres, loopback server, sandbox off,
  rate-limit off (single loopback source IP behind nginx),
  `trust_forwarded_headers: true`.

## Data volume

App state (DB is external; this holds `~/.ziee` extracted tool binaries, caches,
uploads) lives under `/var/lib/ziee`, owned by the non-root `ziee` user.

- A **named volume** (as in the compose files) inherits the right ownership and
  just works.
- A **host bind-mount** at `/var/lib/ziee` is created root-owned and the `ziee`
  user can't write it — `chown` it to the container's `ziee` uid/gid first, or
  use a named volume.

## Code sandbox (opt-in)

`code_sandbox` (bwrap-isolated code execution) is **off by default**. The runtime
image now ships the host deps (`bubblewrap`, `squashfuse`, `fuse3`, `fuse`), so
enabling it is opt-in via an env flag + a compose overlay — the general image and
the default stack are unchanged.

Enable with the overlay (keeps the base `name: ziee-web`, so the container stays
`ziee-web-ziee-web-1`):

```bash
sudo docker compose -f docker-compose.yml -f docker-compose.sandbox.yaml up -d --build
```

The overlay sets `ZIEE_CODE_SANDBOX_ENABLED=true` and grants the **minimal**
runtime privilege bwrap + squashfuse need: `/dev/fuse` + `cap_add: SYS_ADMIN` +
`security_opt: apparmor:unconfined, seccomp:unconfined`.

Two levels of "working":

- **Registration + the admin list** need only the env flag (the boot probe just
  looks for `bwrap` on `PATH`). Once enabled, the startup log shows
  `code_sandbox: registered (rootfs will mount on first execute_command)` and
  **Settings → Sandbox** lists the available rootfs versions from GitHub.
- **Real sandboxed execution** (the first `execute_command` fetches a rootfs from
  GitHub, mounts it via squashfuse, and runs bwrap with a PID namespace)
  additionally needs `/dev/fuse` and **unprivileged user namespaces**. On a host
  that restricts unprivileged userns (`kernel.unprivileged_userns_clone=0`, or an
  AppArmor userns restriction as on Ubuntu 23.10+), the bwrap PID-ns probe fails.
  In that case replace the `devices`/`cap_add`/`security_opt` block in
  `docker-compose.sandbox.yaml` with:

  ```yaml
  services:
    ziee-web:
      privileged: true
  ```

When the sandbox is **off** (the default), the admin rootfs-versions page still
renders the GitHub catalog with a clear notice explaining it's disabled — it no
longer shows a blanket error.

> **Egress:** enabling needs outbound access to `api.github.com` (rootfs catalog
> + fetch). **Security:** the sandbox gives LLM-generated code an
> isolated-but-privileged execution surface — keep it opt-in; never fold it into
> the base compose / general image.

## Notes / limits

- The `bio_mcp` / `web_search` / `lit_search` built-in tools are on by default;
  they only make outbound calls when a user actually invokes them. Disable via a
  bind-mounted config if egress must be locked down.
- The builder pulls a few optional tool binaries from GitHub releases (pandoc,
  typst, pdfium, uv, bun, biomcp) with **unauthenticated** requests. These are
  warn-and-continue: if those hosts are blocked the build still succeeds but the
  image ships without doc-conversion / stdio-MCP tooling. On a shared-IP CI
  runner the anonymous GitHub rate limit can occasionally throttle these.
- Heavy optional features (document conversion, local LLM engines, the sandbox)
  extract/exec extra bundled binaries at first use and may need more host libs;
  the base web app (auth, chat, settings) boots on the slim runtime as-is.
- The existing root `Dockerfile` (runtime-only, prebuilt musl binary for the CI
  release) is left untouched; this image is the self-contained alternative.
