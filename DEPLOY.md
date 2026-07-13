# Deployment reference — TeamCity env params

Canonical list of the TeamCity **Environment Variables** (`env.*`) for every build
config. 🔐 = Password-type param, value lives ONLY in TeamCity (never committed).
Non-secret values are shown. Deploy host: `devops@ressrv02ex6091.ad.wayne.edu -p 22222`;
persistent data under `/data`; host allows ports **18000–19000**; cross-container
addressing via `host.docker.internal`.

> **Deploy-time seeding (replaces desired_state).** The old config-as-code boot
> reconciler was removed (PR #145). A fresh deploy is now configured by plain SQL
> — `deploy/seed/seed.sql` — applied by the one-shot `ziee-seed` service AFTER the
> app is healthy (the app runs its migrations before it reports healthy, so the
> tables exist). The seed is **idempotent**: it registers the 3 org MCP servers,
> fills + enables the `google` provider, reduces the default Users-group
> permissions, and creates the first admin — re-running it each deploy is safe.
> These files live ONLY on the `deploy` branch.

## Branch / release flow (ziee)
`worker → khoi → main → deploy`. TeamCity **ziee** config watches `refs/heads/deploy`,
**manual trigger**. `deploy` now carries deploy-only files that `main` does NOT
(`docker-compose.deploy.yml`, `deploy/seed/`, `DEPLOY.md`), so a release is a
**merge**, NOT a fast-forward:

```bash
git checkout deploy
git merge main            # NON-fast-forward: brings main's changes onto deploy,
                          # keeping the deploy-only files. Resolve any conflict
                          # toward the deploy-only versions.
git push origin deploy    # triggers nothing on its own — TeamCity is manual
```

then Run the TeamCity **ziee** config.

## Port map (host side)
| Service | host port | container |
|---|---|---|
| ziee | 18130 | 8080 |
| biognosia-mcp | 18100 | 8081 |
| Milvus / Neo4j / Redis / Mongo / ES (DB stack) | 18110 / 18111 / 18112 / 18113 / 18114 | 19530 / 7687 / 6379 / 27017 / 9200 |
| RCPA MCP / static | 18120 / 18121 | 9004 / 9005 |
| DSCC MCP / static | 18122 / 18123 | 9006 / 9007 |

---

## ziee  (repo ziee-ai/ziee · branch `deploy` · manual)
Build steps:
1. `docker compose build`
2. `mkdir -p "${ZIEE_DATA_ROOT:-/data/ziee}"/{pg-data,ziee-data}; docker compose up -d`
3. **Apply the seed:** `docker compose run --rm ziee-seed`
   — a plain `up -d` does NOT re-run an already-exited `restart:no` one-shot, so the
   seed must be forced every deploy. `run --rm` blocks and surfaces its exit code
   (fail the build on non-zero). Equivalent: `docker compose up -d --force-recreate ziee-seed`.
4. verify-running (`curl -fsS http://localhost:18130/api/health`).

| Var | Value |
|---|---|
| `ZIEE_JWT_SECRET` 🔐 | (in TeamCity) |
| `ZIEE_STORAGE_KEY` 🔐 | (in TeamCity — **permanent**, never change; the seed encrypts the Google secret with it) |
| `ZIEE_DB_PASSWORD` 🔐 | (in TeamCity — set before first boot) |
| `ZIEE_ADMIN_USERNAME` | `admin` — first admin, **required by the seed** |
| `ZIEE_ADMIN_EMAIL` | `admin@tinnguyen-lab.com` — **required by the seed** |
| `ZIEE_ADMIN_PASSWORD` 🔐 | (in TeamCity) — **required by the seed**; create-only (a later UI change is never reverted) |
| `GOOGLE_CLIENT_ID` | Google OAuth **production** client ID — **required by the seed** |
| `GOOGLE_CLIENT_SECRET` 🔐 | Google OAuth production client secret (in TeamCity) — **required by the seed** |
| `ZIEE_WEB_PORT` | `18130` |
| `ZIEE_PUBLIC_FILE_ORIGIN` | `https://biognosia.tinnguyen-lab.com` — file-link origin handed to ALL MCP servers, incl. **user-added/remote** ones, so it MUST be public (single value). Co-located servers fetch via the edge (verify the deploy host can reach the public URL). |
| `ZIEE_CORS_ALLOW_ORIGIN` | `https://biognosia.tinnguyen-lab.com` (the public origin) |
| `ZIEE_MAX_FILE_UPLOAD_MB` | `128` |
| `COMPOSE_FILE` | `docker-compose.yml:docker-compose.sandbox.yaml:docker-compose.deploy.yml` |
| `COMPOSE_PROJECT_NAME` | `ziee-web` |
| *(optional — seed defaults to these deploy-host endpoints)* `RCPA_MCP_URL` / `DSCC_MCP_URL` / `BIOGNOSIA_MCP_URL` | `http://host.docker.internal:18120/mcp` / `:18122/mcp` / `:18100/mcp` |
| *(optional)* `ZIEE_DATA_ROOT` | `/data/ziee` |

**Google sign-in:** the `ziee-seed` one-shot stamps `GOOGLE_CLIENT_ID` +
`GOOGLE_CLIENT_SECRET` onto the pre-seeded `google` provider (encrypting the secret
with `ZIEE_STORAGE_KEY`, exactly as the app does) and enables it — no admin-UI step.
Register **`https://biognosia.tinnguyen-lab.com/api/auth/oauth/google/callback`** as an
Authorized redirect URI in the Google Cloud OAuth (production) client. The redirect
URI is derived from `X-Forwarded-Proto`/`Host`, so the ingress edge MUST forward
`X-Forwarded-Proto: https` and the real public Host or Google rejects the callback
(`redirect_uri_mismatch`). Both creds are **required** — the seed aborts loud if either
is unset (it never seeds a blank/plaintext secret).

## Databases  (repo ziee-ai/biognosia-mcp · `deploy/db/` · path-gated `+:deploy/db/**` or manual)
Build step: `cd deploy/db && docker compose up -d`.

| Var | Value |
|---|---|
| `LIGHTRAG_DATA_DIR` | `/data/ziee/biognosia-dbs` |
| `NEO4J_PASSWORD` 🔐 | `biognosia2024` (must match copied data) |
| `MONGO_PASSWORD` 🔐 | `biognosia2024` (must match copied data) |
| RAM (tune to host) | `MILVUS_GOMEMLIMIT=256GiB`, `NEO4J_HEAP=32G`, `NEO4J_PAGECACHE=32G`, `REDIS_MAXMEM=48gb`, `REDIS_CACHE_MAXMEM=64gb`, `MONGO_CACHE_GB=32`, `ES_JAVA_OPTS=-Xms8g -Xmx8g` |
| *(optional — compose defaults)* host ports | `MILVUS_HOST_PORT=18110` … `ES_HOST_PORT=18114` |

## biognosia  (repo ziee-ai/biognosia-mcp · branch `main` · on push)
Build step 1 writes `.env` from these; step 2 `docker compose up -d --build` (verify running). GPU via CDI (`nvidia.com/gpu=all`; run `nvidia-ctk cdi generate` on host once).

| Var | Value |
|---|---|
| `BIOGNOSIA_HOST_PORT` | `18100` |
| `BIOGNOSIA_MONGODB_URI` 🔐 | `mongodb://admin:biognosia2024@host.docker.internal:18113/?authSource=admin` |
| `BIOGNOSIA_NEO4J_PASSWORD` 🔐 | `biognosia2024` |
| `BIOGNOSIA_EMBEDDING_DEVICE` | `cuda:0` |
| `BIOGNOSIA_RERANK_DEVICE` | `cuda:0` |
| `BIOGNOSIA_RERANK_STAGE1_DEVICE` | `cuda:1` |
| `MCP_LOG_LEVEL` | `INFO` |
| `MCP_MAX_CONCURRENT_TOOL_CALLS` | `8` |
| *(fixed in the .env heredoc)* DB ports | Milvus `18110`, Redis `18112`, Neo4j `bolt://…:18111`, ES `…:18114` |

## RCPA  (repo ziee-ai/rcpa-mcpserver · branch `main` · on push)
Build step: `docker compose up -d --build --wait`.

| Var | Value |
|---|---|
| `RCPA_HOST_PORT` | `18120` |
| `RCPA_STATIC_HOST_PORT` | `18121` |
| `BASE_URL` | `http://host.docker.internal:18121` |

## DSCC  (repo ziee-ai/dscc-mcpserver · branch `main` · on push)
Build step: `docker compose up -d --build --wait`.

| Var | Value |
|---|---|
| `DSCC_HOST_PORT` | `18122` |
| `DSCC_STATIC_HOST_PORT` | `18123` |
| `BASE_URL` | `http://host.docker.internal:18123` |

---

**Deploy order**: Databases → biognosia → RCPA → DSCC → ziee (so the org MCP servers
the seed registers are reachable when ziee's boot health check probes them; an
unreachable server is auto-disabled, and the next deploy's seed re-enables it).
Verify ziee: `curl -fsS http://localhost:18130/api/health`.
