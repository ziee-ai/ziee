# TeamCity build/deploy steps — ziee `deploy`

The 12 steps the TeamCity **ziee** config runs on a commit to `refs/heads/deploy`
(auto-deploy trigger). The build runs on the TeamCity **BUILD agent** (a separate
box, Docker-only — no host rust/zig toolchain); the resulting images ship to the
WSU deploy server. See also [`../../DEPLOY.md`](../../DEPLOY.md) for env params.

## The 12 steps

| # | Step | Command |
|---|------|---------|
| 1 | Build compile image | `docker build -f deploy/runtime/zigbuild.Dockerfile -t ziee-zigbuild:1.97 .` |
| 2 | Init submodules | `git submodule update --init --recursive --force` |
| 3 | Start SQLx build DB (port 54321 **required**) | throwaway `pgvector/pgvector:pg18` as `ziee-build-pg` |
| 4 | **Compile ziee** (Docker-only, static musl) | `cargo zigbuild --release --no-default-features --features gpu-detect --target x86_64-unknown-linux-musl` (volumes: `ziee-cargo-registry`, `ziee-cargo-git`, **`ziee-target`**) |
| 5 | Stage binary for image | copy `ziee` from `ziee-target` → `dist/ziee-amd64` |
| 6 | **Build API image** | `docker buildx build --ulimit nofile=524288:524288 -t ziee-web:sdkmig --load .` |
| 7 | **Build web (SPA) image** | `docker buildx build -f deploy/runtime/web.Dockerfile -t ziee-web-spa:sdkmig --load .` |
| 8 | Render config.yaml | `bash deploy/runtime/render-config.sh deploy/runtime/config.yaml` |
| 9 | Deploy stack | `docker compose up -d postgres ziee-web web` |
| 10 | Apply deploy seed | `docker compose run --rm ziee-seed` |
| 11 | Health check | `curl -fsS http://localhost:18130/api/health` |
| 12 | Cleanup build DB | `docker rm -f ziee-build-pg` |

## Two build caches — and the SEGFAULT-at-boot recovery

There are **two** caches, on two different steps:

| Cache | Step | Symptom if corrupt |
|---|---|---|
| **Rust compile cache** — the `ziee-target` (+ `ziee-cargo-registry`, `ziee-cargo-git`) docker volumes | **Step 4** (compile) | **bad binary → SIGSEGV-at-boot** → **Step 11 health check fails** / `ziee-web` won't come healthy |
| **Docker image-layer cache** — buildx | **Steps 6 & 7** | bad image layers |

### If the deploy fails at Step 11 with a segfault-at-boot → it's the COMPILE cache (Step 4)

There is **no `--no-cache` flag for `cargo`** — the "no-cache" for Step 4 is to
**delete the target volume** on the BUILD agent (NOT the WSU live server):

```bash
docker volume rm ziee-target                     # forces a clean recompile
# if still bad: docker volume rm ziee-cargo-registry ziee-cargo-git
# then re-run the build from Step 4 (or the whole pipeline)
```

This is the documented fix for the past "deployed ziee SIGSEGV-at-boot" incident —
it was a **corrupted build cache on the host**, not the code, not the GPU.

### If an *image* is bad (rare) → Steps 6/7

Add `--no-cache` to the `docker buildx build` command:

```bash
docker buildx build --no-cache --ulimit nofile=524288:524288 -t ziee-web:sdkmig --load .
```

### Nuclear reset (clears everything, then re-run the pipeline)

```bash
docker volume rm ziee-target ziee-cargo-registry ziee-cargo-git
docker builder prune -af
```

> ⚠ Use `--no-cache` / volume-wipes **only for the one recovery run, then revert** —
> a clean musl compile is slow, and Steps 4/6/7 normally rely on the cache.

## Full step commands (verbatim)

```bash
# 1) Build compile image
docker build -f deploy/runtime/zigbuild.Dockerfile -t ziee-zigbuild:1.97 .

# 2) Init submodules
git submodule update --init --recursive --force

# 3) Start SQLx build database (port 54321 required)
docker rm -f ziee-build-pg 2>/dev/null || true
docker run -d --name ziee-build-pg -p 54321:5432 -e POSTGRES_PASSWORD=buildpw pgvector/pgvector:pg18
until docker exec ziee-build-pg pg_isready -U postgres >/dev/null 2>&1; do sleep 1; done

# 4) Compile ziee (Docker-only, static musl)
docker run --rm --network host \
  -e DATABASE_URL=postgresql://postgres:buildpw@127.0.0.1:54321/postgres \
  -e CARGO_TARGET_DIR=/target \
  -v "$PWD":/io \
  -v ziee-cargo-registry:/usr/local/cargo/registry \
  -v ziee-cargo-git:/usr/local/cargo/git \
  -v ziee-target:/target \
  -w /io/src-app/server \
  ziee-zigbuild:1.97 \
  cargo zigbuild --release --no-default-features --features gpu-detect \
    --target x86_64-unknown-linux-musl

# 5) Stage binary for image
mkdir -p dist
docker run --rm -v ziee-target:/target -v "$PWD/dist":/out alpine \
  cp /target/x86_64-unknown-linux-musl/release/ziee /out/ziee-amd64

# 6) Build API image
docker buildx build --ulimit nofile=524288:524288 -t ziee-web:sdkmig --load .

# 7) Build web (SPA) image
docker buildx build -f deploy/runtime/web.Dockerfile -t ziee-web-spa:sdkmig --load .

# 8) Render config.yaml
bash deploy/runtime/render-config.sh deploy/runtime/config.yaml
export ZIEE_CONFIG_FILE=deploy/runtime/config.yaml

# 9) Deploy stack
docker compose up -d postgres ziee-web web

# 10) Apply deploy seed
docker compose run --rm ziee-seed

# 11) Health check
curl -fsS http://localhost:18130/api/health

# 12) Cleanup build database
docker rm -f ziee-build-pg 2>/dev/null || true
```
