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

## Config as code (desired state) — zero-touch first boot

The image ships **`config/desired-state.yaml`** at `/etc/ziee/desired-state.yaml`.
When enabled, the server reconciles that file into the database on boot — **after**
migrations, **before** it serves — so a fresh deploy comes up **fully configured with
no manual UI setup**: the org MCP servers registered, the root admin created, and
the default group's permissions trimmed.

> ### Where it is turned on
> The deploy overlay **`docker-compose.deploy.yml`** carries the switch, the MCP
> endpoint URLs and the admin password (and the `host.docker.internal`
> mapping the org MCP servers need). The base `docker-compose.yml` is a local test
> stack and deliberately does NOT enable any of it.
>
> ### ⚠ It is OFF unless the deploy turns it on
> **`ZIEE_APPLY_DESIRED_STATE=1` is the switch, and it defaults to OFF.** Shipping
> the file — in the repo or baked into the image — applies **nothing**. Without the
> flag the server logs `desired-state reconcile: disabled` and writes nothing: no
> seeding, no enforce, no MCP/admin/permission writes. That is what keeps a local
> developer's hand-configured models, MCP servers, admin and permissions from ever
> being touched or duplicated. TeamCity sets the flag **only** on the deploy configs.

| Env var | Fills | If unset |
|---|---|---|
| `RCPA_MCP_URL` | the `rcpa` system MCP server's URL | that server is skipped |
| `DSCC_MCP_URL` | the `dscc` system MCP server's URL | that server is skipped |
| `BIOGNOSIA_MCP_URL` | the `biognosia` system MCP server's URL | that server is skipped |
| `ZIEE_ADMIN_USERNAME` / `ZIEE_ADMIN_EMAIL` / `ZIEE_ADMIN_PASSWORD` | **the first administrator — env only.** Applied ONLY to a database with **no account at all** (the very first deploy). Afterwards it is a no-op, so a password the admin changes in the UI is **never reverted** by a redeploy. | no admin is created (the UI shows first-run setup) |
| `ZIEE_APPLY_DESIRED_STATE` | **the switch** — `1` applies the file | **nothing is applied at all** (the local-dev default) |
| `ZIEE_DESIRED_STATE_FILE` | path of the file itself | **the image always sets this** to `/etc/ziee/desired-state.yaml` — point it at a nonexistent path to turn config-as-code OFF |

On the **deploy host** the three MCP servers are published on the HOST (that host
only allows ports `18000-19000`) and reached over `host.docker.internal`:

```bash
docker run --rm -p 8080:8080 \
  --add-host host.docker.internal:host-gateway \
  -e ZIEE_APPLY_DESIRED_STATE=1 \
  -e ZIEE_DB_HOST=db.internal -e ZIEE_DB_USER=ziee -e ZIEE_DB_PASSWORD=secret -e ZIEE_DB_NAME=ziee \
  -e ZIEE_JWT_SECRET='…' -e ZIEE_STORAGE_KEY='…' \
  -e BIOGNOSIA_MCP_URL=http://host.docker.internal:18100/mcp \
  -e RCPA_MCP_URL=http://host.docker.internal:18120/mcp \
  -e DSCC_MCP_URL=http://host.docker.internal:18122/mcp \
  -e ZIEE_ADMIN_USERNAME='admin' -e ZIEE_ADMIN_EMAIL='admin@example.com' -e ZIEE_ADMIN_PASSWORD='…' \
  ziee-web:local
```

> **Required compose change (deploy dependency).** Reaching those host-published
> ports needs the container to resolve `host.docker.internal`, so ziee's compose
> **must** carry
> ```yaml
> extra_hosts:
>   - "host.docker.internal:host-gateway"
> ```
> — the same mapping `biognosia-mcp` and `cpa-website` already use. **`docker-compose.deploy.yml`
> already sets it**; a hand-rolled `docker run` needs `--add-host` as above. Local dev points the same
> env vars somewhere else entirely (e.g. `http://172.21.0.1:9004/mcp`) — which is
> exactly why they are env-templated rather than baked into the manifest.

**Secrets are never in the file.** A password field must be exactly one
`${ENV_VAR}` placeholder; an inline literal is rejected. Resolved values are never
logged (the logs name the env var, never its value). The **administrator is not in
the file at all** — not even its username or email: it comes purely from
`ZIEE_ADMIN_USERNAME` / `ZIEE_ADMIN_EMAIL` / `ZIEE_ADMIN_PASSWORD`, and is created
only on a database with no accounts.

**Overriding the file:** bind-mount your own at the same path —
`-v ./my-desired-state.yaml:/etc/ziee/desired-state.yaml:ro` — or point
`ZIEE_DESIRED_STATE_FILE` somewhere else.

**Idempotent by design.** Re-deploying (or restarting) reconciles again and
creates nothing twice: MCP servers dedup on `(name, is_system)`, the admin is
created only when the deployment has **no** admin (its password is **never**
reset on a later boot — rotate it in the UI and the rotation sticks), and users
dedup on username/email. Concurrently-booting containers (a rolling redeploy)
serialize on a Postgres advisory lock, so they cannot race into duplicate rows.

**A bad entry is skipped; a bad FILE fails the boot.** An unresolvable `${VAR}`,
an inline secret, or a DB error on one entry logs an error and skips just that
entry. But an unreadable or invalid desired-state file **aborts startup** — a
publicly-served container that is silently unconfigured (no admin ⇒ the
unauthenticated first-run setup page is open to whoever finds it) is worse than
one that refuses to start. Note that bind-mounting a host path that does not
exist makes Docker create a *directory* there; that is caught and reported.

**The file only adds and updates — it never deletes.** A server's identity is its
`name`, so renaming (or removing) an entry leaves the OLD row in the database: the
reconciler creates the new name and cannot know the old one is obsolete. After a
rename, delete the stale server once in **Settings → MCP Servers**. (Accounts and
group permissions are unaffected — those are keyed by username/group name.)

**Per-entry `mode`:**

- `ensure` (default) — create when absent; if it already exists, leave its
  fields alone. An admin's later UI edits survive a redeploy.
- `enforce` — create when absent, else re-sync the fields the file **declares**
  on every boot (a field the file omits keeps its current DB value).

The three org MCP servers ship as `enforce` deliberately: ziee's boot health
check probes every enabled MCP server and **auto-disables the unreachable ones**,
so an endpoint that is down when ziee starts gets flipped to `enabled: false` —
`enforce` re-asserts `enabled: true` on the next deploy, where `ensure` would
leave it off forever. The trade-off is that an admin who deliberately disables
one in the UI will see it re-enabled by the next deploy: change it in the file,
not the UI. A server's `groups:` list is re-applied in **both** modes (assignment
is additive; removing a group from the file does not revoke it), because a server
in no group is unusable by non-admin users. Their `usage_mode` is `auto` — the
model decides when to call them.

Group permissions have no mode: they are declarative and re-applied every boot.
Nothing is removed from the product — the file just sets permissions false for the
default **Users** group, and a permission gates the nav entry, the settings tab and
the route together. The shipped file hides, for regular users:

| Surface | Permission dropped |
|---|---|
| Projects (nav) | `projects::*` |
| Hubs (nav) | `hub::*` |
| Knowledge (nav) | `knowledge_base::*` |
| Scheduled Tasks (nav) | `scheduler::*` |
| Settings → Assistants | `assistants::*` |
| Settings → Web Search Keys | `web_search::*` |
| Settings → Literature Keys | `lit_search::*` |
| Settings → Workflows | `workflows::*` |
| Settings → Memory | `memory::*` |
| Settings → Citations | `citations::*` |

General, Profile, LLM providers, MCP servers, chat/files and `notifications::read`
all stay. Note the `*::use` permissions also gate the matching built-in chat tools,
so a user in this group does not get web-search / literature / citations /
knowledge-base tools in chat either — intended, since they cannot configure them. (`projects::*` is a no-op today —
the default group was never granted it — and is declared so a future migration
cannot silently un-hide Projects.) An `add:` list may **not** contain a wildcard:
a manifest can never grant `*`. Note the root admin **bypasses all permission
checks** by design, so it still sees everything — a non-admin account (created by
the admin in the UI) is what shows the trimmed UI.

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
  `check-sse-headers.mjs` guards both directives against regression: the
  Dockerfile's `config-check` stage runs it on every image build (the runtime
  COPYs `nginx.conf` from that stage, so the build fails if a directive is
  dropped), and it can be run standalone with `node docker/web/check-sse-headers.mjs`.
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
