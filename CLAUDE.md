# Ziee - Developer Documentation Hub

Essential documentation for developing Ziee, a full-stack application with Rust backend and React meta-framework frontend.

---

## Quick Start

```bash
# One-time setup (or after any dep bump):
npm install                          # hoists deps for BOTH UI workspaces
cd src-app && cargo check --workspace  # builds the entire Rust workspace

# Backend
cd src-app/server
CONFIG_FILE=config/dev.yaml cargo run

# Frontend
cd src-app/ui
npm run dev
```

Access at http://localhost:5173

### Monorepo layout

- **Rust** — single workspace at `src-app/Cargo.toml` listing 9 member
  crates. Shared dep versions live in `[workspace.dependencies]`; bump
  once there, every member picks it up. One `Cargo.lock` at
  `src-app/Cargo.lock`. Cargo's config (`POSTGRESQL_VERSION` etc.) is
  workspace-wide at `src-app/.cargo/config.toml`.
- **npm** — root `/package.json` declares `workspaces:
  ["src-app/ui", "src-app/desktop/ui"]`. `npm install` from the repo
  root hoists shared deps into `/node_modules`. One
  `/package-lock.json`. `overrides` pins react/react-dom/typescript
  across workspaces.
- **Drift guard** — `npx syncpack lint` (or `just sync-check`) flags
  any shared dep whose version differs between
  `src-app/ui/package.json` and `src-app/desktop/ui/package.json`.
  Rules live in `/.syncpackrc.json`.

---

## Naming convention

The application is named **ziee** (not `ziee-chat`). This applies to:

- Cargo package + binary names (`ziee`; the built binary is `target/<triple>/release/ziee`)
- Cargo lib name (`ziee`; imports are `use ziee::...`)
- Default paths (`~/.ziee/`, `/tmp/ziee-*`, etc.)
- Env var prefixes (`ZIEE_*`)
- JWT claim values (`iss: "ziee"`, `aud: "ziee-api"`, `aud: "ziee-download"`)
- Docker compose project (`name: ziee`)
- Log messages, error messages, doc comments
- Anything user- or operator-facing

The string `ziee-chat` survives only in **external references that are opaque
to us**:

- The GitHub repo URL (`github.com/phibya/ziee-chat`) — used by the sandbox
  rootfs fetcher (`src-app/server/src/modules/code_sandbox/runtime_fetch.rs`),
  the cosign cert-identity regex
  (`scripts/bootstrap-first-rootfs-release.sh`), and `sandbox-rootfs/README.md`.
- Historical log/diagnostic artifacts under `src-app/server/test-logs/` and
  `src-app/ui/docs/antd-diagnostics/`. They roll over naturally.

Anywhere else, treat `ziee-chat` as a bug to fix.

---

## Development Environment

### Docker Compose

**Location:** `/home/pbya/projects/ziee/src-app/docker-compose.yaml`

**IMPORTANT:** When working with database schema changes:
1. The docker-compose file is in `src-app/` directory, NOT the project root
2. To reset the database after migration changes:
   ```bash
   cd /home/pbya/projects/ziee/src-app
   docker compose down
   docker compose up -d
   ```
3. The PostgreSQL build container is named `ziee-postgres-build-1`
4. Port mappings:
   - `54321` → Build database (SQLx compile-time verification)
   - `54322` → Test database (integration tests)

### Build Database (build.rs)

The `build.rs` script automatically manages the build database for SQLx compile-time verification:

1. **Automatic database setup:** On each build, `build.rs`:
   - Connects to `postgresql://postgres:password@127.0.0.1:54321/postgres`
   - Wipes the database (drops and recreates `public` schema)
   - Runs all migration files from `migrations/`
   - Sets `DATABASE_URL` for SQLx compile-time query verification

2. **When to run `cargo clean`:**
   - After modifying migration files (to force build.rs to re-run migrations)
   - If you see "relation does not exist" SQLx errors
   - If the build database schema is out of sync

3. **Example workflow for migration changes:**
   ```bash
   # Edit migration file: migrations/00000000000006_create_assistants_table.sql
   cargo clean    # Force build.rs to re-run
   cargo build    # build.rs will wipe database and run all migrations
   ```

4. **Important notes:**
   - DO NOT manually set `DATABASE_URL` when building (use defaults from build.rs)
   - The build database is ephemeral - it gets wiped on every clean build
   - Changes to migration files require `cargo clean` to take effect

---

## Memory System (LLM Memory)

The `memory` module ships per-user persistent memory with vector
retrieval (mem0 / Letta-style). Postgres-backed via **pgvector**,
which is bundled into the embedded PG binary at build time.

### Host build deps (Linux)

```bash
sudo apt install build-essential   # provides make + gcc
```

`build_helper/pgvector.rs` invokes `make` against the vendored
pgvector source under `src-app/server/vendor/pgvector` and downloads
matching Postgres 17.5.0 binaries from `theseus-rs/postgresql-binaries`
to link against. Fail-soft: if the build fails (no make, no network,
unsupported target triple), zero-byte stub assets are written so the
crate still compiles; at runtime the server logs "memory features
disabled" and `memory_admin_settings.enabled` flips off.

macOS uses the Apple-Silicon SDK-path wrapper ported verbatim from
the legacy ziee-chat-ref project (handles `/opt/homebrew` paths).
Windows needs `nmake` + MSVC.

### docker-compose

The `pgvector/pgvector:pg17` image is required for the build DB
(`docker-compose.yaml` already references it on this branch). The
test DB inherits the same image. Without it, `CREATE EXTENSION vector`
in migration 46 will fail at build.rs time.

### Pgvector submodule

The vendored pgvector source is a git submodule at
`src-app/server/vendor/pgvector` pinned at `v0.7.4`. Run
`git submodule update --init` after the first clone of this branch.

### Memory tables

- `user_memories` — per-user fact rows (vector(N) default 768)
- `user_memory_settings` — per-user opt-in toggles (default OFF)
- `memory_admin_settings` — deployment-wide config (default OFF)
- `conversation_memory_settings.memory_mode` — per-conversation
  override (`inherit`/`on`/`off`); replaced the inline
  `conversations.memory_mode` column (dropped in migration 76)
- `assistant_core_memory` — Letta-style always-in-context blocks
- `conversation_summaries` — rolling per-branch summary

### Surfaces

- REST CRUD at `/api/memories`, `/api/memory/settings`,
  `/api/memory/admin-settings`.
- Built-in MCP server at `/api/memories/mcp` exposing
  `remember` / `recall` / `forget` tools (deterministic UUID via
  `Uuid::new_v5(NAMESPACE_URL, "memory.ziee.internal")`).
- Chat extension `chat::extensions::memory` injects retrieved
  memories before each LLM call (`before_llm_call` hook) and spawns
  background extraction after each assistant reply (`after_llm_call`).
- Onboarding step `memory-setup` in the `getting-started` guide asks
  the admin to enable memory + pick an embedding model.
- User-facing pages: `/memories`, `/settings/memory`,
  `/settings/memory-admin`.

---

## Hub Seed (build-time fetch)

The embedded hub catalog at `<binary>/binaries/hub-seed/` is fetched
fresh from the `ziee-ai/hub` GitHub release on every `cargo build`.
`build_helper/hub_seed.rs` resolves the latest non-prerelease tag,
downloads the 6 release artifacts (tarball + index + their sha256
sidecars + cosign keyless bundles), verifies them with the same
chain the runtime refresh path uses (`hub_manager.rs` verify
functions), and stages the result into `binaries/hub-seed/` for
`include_dir!` to bake into the binary. `SEED_HUB_VERSION` is set
by the build helper via `include_str!(concat!(env!("OUT_DIR"),
"/hub_seed_version.txt"))` — keeping the version + index in lockstep
is the build's responsibility, not the maintainer's.

### Failure contract — different from every other build helper

`pandoc`, `typst`, `pdfium`, `uv`, `bun`, `sandbox_runtime` all
**warn-and-continue** on setup failure. `hub_seed` **panics**: the
seed is the source of truth at runtime for air-gapped / first-boot
users, and shipping a binary with an empty or stale seed silently
degrades the hub UI. See the divider comment in
`build.rs::setup_external_binaries()` for the rationale.

### Env vars (build time)

- `HUB_RELEASE_TAG=v0.x.y` — pin to a specific release tag instead
  of resolving the latest. Required for reproducible / air-gapped
  builds: an operator without GitHub access must set this AND
  pre-stage `binaries/hub-seed/` (the skip-if-fresh path consults
  that cache before any network call).
- `GITHUB_TOKEN=ghp_...` — honored if set. Lifts the unauthenticated
  60-req/hr-per-IP GitHub API limit to 5000/hr; matters on CI matrix
  builds that share an egress IP. Falls back to unauthenticated
  requests if unset.

### Cache (`binaries/hub-seed/`)

Persists across `cargo clean` (it's manifest-relative, not in
`target/`). The `.tag` sidecar holds the cached release tag; on each
build the helper queries GitHub for the latest tag and skips the
download when `.tag` matches.

**Tamper detection limit**: `cargo:rerun-if-changed=<seed_dir>`
catches add / remove / rename inside the seed dir (directory
mtime changes), but it does NOT catch in-place file edits
(cargo only watches the dir's own mtime, not its contents'
mtimes). The runtime test `seed_index_version_matches_const`
is the in-place-edit backstop: it compares the on-disk
`index.json`'s `hub_version` field against `SEED_HUB_VERSION`
at test time, so a manual edit that bumps content without
bumping the version fails the test suite.

Concurrent `cargo build` invocations serialize on an advisory
`flock(2)` over `binaries/.hub-seed.lock` (kernel auto-releases
on process exit, so SIGKILL is safe).

### Forcing a fresh fetch

```bash
rm -rf src-app/server/binaries/hub-seed
cargo check -p ziee
```

---

## Code Sandbox

The `code_sandbox` module exposes a bwrap-isolated code-execution
environment as a **built-in MCP server**. Disabled by default; flip
`code_sandbox.enabled: true` in your config after the host setup below.

### Threat model

Protects against: prompt-injection-induced exfiltration, accidental
destructive commands (`rm -rf /`, fork bombs, log-bombing), host
filesystem pollution outside the workspace.

Does **NOT** protect against: Linux kernel 0-days. Not suitable for
hostile multi-tenant execution without an outer microVM (gVisor or
Firecracker).

**Network egress is intentionally open** (bwrap runs with `--share-net`, so
the sandbox shares the host network and can reach the internet *and* host
localhost). The "exfiltration" protection is therefore NOT an egress block —
it's that there is nothing sensitive to exfiltrate: `--clearenv` wipes the
server's entire environment (no `DATABASE_URL`/JWT/`*_API_KEY` reach the
sandbox), and each conversation only sees its own workspace. If a deployment
needs egress *blocking*, the future options are bwrap `--unshare-net`
(no network at all), Landlock-NET (ABI v4, per-port TCP allowlist), or an
egress-filtering proxy — none enabled today.

### Cross-platform

The sandbox runs on all three host OSes via the `SandboxBackend` seam
in `src-app/server/src/modules/code_sandbox/backend/`:

- **Linux** — `linux_bwrap` runs bwrap directly on the host. The
  reference path; every hardening primitive is native here.
- **macOS** — `mac_vm` boots a libkrun microVM (Apple Silicon only)
  bundling a Linux kernel + the `sandbox-guest-agent`; bwrap runs
  inside that. Host requires `libkrun.dylib` bundled at
  `Contents/Frameworks/`.
- **Windows** — `wsl2` imports a per-flavor WSL2 distro
  (`ziee-sandbox-<flavor>-v<schema>`), provisions it (narrow AppArmor
  profile, sysctls re-applied on every VM boot, rsync + bwrap
  installed), and reaches the in-distro agent over **AF_VSOCK** (NOT
  loopback TCP — that was reachable across distros; see HIGH-1 in
  `.sec-audits/wsl2-sandbox-prior-art-2026-05-22.md`). Host requires
  WSL ≥ 2.5.10 / 2.6.1 (CVE-2025-53788 gate enforced by `probe_host`).

`build_bwrap_argv` is shared across all three backends — same argv,
same `--clearenv`/`--unshare-user`/seccomp/cgroup. They differ only in
**where** bwrap runs and how the workspace is plumbed in.

### Admin UI

One settings page at **`/settings/sandbox`** ("Code Sandbox" in the
admin sidebar) with two card sections:

- **Rootfs environments** — list cached flavors, pre-fetch with live
  SSE progress, evict.
- **Resource limits** — the singleton `code_sandbox_settings` row:
  memory / pids / cpu / prlimit caps + wall-clock timeout + VM
  idle-evict. Changes invalidate the in-process cache so the next
  `execute_command` reads the new caps.

Permissions: `code_sandbox::environments::{read,manage}` +
`code_sandbox::resource_limits::{read,manage}`. Administrators have
all four via the `*` wildcard.

### Host package install per distro

**Runtime deps** (what operators consuming a binary release need):

```bash
# Debian / Ubuntu
sudo apt install bubblewrap squashfuse fuse3

# Fedora / RHEL / CentOS
sudo dnf install bubblewrap squashfuse fuse3

# Arch
sudo pacman -S bubblewrap squashfuse fuse3

# openSUSE
sudo zypper install bubblewrap squashfuse fuse3

# Alpine
sudo apk add bubblewrap squashfuse fuse3
```

That's it for runtime. **`cosign` is no longer required** — verification is done in-process via the `sigstore` Rust crate. **`libseccomp2` is no longer required at runtime** — it's statically linked into the binary at build time (via `.cargo/config.toml`'s `LIBSECCOMP_LINK_TYPE=static`).

**Additional build-from-source deps** (only if compiling the server yourself with `--features code_sandbox_seccomp`):

```bash
# Debian / Ubuntu
sudo apt install libseccomp-dev pkg-config

# Fedora
sudo dnf install libseccomp-static libseccomp-devel pkgconf-pkg-config

# Arch
sudo pacman -S libseccomp pkgconf
```

**Additional build-the-rootfs-locally deps** (only if running `just sandbox-build` instead of letting the server auto-fetch a published rootfs):

```bash
# Debian / Ubuntu
sudo apt install mmdebstrap squashfs-tools
```

### Rootfs setup

The sandbox needs an Ubuntu-based **squashfs** rootfs (~1.6-2.0 GB
compressed) mounted via `squashfuse` and bind-mounted read-only into
each bwrap call.

```bash
# Build locally (10-15 min one-time)
just sandbox-build full        # or `minimal` for ~150 MB fast iteration

# Mount + flip the `current` symlink
just sandbox-mount

# Tear down (unmount + rm)
just sandbox-clean
```

The boot probe at server start reads
`<rootfs>/.ziee-sandbox-rootfs-schema` and refuses to enable on
mismatch with the binary's embedded `SANDBOX_ROOTFS_SCHEMA_VERSION`.
See `src-app/sandbox-rootfs/README.md` for the bootstrap (first
release) procedure.

### Startup hardening line

Expected startup hardening line (look in server logs):
- Built with `--features code_sandbox_seccomp`:
  `pid_ns: on, cgroup_v2: on (delegated), seccomp: on`
- **Stock build** (no feature flag): `pid_ns: on,
  cgroup_v2: on (delegated), seccomp: off-feature-not-linked`. The
  rest of the hardening (rlimits via prlimit, PID-ns, cgroup, --clearenv,
  --die-with-parent, output cap, wall-clock timeout) is unaffected.
- Rootfs schema mismatch: `code_sandbox: rootfs schema version
  mismatch; sandbox will NOT be registered` → install a compatible
  rootfs; the server auto-fetches the matching schema on the next
  `execute_command`.

**Enabling seccomp:**
```bash
# Install libseccomp dev + static archive per your distro (see the
# "Additional build-from-source deps" section above).
cargo build --release --features code_sandbox_seccomp
```
The `code_sandbox_seccomp` cargo feature is opt-in because libseccomp
must be present at link time on the build host. The resulting binary
**static-links** libseccomp (via `.cargo/config.toml`'s
`LIBSECCOMP_LINK_TYPE=static`), so operators don't need
`libseccomp2` installed at runtime. Without the feature, the sandbox
runs with all other hardening in place and seccomp is logged as
`off-feature-not-linked`.

### Tests

The sandbox test suite is organized into 6 tiers:

| Tier | Count | Needs | Speed | Run via |
|---|---|---|---|---|
| 1 — in-source unit | ~75 | nothing | <100 ms | `just check-sandbox-unit` |
| 2 — DB integration | ~17 | Postgres | ~30 s | `just check-sandbox-unit` |
| 3 — HTTP handler | ~11 | TestServer | ~15 s | `just check-sandbox-unit` |
| 4 — bwrap-direct | ~14 | rootfs mounted | ~20 s | `just check-sandbox` |
| 5 — real-LLM chat | 3 | ANTHROPIC_API_KEY + rootfs | ~2 min | `just check-sandbox-llm` |
| 6 — HTTP-E2E | ~22 | rootfs mounted | ~45 s | `just check-sandbox` |

**CI runs zero tests** — `.github/workflows/code_sandbox.yml` is
build-and-publish-only (triggered on `sandbox-rootfs-v*` tags, signs
artifacts with keyless cosign, publishes to GitHub Releases, auto-PRs
an update to `known_revisions.toml`). Cosign keyless signing is the
one thing that genuinely requires GitHub Actions (the OIDC issuer is
only valid for real Actions runs); everything else is faster locally.

Maintainer's responsibility before pushing:

```bash
just check                  # schema sync + Tier 1/2/3 (~30 s)
just check-sandbox          # adds Tier 4 + 6 (needs rootfs mounted)
just check-release-ready    # adds reproducibility check (~15 min, pre-tag)
```

Or run cargo directly:

```bash
# Tier 1 (unit, no external deps):
cd src-app/server && cargo test --lib code_sandbox::

# Tier 2 + 3 (DB + HTTP; sandbox disabled):
cargo test --test integration_tests -- --test-threads=1 code_sandbox::

# Tier 4 (bwrap-direct, needs rootfs mounted):
ZIEE_SANDBOX_ROOTFS=.ziee-cache/sandbox-rootfs/current \
    cargo test --test integration_tests -- --test-threads=1 \
    --ignored code_sandbox::tier4_

# Tier 6 (HTTP-E2E, the only tier exercising the FULL production path):
ZIEE_SANDBOX_ROOTFS=.ziee-cache/sandbox-rootfs/current \
    cargo test --test integration_tests -- --test-threads=1 \
    --ignored code_sandbox::tier6_

# Tier 5 (real LLM, costs ~$0.30 in API tokens):
ANTHROPIC_API_KEY=sk-ant-... \
    ZIEE_SANDBOX_ROOTFS=.ziee-cache/sandbox-rootfs/current \
    cargo test --test integration_tests -- --ignored chat::sandbox_real_llm

# Everything bwrap-needing in one shot (Tiers 4+5+6):
ZIEE_SANDBOX_ROOTFS=.ziee-cache/sandbox-rootfs/current \
    cargo test --test integration_tests -- --test-threads=1 \
    --ignored code_sandbox::tier4_ code_sandbox::tier6_
```

**Tier 6 is the layer that exercises the full production code path**
(real HTTP → real handler → real bwrap → real command → real response).
The lower tiers exercise individual layers but Tier 6 is what proves
the integration works end-to-end. Add new Tier-6 tests when shipping
new tool behaviors.

**Testing the auto-fetch path locally** (no GitHub release needed):

```bash
just dev-release minimal    # builds the rootfs + stages it in a local mirror
# Then boot the server with code_sandbox.enabled: true and trigger
# any execute_command MCP call — the server downloads from the local
# mirror, sha256-verifies, mounts, and runs.
```

`dev-release.sh` stands up a loopback HTTP "registry" via
`python3 -m http.server` and writes a dev-override
`known_revisions.dev.toml` with the freshly-built sha256. The two
env vars it sets (`CODE_SANDBOX_KNOWN_REVISIONS_OVERRIDE` and
loopback `http://` in `CODE_SANDBOX_ROOTFS_MIRROR`) are physically
compiled out of release builds via `cfg!(debug_assertions)`, so the
mechanism can't be abused in production. Cosign signing is
deliberately skipped (`signed = false` in the dev TOML); real
keyless cosign verification needs a true GitHub Actions OIDC run.

### Production deployment

Operator workflow is **install host deps → boot**. The server handles
everything else (download, sha256 + cosign verify, mount, unmount).

1. Install host deps:
   - **Linux:** `sudo apt install bubblewrap squashfuse fuse3` (Debian /
     Ubuntu; per-distro table above for Fedora / Arch / Alpine).
   - **macOS:** ensure the app bundle ships `libkrun.dylib` under
     `Contents/Frameworks/` (the `Cross-platform` section above).
     Apple Silicon required.
   - **Windows:** `wsl --update` to ≥ 2.5.10 / 2.6.1 (`probe_host`
     enforces this; older runtimes are refused with a clear log). The
     server imports the per-flavor distro + provisions it on first
     `execute_command` (bubblewrap + rsync from inside the distro).
     **Two one-time admin-shell steps** the user must run before the
     sandbox is usable:

     1. Hyper-V Administrators group (resolves the WSL utility VM's
        VmId via `hcsdiag list`):
        ```powershell
        net localgroup "Hyper-V Administrators" $env:USERNAME /add
        # Sign out + back in so the group SID attaches at the next login.
        ```

     2. AF_HYPERV port-template GUID registration (the
        `HV_GUID_VSOCK_TEMPLATE` family is not auto-routable from the
        Windows host to a WSL guest's AF_VSOCK listener; vmcompute
        rejects connect attempts with WSA 10060 unless the specific
        GUID is registered):
        ```powershell
        scripts/register-sandbox-vsock-ports.ps1
        # Registers ports 10001..10100 + runs `wsl --shutdown` so
        # vmcompute picks up the new registrations at the next VM boot.
        ```
2. Set `code_sandbox.enabled: true` in config.
3. Boot the server. The startup log shows
   `code_sandbox: registered (rootfs will mount on first execute_command)`.

Per-flavor lazy-fetch + lazy-mount: the FIRST `execute_command` MCP
call for each flavor triggers download (sha256 + sigstore verify) and
squashfuse mount. The chat UI surfaces a system note via
`structuredContent.fetch_info` on calls that did a fetch ("Fetched
'full' sandbox, 853 MB, 2m 14s"). Users who only invoke `minimal`
never pay the `full` download cost; users who never invoke
code execution at all never spawn squashfuse.

The hardening line appears in the log at first lazy-init:
`code_sandbox: hardening = { ... pid_ns: ..., cgroup_v2: ..., seccomp: ... }`.

The server is the sole owner of every spawned squashfuse process. It
spawns each with `PR_SET_PDEATHSIG=SIGTERM`, so FUSE daemons die with
the server even on `kill -9` / OOM-kill. There is no
`fetch-sandbox-rootfs` or `mount-sandbox-rootfs` CLI subcommand —
the runtime handles both.

**Air-gapped operators** that can't reach GitHub Releases can
pre-stage a rootfs squashfs manually. Copy a built
`ziee-sandbox-rootfs-v{schema}.{revision}-{arch}-{flavor}.squashfs`
file into the cache directory (default
`/var/lib/ziee/sandbox-rootfs/`); the runtime detects it on the
first `execute_command`, skips the network, and mounts.

For cgroup v2 enforcement (recommended), give the server uid a
delegated slice on the host:

```bash
sudo mkdir -p /sys/fs/cgroup/ziee-sandbox.slice
echo "+memory +pids +cpu" | sudo tee \
    /sys/fs/cgroup/ziee-sandbox.slice/cgroup.subtree_control
sudo chown -R <server-uid>:<server-gid> /sys/fs/cgroup/ziee-sandbox.slice
```

### Rootfs release process

See [`src-app/sandbox-rootfs/RELEASE-RUNBOOK.md`](./src-app/sandbox-rootfs/RELEASE-RUNBOOK.md)
for the bootstrap script (`scripts/bootstrap-first-rootfs-release.sh`)
and the ongoing release workflow.

---

## Biomedical MCP server (BioMCP)

The `bio_mcp` module exposes ~45 biomedical databases (PubMed,
ClinicalTrials.gov, ClinVar/MyVariant, UniProt, ChEMBL/OpenFDA/
OpenTargets, PharmGKB/CPIC, …) to the chat model as a **built-in MCP
server**, by vendoring + wrapping
[genomoncology/biomcp](https://github.com/genomoncology/biomcp) (MIT).
**On by default** for connected deployments.

### Architecture — proxy + managed sidecar

Unlike the other built-in MCP servers (memory/files/code_sandbox), which
are in-process Axum routes, BioMCP is an **external single-binary sidecar
with no auth of its own**. So ziee owns a thin proxy (`bio_mcp/`):

- A built-in `mcp_servers` row (`is_built_in=true`, `is_system=true`,
  `transport_type='http'`, deterministic id `bio.ziee.internal`) whose
  `url` is the ziee-owned route `POST/GET/DELETE /api/bio/mcp`.
- That route (`handlers.rs`) holds the JWT boundary
  (`RequirePermissions<(BioQuery,)>` → `bio::query`, granted to the Users
  group by migration 96), then transparently reverse-proxies the MCP
  streamable-HTTP body (JSON + SSE) to the sidecar's `/mcp`, forwarding
  ONLY the MCP protocol headers.
- `supervisor.rs` lazily spawns ONE long-lived
  `biomcp serve-http --host 127.0.0.1 --port <ephemeral>` per process on
  the first `/api/bio/mcp` call, `env_clear` + injects the configured API
  keys, polls `/readyz`, applies a flap backoff + idle-reaps it.
  `PR_SET_PDEATHSIG` (Linux) + `kill_on_drop` tear it down with the server.
- The `bio_mcp` chat extension (order 27, before `mcp` at 30) flags
  `attach_bio_mcp` on tool-capable chats when the row is enabled and
  injects a one-line untrusted-content guard; `auto_attach_builtin_ids` +
  `is_builtin_server_id` then auto-attach it and bypass per-call approval
  (read-only searches). The biomcp surface is a single `biomcp` tool.

### Key config — the standard MCP "Headers" editor (no bespoke UI)

BioMCP is an **admin-configurable** built-in (NOT in the zero-config edit
deny-list, so its row stays editable). Admins set the upstream API keys
(`NCBI_API_KEY`, `S2_API_KEY`, `OPENFDA_API_KEY`, `NCI_API_KEY`,
`ONCOKB_TOKEN`, `ALPHAGENOME_API_KEY`, `DISGENET_API_KEY`) as **secret
entries in the standard MCP system-server Headers editor**. The supervisor
reads the row's decrypted `headers` in-process and injects each as a
process env var into the sidecar — never sent over HTTP (the proxy strips
them); a name denylist (`PATH`/`HOME`/`LD_*`/`DYLD_*`) blocks loader
hijack. Unauthenticated works (rate-limited) when no keys are set.

### Threat model / egress

**Connected-only** — the sidecar queries live upstream APIs; an air-gapped
box gets almost nothing. **Query terms egress to public APIs**, so
IP-sensitive deployments turn it off with `bio_mcp: { enabled: false }`
(the deploy-level kill switch; the per-deployment admin toggle is the bio
row's `enabled` column). Coverage skews oncology. BioMCP feeds untrusted
third-party content into context — the untrusted-content guard + the
external-MCP posture are the mitigations.

### Binary delivery (build_helper + extract-on-first-use)

`build_helper/biomcp.rs` fetches the pinned `BIOMCP_VERSION` release per
target triple at build time, **mandatorily sha256-verifies** the `.sha256`
sidecar, stages it under `binaries/{target}/biomcp/` (+ a `.version`
sidecar so a `BIOMCP_VERSION` bump re-fetches), and `embedded.rs` bakes it
in via `include_bytes!` + extract-on-first-use to `~/.ziee/bin/biomcp`.
Fail-soft (mirrors pgvector): on any failure a zero-byte stub is staged →
`biomcp_available()` is false → the module self-disables at boot with a
clear log. Supported triples match uv/bun (Linux/macOS x86_64+arm64,
Windows x86_64). Override `BIOMCP_VERSION` / `BIOMCP_GITHUB_REPO` at build
time.

### Tests

| Tier | Where | Needs |
|---|---|---|
| 1 unit | `bio_mcp/{mod,supervisor}.rs` `#[cfg(test)]` + `mcp/chat_extension/mcp.rs` | nothing |
| 2 DB | `tests/bio_mcp/mod.rs::test_bio_row_registered_as_editable_builtin` | Postgres + staged binary |
| 3 HTTP | `tests/bio_mcp/mod.rs` (401 / 403 / graceful-503) | TestServer |
| 4 real sidecar | `tests/bio_mcp/mod.rs::test_real_sidecar_proxy_initialize` | staged binary (self-skips on a stub build) |

Enable bio in a test via `TestServerOptions { bio_mcp_enabled: true, .. }`
— it defaults OFF in tests so unrelated/chat tests never spawn the sidecar.

---

## Local LLM Runtime — testing

The `llm_local_runtime` module turns local engines (llama.cpp /
mistral.rs subprocesses) into an OpenAI-compatible provider via a
same-port reverse proxy at `/api/local-llm/v1/*`. The test suite covers
the full lifecycle without needing a published engine release.

The engine library code — binary download/extract/cache, GGUF/safetensors
metadata parsing, the per-engine settings vocabulary (`LlamaCppSettings` /
`MistralRsSettings`), and the health state machine — lives in
`src-app/server/src/modules/llm_local_runtime/engine/`, folded in from the
former standalone `llm-runtime` crate (now deleted; the server was its sole
consumer). The per-engine CLI arg-builders are in `deployment/local.rs`
(`llamacpp_argv` / `mistralrs_argv`); a model's `engine_settings` JSONB
deserializes into the typed settings, and the health state machine is wired
into `auto_start.rs`'s crash path (exponential backoff + a flap cap that
gives up after 5 crashes / 60s instead of re-spawning forever).

### Test fixtures

- **`stub-engine`** (`src-app/stub-engine/`, a workspace
  member, `publish = false`) — a tiny axum OpenAI-compatible server
  (`/health`, `/v1/chat/completions` incl. SSE, `/v1/embeddings`,
  `/v1/models`). It's spawned by the *real* deployment path exactly as if
  it were `llama-server`, so spawn → health → proxy forward → bearer
  rewrite → SSE all run for real; only token generation is canned. It
  ignores unknown llama-server flags; behaviour knobs come via the request
  body (`stub_hang_ms`, `stub_force_status`) or a `stub-unhealthy` path
  sentinel (env is wiped by the deployment's `env_clear`).
- **`MockReleaseServer`** (`src-app/server/tests/llm_local_runtime/mock_release.rs`)
  — packages the stub-engine as a release artifact and serves it from a
  loopback HTTP server, so `POST /versions/download` exercises the full
  download → extract → cache → register path. Mirrors
  `code_sandbox/mirror_fixture.rs`.

### Debug-only test env vars (compiled out of release builds via `cfg!(debug_assertions)`)

- `LLM_RUNTIME_RELEASE_MIRROR` / `LLM_RUNTIME_API_MIRROR` — override the
  GitHub release/API hosts in `llm_local_runtime/engine/download.rs` so the
  download path resolves against the mock release server.
- `LLM_RUNTIME_REAPER_TICK_MS` — shorten the idle-reaper's 60s tick so
  idle-eviction / drain tests observe behaviour in seconds
  (`llm_local_runtime/reaper.rs`).

These are the same testability-seam pattern as code_sandbox's
`CODE_SANDBOX_ROOTFS_MIRROR`; they cannot be set in a release build.

### Test tiers

| Tier | Where | Needs | Notes |
|---|---|---|---|
| 1 unit | in-source `#[cfg(test)]` in `proxy.rs`, `engine/{health,metadata,download,error}.rs`, `deployment/local.rs` (argv builders), `ai-providers/model_registry.rs` | nothing (Postgres only to *compile* the server lib) | token cache, state machine, GGUF parse, mirror-default, argv-shape |
| 2 integration | `server/tests/llm_local_runtime/*_test.rs` | Postgres + stub-engine; `model_files_real_test` also needs `HUGGINGFACE_API_KEY` + network | proxy auth/forward, lifecycle, reaper/drain, settings, token rotation, provider create, gpu-detect, sse-logs, validation, engine download, supervision (flap cap) |
| gold | `server/tests/llm_local_runtime/gold_smoke.rs` (`#[ignore]`) | a real `llama-server` + tiny GGUF | env-gated: `ZIEE_REAL_LLAMA_SERVER`, `ZIEE_REAL_GGUF` |
| 3 E2E | `ui/tests/e2e/12-local-runtime/` | Playwright; engine flows need an engine mirror | UI surface specs run engine-free; `04-engine-lifecycle` skips unless `ZIEE_E2E_ENGINE_MIRROR` is set |

```bash
# Tier 1
cd src-app && cargo test --lib -p ziee llm_local_runtime:: && cargo test -p ai-providers

# Tier 2 (needs the HF key for the real-download test)
source src-app/server/tests/.env.test
cargo test --test integration_tests llm_local_runtime:: -- --test-threads=1 \
    2>&1 | tee local-runtime-int-$(date +%Y%m%d-%H%M%S).log

# Gold smoke (manual, real engine)
ZIEE_REAL_LLAMA_SERVER=/path/llama-server ZIEE_REAL_GGUF=/path/tiny.gguf \
    cargo test --test integration_tests -- --ignored llm_local_runtime::gold_smoke

# E2E (UI surface) — always --workers=1
cd src-app/ui && npm run test:e2e -- tests/e2e/12-local-runtime --workers=1
```

**Build the stub first** (the mock fixture builds it on demand, but
pre-building avoids a nested cargo call during tests):
`cargo build -p stub-engine`.

**Engine settings (canonical names):** llama.cpp — `ctx_size`,
`n_gpu_layers`, `batch_size`, `threads`, `embeddings`, `rope_freq_base`,
`rope_freq_scale`; mistral.rs — `max_seqs`, `prefix_cache_n`, `dtype`,
`model_format`. These are the keys a model's `engine_settings` JSONB must
use (NOT the old `context_size`); the arg-builders deserialize + validate
them. mistral.rs uses the `gguf` / `plain` subcommand form — those flags
are **not yet verified against a real `mistralrs-server` binary** (no
binary available), only against the (now-deleted) reference crate + the
argv-shape unit tests; confirm against `--help` before relying on it.

---

## Chat Projects

Flat, per-user grouping above conversations. Each project owns:

- `instructions` (TEXT, capped at 64 KiB) — wrapped + injected as a
  system message into every conversation in the project.
- Attached files (M:N via `project_files`, hard-capped at 100 per
  project) — prepended onto the user message as provider-routed
  ContentBlocks.
- Default assistant + default model (nullable FKs, `ON DELETE SET NULL`).
- Inline MCP settings — snapshotted into the conversation's
  `conversation_mcp_settings` row at conversation create time. Snapshot,
  not read-through: subsequent project MCP edits do NOT propagate to
  existing conversations.

### Backend module

- Code: `src-app/server/src/modules/project/{mod,models,types,repository,routes,handlers,permissions,events}.rs`
- Migrations: `00000000000046..00000000000049` (projects table,
  project_files join, conversations.project_id ALTER, Administrators
  permission grant).
- Chat extension: `src-app/server/src/modules/chat/extensions/project/`
  at **order 8** — runs BEFORE the assistant extension (order 10) so
  the final wire format is `[assistant_sys, project_sys, user_msg]`
  (assistant at older position, project closer to the user message).
  Mutation logic lives in the pure function
  `apply_project_context(&mut ChatRequest, instructions, file_blocks)`
  so it's directly unit-testable.
- File→ContentBlock routing: shared `chat/extensions/file/processor.rs`
  `process_file_blocks()` — single source of truth for both the file
  extension and the project extension.

### API

13 endpoints under `/projects/*` — full CRUD + `/duplicate` + `/files`
(attach by ID + multipart upload+attach + detach + list) +
`/conversations` + `/mcp-settings` (get/put). Combined upload returns
**422** (not 400) when the 100-file cap is hit.

Cross-cutting on the chat module:
- `POST /conversations` accepts optional `project_id`; if set with no
  explicit `model_id` it snapshots `project.default_model_id`.
- `PUT /conversations/{id}` accepts tri-state `project_id`
  (missing/null/uuid) using the existing `deserialize_nullable_field`.
- `GET /conversations?project_id=null` filters to unfiled;
  `?project_id=<uuid>` filters to that project.
- `SendMessageRequest` does **NOT** accept `project_id` — project is
  derived server-side from `conversation.project_id` (security: clients
  cannot inject project Y's context into conversation X).

### Frontend module

`src-app/ui/src/modules/projects/` — stores (Projects, ProjectDetail,
ProjectDrawer), pages (ProjectsListPage, ProjectDetailPage), components
(ProjectFormDrawer, ProjectFilesPanel, ProjectConversationsList,
ProjectMcpSettingsPanel, ConversationProjectChip), sidebar widget
(`ProjectsNavWidget` in `sidebarNavigation` at order 5, above
`RecentConversationsWidget`).

The chat module's `RecentConversationsWidget` is wrapped at the slot
registration site with `projectIdFilter={null}` so it shows ONLY
unfiled conversations when the projects module is present (avoids
duplication with per-project lists).

**`pendingProjectId` contract**: `Stores.Chat` exposes a
`pendingProjectId` field. `NewChatPage` reads `?project_id=<uuid>` from
the URL on mount and calls `setPendingProjectId`. The next
`createConversation` call consumes + clears it (cleared on both success
and error so a failed send doesn't latch the project). This is the
mechanism by which "New chat in this project" affordances (from
ProjectsNavWidget hover + ProjectDetailPage header) thread the project
through the chat module.

### Tests

| Tier | Location | What's covered |
|---|---|---|
| 1 unit | `src/modules/project/{permissions,handlers}.rs` `#[cfg(test)]` + `chat/extensions/project/project.rs` `#[cfg(test)]` | Permission constants, name validator, text-length validators, file-count cap constant, and **8 wire-format mutation tests** on `apply_project_context()` covering stack-both, file prepend, assistant-layering, no-op cases |
| 2 integration | `tests/project/*.rs` | CRUD, permissions, ownership, file attach/detach/cascade/cap-422, conversation move + scoping, duplicate (incl. name-collision suffix), MCP snapshot, default_model snapshot |
| 3 real-LLM | `tests/project/injection_test.rs` | Real-provider tests that send a chat message and assert the LLM response reflects the project instructions / files / stacking. Gated on `ANTHROPIC_API_KEY` (or other provider keys) — skipped when unset. Mirrors `tests/chat/file_attachments_real_providers_test.rs` pattern |
| E2E | `tests/e2e/11-projects/` | Full Playwright flow: list/create/edit/attach/duplicate/delete-orphan/conversation-in-project + a real-LLM `message-uses-project-context` spec |

**Running just the project tests:**

```bash
# Tier 1
cargo test --lib -p ziee project::

# Tier 2 + 3 (Tier 3 skips when no API keys)
source tests/.env.test
cargo test --test integration_tests project:: -- --test-threads=1 \
    2>&1 | tee project-int-$(date +%Y%m%d-%H%M%S).log
```

---

## Realtime Sync

Cross-device sync over a per-user **Server-Sent-Events** stream. The wire
payload is **notify-and-refetch only** — `{entity, action, id}`, never row
data — so a misrouted event can't leak: the client refetches via the
existing permission-checked REST endpoint, and the SSE channel carries
nothing sensitive.

### Backend module

`src-app/server/src/modules/sync/{mod,event,registry,handlers,extractor}.rs`.

- **No central audience table.** Each emitting handler picks the `Audience`
  explicitly at the `publish(entity, action, id, audience, origin)` call site —
  `Audience::owner(user_id)` / `perm::<P>()` / `any_of::<L>()` / `all_of::<L>()`
  / `everyone()`. Adding a `SyncEntity` variant does NOT force an audience at
  compile time; the author must choose the correct audience at every emit site
  (owner-scope by default; never `everyone()` for per-user data). The perm used
  MUST equal the read-perm gating the client's refetch endpoint.
- **`registry.rs`** — per-user keyed connection pool (NOT the global
  broadcast pool used by download/hardware SSE). Caps: `512` global / `12`
  per-user / `1024` bounded channel depth (a stalled reader is pruned →
  the client reconnects + resyncs). Mutex is poison-recovering.
- **`handlers.rs`** — `GET /api/sync/subscribe`, gated by `profile::read`.
  Sends a `connected{connection_id}` handshake, keep-alive, then a
  `tokio::select!` over {channel recv, 60s re-check, JWT `exp` deadline}. The
  re-check re-resolves `is_active` + the baseline perm and tears the stream
  down on loss; a transient DB error keeps the stream.
- **`extractor.rs::SyncOrigin`** reads `X-Sync-Connection-Id` (the client
  echoes the connection id back on mutations) so the fan-out skips the
  originating connection (self-echo suppression).

### Emitting changes

Mutating handlers call `publish as sync_publish` with
`(entity, action, id, owner, origin.0)`. Conventions:

- **Owner-scoped** entities pass `Some(owner_id)`; **Permission/Everyone**
  pass `None`.
- **Dual-audience** mutations emit BOTH the admin entity AND the user-view
  entity (e.g. a provider change → `LlmProvider` for admins +
  `UserLlmProvider` for every user, each refetching its own scoped view).
- Group-permission edits fan a `Session` signal to **all members** via
  `publish_session_to_users` (one registry-lock batch) so their devices
  re-bootstrap `/auth/me` immediately (the 60s re-check is the backstop).
- Background/detached tasks (e.g. a runtime-version download completing, a
  model finishing an upload/download) emit on **completion**, with
  `origin = None`.

### Frontend

`src-app/ui/src/core/sync/` = `SyncClient.ts` (SSE loop + epoch-guarded
reconnect), `connection.ts` (header holder), `types.ts`, `index.ts`. There is
**no** `registry.ts`, **no** per-module `sync.ts`, and **no** `registerSync`.
Each module's Zustand store subscribes DIRECTLY in its `__init__.__store__` to
`sync:<entity>` (+ `sync:reconnect`) and refetches.

- The SyncClient re-emits each frame onto the existing EventBus as a
  per-entity `sync:<entity>` event; each store's `sync:<entity>` subscription
  refetches it (per-surface policy lives in the subscription). The generated
  `SyncEntity` TS union (`api-client/types.ts`) auto-derives the
  `sync:${entity}` EventBus key (`core/sync/types.ts`), so a new backend entity
  becomes a valid event key on the next OpenAPI regen.
- **No compile-time coverage gate** (no `ENTITY_COVERAGE`, no
  `assertSyncCoverage()`). Coverage is by convention: a store that cares about
  an entity subscribes to its `sync:<entity>` (~28 stores do today).
- **No-403 reconnect rule** is enforced by the store SELF-GATING its refetch:
  the `sync:<entity>` / `sync:reconnect` handler calls
  `hasPermissionNow(Permissions.X)` and returns early if the user lacks it
  (`sync:reconnect` fires for every store on reconnect regardless of server
  audience). The perm checked MUST equal the read-perm the refetch endpoint
  enforces. Examples: `mcp/stores/McpServer.store.ts:228-248`,
  `SystemMcpServer.store.ts:170-176`.

### Tests

| Tier | Location | Covers |
|---|---|---|
| unit | `modules/sync/{registry,event}.rs` `#[cfg(test)]` | audience routing isolation, self-echo skip, caps→429, snapshot refresh, lagging-conn prune, batch session delivery, the audience table + notify-only wire format |
| integration | `server/tests/sync/subscribe_test.rs` | subscribe auth-gate (401) + SSE handshake |
| E2E | `ui/tests/e2e/13-sync/` (`--workers=1`) | cross-device delivery without reload; cross-user isolation (A's 2nd device = positive control) |

---

## MCP Tool-Call History

Every MCP tool-call invocation is recorded to `mcp_tool_calls` (migration 105) —
the MCP analog of `workflow_runs`. Owner-scoped, surfaced per-server in the
McpServerDrawer "Calls" tab, with realtime refresh + time-based auto-pruning.

### Recording (the chokepoint)

Recording happens once, inside **`McpSession::call_tool`**
(`mcp/client/session.rs`), via an `McpCallContext` stamped onto the (ephemeral)
session at creation — so all paths (chat / rest / always / approval / sampling,
incl. built-ins) record without per-call-site duplication. The manager
(`get_or_create_with_context`, now `+ source`) stamps the chat/REST sessions;
the 3 `new_with_sampling` sites stamp via `set_call_context`. The insert is
**fire-and-forget** (`tokio::spawn`) so a DB hiccup can't fail the tool call;
an unstamped (pooled, non-tool-call) session records nothing. `ask_user` is
intercepted before `call_tool` and is correctly never recorded. The full
`ToolResult` is stored in `result_json` with base64 bytes stripped to
`{_stripped,_bytes}` references (only on binary content blocks), secret-keyed
values redacted, and the whole serialized result capped at 1 MiB; args are
likewise redacted + capped at 16 KiB (mirrors the chat path's result caps).

### Surfaces

- Table `mcp_tool_calls` (FKs from `tool_use_approvals`; terminal status
  `completed|failed|timeout|cancelled`; `source` enum; `is_built_in` flag).
- REST `GET /api/mcp/tool-calls?page&per_page&server_id&conversation_id` +
  `GET /api/mcp/tool-calls/{id}`, gated `mcp_servers::read` (held by Users),
  owner-scoped (cross-user single-row → 404). Code in `mcp/tool_calls/`.
- Owner-scoped sync entity `McpToolCall` (Create emitted from the record task,
  `origin=None`); the `McpToolCalls` store refetches on `sync:mcp_tool_call`.
- Retention: `mcp_user_policy.tool_call_retention_days` (admin-configurable on
  the existing User-MCP-policy card; 0 = keep forever); a boot-time prune loop
  (`mcp/tool_calls/prune.rs`, ~6 h) deletes older rows.

### Tests

- Unit: `mcp/tool_calls/record.rs` `#[cfg(test)]` (arg cap, byte-strip, status map).
- Integration: `tests/mcp/tool_call_history_test.rs` (record via REST +
  `MockMcpServer`, error path, owner-scope/404, perm-gate 403, retention
  roundtrip) + `tests/mcp/sync_emit_test.rs` (owner-scoped emit via `SyncProbe`).
- E2E: `ui/tests/e2e/07-mcp/mcp-tool-call-history.spec.ts` +
  `13-sync/mcp-tool-call-sync.spec.ts` (live cross-device delivery).

---

## Web Search + Page Fetch

The `web_search` module exposes web **search** + page **fetch** as a built-in
MCP server (`web_search.ziee.internal`, loopback JSON-RPC at
`/api/web-search/mcp`), modeled on `memory_mcp`/`files_mcp`. Two tools:
`web_search(query, max_results?)` and `fetch_url(url)`. Connected-only;
degrades silently (tools simply not attached) when offline / unconfigured.

### Provider registry + fallback chain

The *set* of engines lives in **code** — `modules/web_search/providers/mod.rs`
`catalog()` (v1: `searxng`, `brave`). The DB (`web_search_providers`) only
stores `{api_key, config}` per registry key, so adding Tavily/Exa/Google-CSE
is a code-only change (a `SearchProvider` impl + a `catalog()` entry + a
`build()` arm) — **no migration, no frontend change** (the admin UI renders
from the descriptor catalog via `GET /api/web-search/providers`).

`search_via_chain` walks `web_search_settings.provider_chain` in order: skip
unconfigured entries; call `search`; **fall back to the next entry only on
error/timeout/quota** — a successful (even empty) result is final. The
`structuredContent.provider` names the engine that served.

### SSRF (two trust boundaries)

Reuses `utils/url_validator.rs`. The untrusted, model-supplied **page-fetch**
URL uses `PUBLIC_HTTP_OR_HTTPS` (blocks loopback/RFC1918/IMDS; redirects
re-validated). The admin-configured **SearXNG** base URL is trusted, so it uses
a custom policy literal that allows private/loopback (a self-hosted SearXNG on
a LAN is the common case). Brave uses `STRICT`. Fetched content is third-party
data — the tool descriptions + a system nudge tell the model never to follow
instructions embedded in it.

### Enablement + keys

The MCP server row is **always registered**; the chat extension
(`chat_extension/`, order **26** — before the MCP tool-collector at 30) only
sets the `attach_web_search_mcp` flag (read by `auto_attach_builtin_ids` in
`mcp/chat_extension/mcp.rs`) when the model is tool-capable, web search is
`enabled`, and ≥1 chain provider is configured. **Forgetting the two `mcp.rs`
edits** (`auto_attach_builtin_ids` + `is_builtin_server_id`) is a silent
failure: the server registers and curl works, but the model never sees the
tools. Keys are **deployment-wide** (admin-configured, shared; no per-user),
encrypted at rest via `common::secret` (dual-column + `SecretView` redaction).

### Settings + surfaces

- Singleton `web_search_settings` (enable, `provider_chain TEXT[]`, caps) +
  per-engine `web_search_providers` (migration `097`); `web_search::use`
  granted to the Users group (migration `098`); admins hold
  `web_search::admin::{read,manage}` via `*`.
- REST: `GET/PUT /api/web-search/settings`, `GET /api/web-search/providers`,
  `PUT /api/web-search/providers/{provider}`. Sync entity `WebSearchSettings`.
- Admin page at `/settings/web-search` (`ui/src/modules/web-search/`): global
  card + reorderable provider-chain editor + generic per-provider config cards.

### Tests

| Tier | Where | Covers |
|---|---|---|
| 1 unit | `modules/web_search/{providers,fetch}.rs` + `mcp/chat_extension/mcp.rs` `#[cfg(test)]` | chain dispatch, `is_configured`, readability→markdown extraction + char-truncation, SSRF policy selection, the `auto_attach`/`is_builtin` web_search branches |
| 2 integration | `tests/web_search/settings_test.rs` | settings GET/PUT, 403 gating, **API key stored-but-never-returned**, chain/caps validation (400) |
| 3 HTTP handler | `tests/web_search/mcp_test.rs` | JSON-RPC initialize/tools-list, `use`-permission gate, no-provider error, **search via a mock SearXNG**, **fetch via a loopback fixture** |

**Debug-only test seams** (compiled out of release via `cfg!(debug_assertions)`;
cannot be set in production):
- `WEB_SEARCH_FETCH_ALLOW_LOOPBACK=1` relaxes the page-fetch policy to
  `DEV_LOCAL` so a `127.0.0.1` page fixture is reachable. SearXNG tests need no
  seam (its trusted policy already allows loopback).
- `WEB_SEARCH_BRAVE_ENDPOINT=<url>` overrides Brave's endpoint (and relaxes its
  policy to `DEV_LOCAL`) so a loopback mock can stand in for the SaaS — used to
  drive the live `[searxng→error, brave→serves]` fallback test.

Same pattern as `CODE_SANDBOX_ROOTFS_MIRROR` / `LLM_RUNTIME_*_MIRROR`.

```bash
# Tier 1
cargo test --lib -p ziee web_search::
# Tier 2 + 3 (scoped)
cargo test --test integration_tests web_search:: -- --test-threads=1
```

---

## Live Literature Search & Screening

The `lit_search` module exposes scholarly literature **search** + open-access
full-text **fetch** as a built-in MCP server (`lit_search.ziee.internal`,
loopback JSON-RPC at `/api/lit-search/mcp`), modeled on `web_search`. Two tools:
`literature_search(query, max_results?, year_from?, year_to?)` and
`fetch_paper_fulltext(ids, max_papers?)`. Connected-only; an adjunct to (never a
replacement for) systematic searching — results are untrusted DATA.

### Source registry + UNION aggregation

The *set* of sources lives in **code** — `modules/lit_search/connectors/mod.rs`
`catalog()` (6: `europepmc`, `crossref`, `semanticscholar`, `pubmed`, `arxiv`,
`core`). The DB (`lit_search_connectors`) stores only `{api_key, config}` per
key, so adding a source is a code-only change (a `LitConnector` impl + a
`catalog()` entry + a `build()` arm). `aggregate_search` fans out to all enabled
connectors **concurrently** (UNION, not a fallback chain): a failing source
contributes zero records and lands in `degraded_sources`; the rest still return.
Then `dedup::merge_by_doi` → `ranking::rank` → `completeness::estimate`. Five
sources work **keyless** (default-enabled); **CORE** needs a free key
(default-off, self-skips into `degraded_sources` when enabled-but-unkeyed).

### Full text + the `/lit` sandbox mount

`fetch_paper_fulltext` resolves OA full text (EuropePMC `fullTextXML` / Unpaywall
PDF→pdfium / arXiv PDF), caches it content-addressed under
`<app_data>/lit-cache/` with a Postgres index (`lit_fulltext_cache`), and
hard-links it into a **per-conversation view dir** that `code_sandbox`
bind-mounts **read-only at `/lit`** (so the model can `cat`/`grep` papers). Status
vocabulary is exactly `full_text | not_open_access | not_found` (no
`abstract_only`). Negative rows carry a 6h TTL (transient failures re-resolve);
`full_text` rows never expire. Paywalled → `not_open_access` (no scraping).

### SSRF (two boundaries)

`connectors/mod.rs::connector_policy()` builds the HTTP client with
`PUBLIC_HTTP_OR_HTTPS`. For the search connectors (FIXED public hosts) it's
defense-in-depth; for the **full-text resolver** (which fetches Unpaywall-supplied
PDF URLs — third-party-controlled hosts) it's a **PRIMARY** SSRF boundary. Do not
weaken it.

### Enablement, keys, kill switch

- Deploy-level kill switch: `lit_search: { enabled: false }` in config (skips MCP
  registration entirely — distinct from the runtime `lit_search_settings.enabled`
  admin toggle, the sole attach gate). Query terms egress to public APIs.
- Keys are **deployment-wide** (admin-configured, encrypted via `common::secret`);
  optional keys only raise rate limits, except CORE's required key.
- Chat extension (`chat_extension/`, order **28** — after web_search/bio_mcp,
  before the MCP collector at 30) sets `attach_lit_search_mcp`; the two `mcp.rs`
  edits (`auto_attach_builtin_ids` + `is_builtin_server_id`) are required or the
  tools register but the model never sees them.

### Settings + surfaces

- Singleton `lit_search_settings` + per-source `lit_search_connectors` +
  `lit_fulltext_cache` (migration `100`); `lit_search::use` granted to Users
  (migration `101`); admins hold `lit_search::admin::{read,manage}` via `*`.
- The built-in MCP row is **hidden from the System MCP page + edit-deny-listed**;
  config lives only on `/settings/literature` (frontend module `literature`).
- Screening UX is the chat **right-panel** (`literature` panel renderer);
  screening state persists in the serializable panel-tab `data` (no server-side
  tables). Sync entity `LitSearchSettings`.

### Two chat/MCP-core improvements this module shipped (benefit ALL built-in tools)

- **`structured_content` persisted** on `tool_result` blocks (size-capped at 1 MB
  in `mcp/chat_extension/helpers.rs`; the model reads it only via `get_tool_result`).
- **`tool_result_mcp`** built-in (`get_tool_result(tool_use_id, offset?, max_chars?)`,
  loopback `/api/tool-result/mcp`) — exact, paged, read-only recall of a prior
  result (incl. its `structured_content`), conversation-ownership-scoped. The
  cleared/truncated placeholder in `clear_old_tool_results` points the model at it.
- **`web_search` retrofit**: emits a readable text digest + typed
  `structuredContent`, never stringified JSON in the text channel.

### Debug-only test seams (compiled out of release via `cfg!(debug_assertions)`)

- `LIT_SEARCH_ALLOW_LOOPBACK=1` relaxes the SSRF policy to `DEV_LOCAL`.
- `LIT_SEARCH_<SOURCE>_ENDPOINT` (e.g. `LIT_SEARCH_EUROPEPMC_ENDPOINT`,
  `LIT_SEARCH_PUBMED_ESEARCH_ENDPOINT`, `LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT`) redirects
  a connector/resolver at a loopback mock — mirrors web_search's
  `WEB_SEARCH_BRAVE_ENDPOINT`.

```bash
# Tier 1
cargo test --lib -p ziee lit_search:: tool_result_mcp::
# Tier 2 + 3 (scoped; Tier 3 /lit mount is #[cfg(target_os = "linux")] + rootfs-gated)
cargo test --test integration_tests lit_search:: tool_result_mcp:: -- --test-threads=1
```

---

## MCP tool-result file persistence (`resource_link` + `ziee://`)

A `tool` result can carry a `resource_link` content block pointing at a **file** the tool
produced (chart / PDF / CSV). `modules/mcp/resource_link.rs::persist_links` is the single
in-process consumer that turns those links into durable file-store artifacts. It is called
by the chat MCP path (both `chat_extension/mcp.rs` save sites) and is call-ready for the
workflow tool dispatcher.

Per link it dispatches on the URI:
- `is_saved == Some(true)` — already in the store; referenced, never re-saved.
- `ziee://<host_abs_path>` — a **trusted in-process** tool (e.g. `code_sandbox::get_resource_link`
  for a transient workspace artifact) placed a file on the host; the bytes are read straight
  off disk (no JWT/HTTP) behind three guards, ingested, and the URI rewritten to
  `/api/files/{id}`.
- else (`is_saved:false`/`None`, an HTTP URL) — fetched over HTTP (built-in JWT or external
  headers), ingested; skipped when no `jwt_secret` (the dispatcher passes `None`).

**The three `ziee://` guards (security-critical):**
1. **Trusted-emitter only** — `is_trusted_resource_emitter(server_id)` (all deterministic
   built-in ids; a SUPERSET of the approval-bypass `is_builtin_server_id`, adding
   code_sandbox/skill/workflow). External/user servers are never honored.
2. **Confine + canonicalize** — `confine_under_roots`: canonicalize (resolve symlinks),
   require the path under an `allowed_root` (the per-conversation sandbox workspace for chat;
   the run workspace for the dispatcher). Plus a non-absolute reject.
3. **Strip-before-client** — saved links' URIs become `/api/files/{id}`; any *unsaved*
   host-path `ziee://` link (failed/rejected/untrusted) has its URI blanked; and
   `structured_content` is scrubbed of `ziee://<host-path>` strings at capture
   (`helpers::execute_tool`, defense-in-depth for the `get_resource_link` →
   `structuredContent` channel that's persisted + shipped + recalled via `tool_result_mcp`).
   The single `is_ziee_host_path` predicate (absolute remainder) drives every blank/scrub
   decision, so `ziee://workflow-runs/...` handles are **preserved**. The raw host path
   never reaches the browser/LLM.

### `ziee://` scheme registry — TWO distinct in-tree meanings (do not conflate)
- `ziee://<absolute-host-path>` — **this** consumer scheme (a host file to ingest). Always
  absolute; consumed only by `persist_links`.
- `ziee://workflow-runs/<run>/...` — `workflow_mcp`'s logical **resource** handle, served as
  an MCP `resource` block (NOT a `resource_link`), parsed by `workflow_mcp/resources.rs`. Its
  remainder is relative, so `persist_links`' non-absolute reject rejects it even if one ever
  reached it. Keep the two dialects distinct when adding either.

### Run-link note
`persist_links` accepts `workflow_run_id: Option<Uuid>` and, when `Some`, links each
newly-ingested (run-created) file to the run via `Repos.file.set_workflow_run_id` after the
save loop — so the A5 cascade deletes only files a run CREATED, never `is_saved:true` files it
referenced. The workflow `ToolDispatcher` passes `Some(run_id)`; the chat path passes `None`.
(`files.workflow_run_id` + `set_workflow_run_id` are present as of PR #110.) The shared save
tail is `file::ingest::ingest_bytes` — one path for the chat resource_link save, the workflow
tool-step save, and run-artifact collection. `created_by="workflow"` is the run-provenance
value (see migration 34's vocabulary).

```bash
cargo test --lib -p ziee mcp::resource_link::
cargo test --test integration_tests mcp::resource_link -- --test-threads=1
```

---

## Citation Management + Verification

The `citations` module is a built-in MCP server (`citations.ziee.internal`,
loopback JSON-RPC at `/api/citations/mcp`) backing a **persistent, verified,
CSL-JSON bibliography** (a user-level library + per-project reference lists). The
defining rule is **never invent a citation** — every DOI/PMID must resolve to a
real record. A reduced scope of the "scientific-writing" feature (drafting/
editor/manuscript-export were dropped). It reuses `lit_search`'s
`dedup::normalize_doi`; the resolver is otherwise self-contained (doi.org /
NCBI / Crossref), using no lit_search connectors.

### Storage

- `bibliography_entries` (migration `102`) — the user library. The full record is
  **CSL-JSON in a `JSONB` column** (source of truth); scalar columns
  (`doi`/`pmid`/`pmcid`/`arxiv_id`/`title`/`year`/`citation_key`/`dedup_fingerprint`/
  `verification_status`) are a projection for indexing/dedup/search. Partial unique
  indexes on `(user_id,lower(doi))`, `(user_id,pmid)`, and `(user_id,dedup_fingerprint)
  WHERE doi IS NULL AND pmid IS NULL` are the race-safe dedup guards; GENERATED
  `content_tsv` for FTS.
- `project_bibliography` (migration `103`) — M:N link to projects (mirrors
  `project_files`). One library; a project's list is links, not copies.
- `citations::use` + `citations::manage` granted to the Users group (migration `104`).

### Resolve + verify (the engine — `resolve.rs` / `verify.rs`)

- **DOI** → doi.org content negotiation (`Accept: application/vnd.citationstyles.csl+json`)
  → CSL-JSON; **arXiv** → `10.48550/arXiv.<id>` → doi.org; **PMID/PMCID** → NCBI
  ID-Converter → DOI → doi.org; **title/raw** → Crossref bibliographic query → best
  title-match → its DOI. 404 ⇒ `not_found` (fabricated).
- **Status** ∈ `verified | mismatch | not_found | unverified`. `not_found` is reserved
  for a *supplied id that fails to resolve*; an identifier-less item rests at
  `unverified` (legitimate — books/theses/datasets), NOT a red flag.
- **Title-match heuristic** (from the user's `doi-to-ref.js`): normalize + substring
  OR ≥60% word overlap.
- **Dedup**: normalized DOI → PMID → exact `dedup_fingerprint` (auto-link) → fuzzy
  near-match (flagged `possible_duplicate` for review, never auto-merged).

### Tools + REST

- MCP tools (batch-first, `items[]`): `lookup_citations` / `add_citations` /
  `verify_citations` / `list_citations` / `format_citations` / `remove_citations`.
  Per-item input is a flexible `CitationInput` — the model sends `id`/`title`/`csl`/`raw`
  and is **never required to supply a DOI**. Cap = 100 items/call (over-cap errors, no
  silent truncation). Auto-attached to tool-capable chats via the `citations`
  chat-extension flag + the two `mcp.rs` edits (`auto_attach_builtin_ids` +
  `is_builtin_server_id`).
- REST (`rest.rs`, OpenAPI `Citations.*`): `GET /api/citations`, `POST /api/citations/
  import|verify`, `GET /api/citations/export|styles`, `DELETE /api/citations/{id}`,
  `POST/DELETE /api/projects/{project_id}/citations[/{entry_id}]`.

### Format/export (`format.rs` + `csl.rs`)

CSL-JSON / BibTeX (embedded pandoc `-f csljson -t bibtex`, double-braced titles) / RIS
(pure-Rust writer — pandoc has no RIS *writer*) / CSL-styled text (pandoc `--citeproc`).
CSL styles bundle from `resources/csl/` via `include_dir!` (CC BY-SA 3.0 + NOTICE; **no
locales** — pandoc 3.x ships them). With no bundled `.csl`, text uses pandoc's built-in
default. **Verified pandoc 3.7 readers**: bibtex/biblatex/ris/endnotexml/csljson (NOT
`.nbib`/MEDLINE).

### Frontend (`ui/src/modules/citations/`)

Settings page at **`/settings/citations`** (`settingsUserPages` slot) — card list +
verification badges + Import/Verify-all/Export. A project's reference list is a
`knowledge_kinds` project-extension ("References", next to "Knowledge files"). No editor.

### Debug-only test seams (`cfg!(debug_assertions)`)

`CITATIONS_RESOLVER_ENDPOINT` (doi.org), `CITATIONS_IDCONV_ENDPOINT`,
`CITATIONS_CROSSREF_ENDPOINT` + `CITATIONS_ALLOW_LOOPBACK=1` — point the resolver at a
loopback mock for deterministic tests (mirrors `WEB_SEARCH_BRAVE_ENDPOINT`).

```bash
# Tier 1 (unit)
cargo test --lib -p ziee citations::
# Tier 2/3 (integration + MCP + mock-resolve)
cargo test --test integration_tests citations:: -- --test-threads=1
```

---

## Documentation Index

### 📐 Architecture

**[UI Meta-Framework Architecture](./.claude/META_FRAMEWORK_ARCHITECTURE.md)**
- Module system with auto-discovery
- Store system (Zustand with proxies)
- Event bus (type-safe, decoupled)
- Slot system (extensible UI)
- Router integration
- Complete module examples

**[React Component Patterns](./.claude/REACT_COMPONENT_PATTERNS.md)** ⚠️ CRITICAL
- Correct store access patterns
- Permission gating (Can / usePermission / slot field)
- Anti-patterns to avoid
- Initialization system
- Error handling
- Loading states

**[Permission Gating](./.claude/PERMISSION_GATING.md)** ⚠️ CRITICAL (when adding admin features)
- The `PermissionExpr` type and four gating layers (slot → route → `<Can>` → `usePermission`)
- Root admin vs Administrators group
- Wildcards and `is_admin` short-circuit
- Slot fields + route field for declarative gating
- Checklist for adding a new feature
- Anti-patterns to avoid

**[Frontend Dependency Hygiene](./.claude/FRONTEND_DEPS.md)**
- `npm run check` gate (tsc + antd doctor + antd lint)
- `@ant-design/cli` workflow + `just antd-check`
- Within-major vs cross-major bump cadence
- Common antd v6 deprecation fixes
- Deferred major bumps + why

**[Backend Architecture](./.claude/BACKEND_ARCHITECTURE.md)**
- Rust module system
- Permission system (RBAC)
- OpenAPI integration
- Error handling patterns
- Database integration (SQLx)

### 🧪 Testing

**[Testing Guide](./.claude/TESTING_GUIDE.md)**
- E2E testing with Playwright
- Semantic selectors (accessibility-first)
- Component selectors (auto-generated)
- Backend integration tests
- Accessibility testing (WCAG 2.1 AA)
- Test best practices

### 🔧 Development

**[Development Guide](./.claude/DEVELOPMENT_GUIDE.md)**
- Running the application
- Development workflow
- Building for production
- Module porting guide
- Common tasks
- Troubleshooting

---

## Project Overview

**Stack:**
- **Backend:** Rust + Axum + PostgreSQL
- **Frontend:** TypeScript/React + Zustand
- **API:** OpenAPI 3.0 (auto-generated types)
- **Auth:** JWT (Local, LDAP, OAuth2)
- **Authorization:** RBAC with fine-grained permissions

**Architecture:**
- Modular plugin-based system
- Declaration merging for type safety
- Event-driven cache invalidation
- Lazy loading and code splitting

---

## Key Concepts

### Backend Module Structure

```
modules/example/
├── mod.rs           # Module definition
├── routes.rs        # API handlers & OpenAPI docs
├── models.rs        # Request/response types
├── repository.rs    # Database layer
└── permissions.rs   # Permission definitions
```

**Learn more:** [Backend Architecture](./.claude/BACKEND_ARCHITECTURE.md)

### Frontend Module Structure

```
modules/example/
├── module.tsx       # Module registration
├── types.ts         # Type declarations
├── stores/          # Zustand stores
├── events/          # Event definitions
├── components/      # UI components
└── widgets/         # Reusable widgets
```

**Learn more:** [UI Architecture](./.claude/META_FRAMEWORK_ARCHITECTURE.md)

---

## Common Workflows

### Adding a New Feature

1. **Backend:** Create models → Define permissions → Implement routes → Generate OpenAPI
2. **Integration Tests:** Write tests → Verify all pass
3. **Frontend:** Create stores → Define events → Build components → Register module
4. **E2E Tests:** Write tests following semantic selector patterns

**Detailed guide:** [Development Guide - Adding a New Feature](./.claude/DEVELOPMENT_GUIDE.md#adding-a-new-feature)

### Module Porting

When porting from reference project:
1. **NEVER write from scratch** - Copy, then refactor
2. **Backend first** - Complete 8 phases
3. **Write integration tests** - Verify all pass
4. **Frontend next** - Complete 8 phases
5. **Write E2E tests** - Verify behavior

**Detailed guide:** [Development Guide - Module Porting](./.claude/DEVELOPMENT_GUIDE.md#module-porting)

---

## Quick Reference

### Generate OpenAPI

```bash
cd src-app/server
CONFIG_FILE=config/dev.yaml cargo run -- --generate-openapi
```

Generates: `ui/src/api-client/openapi.json` and `ui/src/api-client/types.ts`

### Run Tests

```bash
# Backend integration tests (IMPORTANT: Source env file first!)
source tests/.env.test
cargo test --test integration_tests -- --test-threads=1

# Or in one command
source tests/.env.test && cargo test --test integration_tests -- --test-threads=1

# Run specific module tests
source tests/.env.test && cargo test --test integration_tests assistant:: -- --test-threads=1

# Frontend E2E tests
npm run test:e2e

# Specific E2E test
npm run test:e2e -- tests/e2e/users/users.spec.ts
```

**⚠️ CRITICAL: ALWAYS Save Full Test Logs**

When running tests, **ALWAYS redirect full output to a file**. Never rely on filtered/grepped output or background jobs alone.

```bash
# ✅ CORRECT: Save full logs for later analysis
source tests/.env.test && cargo test --test integration_tests -- --test-threads=1 2>&1 | tee test-results-$(date +%Y%m%d-%H%M%S).log

# ✅ CORRECT: For specific modules
source tests/.env.test && cargo test --test integration_tests chat:: -- --test-threads=1 2>&1 | tee chat-tests-$(date +%Y%m%d-%H%M%S).log

# ❌ WRONG: Filtering loses critical failure details
cargo test 2>&1 | grep "FAILED"  # Can't see which test failed!

# ❌ WRONG: Background jobs make logs hard to retrieve
cargo test &  # Output is lost or fragmented
```

**Why this matters:**
- Full logs show **which specific tests** failed (not just the count)
- Failure details include assertion messages, panic locations, and stack traces
- Re-running tests to get details wastes time and money
- Test output can be 10+ minutes and thousands of lines - impossible to reconstruct

**Recommended workflow:**
1. Run tests with `tee` to save full output while seeing progress
2. When tests complete, check the saved log file for failure details
3. Use `grep` on the saved log file to extract specific information

**CRITICAL: Integration Test Requirements**
1. **MUST source `tests/.env.test` before running tests:**
   - Sets `HUGGINGFACE_API_KEY` for download tests
   - Without this, 11 llm_model download tests will fail
   - File location: `/home/pbya/projects/ziee/src-app/server/tests/.env.test`

2. **Test execution:**
   - Use `--test-threads=1` to avoid database conflicts
   - Tests take ~8-10 minutes to run sequentially
   - Expected result: 253-256/256 tests pass
   - Occasional flaky failures (1-3 tests) due to connection timeouts are normal - re-run those specific tests

3. **Without env file:**
   - 245/256 tests pass
   - 11 failures in `llm_model::download_*` tests (expected)

### Database Migrations

```bash
# Create migration
sqlx migrate add description

# Run migrations
sqlx migrate run

# Revert last
sqlx migrate revert
```

### Kill Stale Vite Processes

**CRITICAL: When code changes don't appear during E2E tests:**

E2E tests start their own Vite dev servers, but old Vite processes can persist and serve stale cached code. This prevents code changes (including console.log statements and UI updates) from taking effect.

**Symptoms:**
- Console logs added to code don't appear in test output
- UI changes (e.g., changed placeholder text) don't show up
- Store initialization changes don't execute
- Changes work in manual testing but not in automated tests

**Solution:**
```bash
# ✅ CORRECT: Kill only Vite processes
pkill -f "vite --config"

# ❌ WRONG: Don't kill all node processes (breaks other services)
killall -9 node
```

**When to use:**
- After adding debug logging to stores or components
- After modifying store initialization code
- When changes work in dev mode but not in tests
- As a first troubleshooting step when tests behave unexpectedly

**Note:** This is different from clearing Vite cache (`rm -rf node_modules/.vite`). The issue is running processes serving stale code, not cached files.

---

## Critical Patterns

### Frontend Store Usage

```typescript
// ✅ CORRECT: Declarative store access
export function MyComponent() {
  const { items, loading } = Stores.MyStore  // State (reactive)

  const handleCreate = () => {
    Stores.MyStore.createItem({ name: 'New' })  // Action (direct call)
  }
}

// ❌ WRONG: Never use hooks directly
const store = useMyStore()  // Don't do this!

// ❌ WRONG: Never manually load in useEffect
useEffect(() => {
  if (!isInitialized) {
    Stores.MyStore.loadItems()  // Don't do this!
  }
}, [isInitialized])
```

**Learn more:**
- [React Component Patterns](./.claude/REACT_COMPONENT_PATTERNS.md) ⚠️ **MUST READ**
- [UI Architecture - Store Usage](./.claude/META_FRAMEWORK_ARCHITECTURE.md#23-store-usage-pattern)

### E2E Selector Priority

1. `getByRole()` - Semantic (always prefer)
2. `getByLabel()` - Form controls
3. `getByText()` - Visible text
4. `getByTestId()` - Escape hatch (last resort)

**Learn more:** [Testing Guide - Selector Priority](./.claude/TESTING_GUIDE.md#selector-priority)

### Event Emission

```typescript
// All mutations must emit events
createItem: async (data) => {
  const item = await ApiClient.Item.create(data)
  await emitItemCreated(item)  // CRITICAL: Emit for cache invalidation
  set(state => ({ items: [...state.items, item] }))
}
```

**Learn more:** [UI Architecture - Event System](./.claude/META_FRAMEWORK_ARCHITECTURE.md#3-event-system)

---

## Known Issues

### Event-Only Widget Architecture (LLM Provider Group Assignments)

**Problem:** The `LLMProviderGroupWidget` and similar widgets rely **exclusively** on event-driven updates with **no mount-time data fetching**. This creates a fundamental incompatibility with page reload testing strategies.

**Architecture:**
```typescript
// LLMProviderGroupWidget.tsx
export function LLMProviderGroupWidget({ group }: GroupWidgetProps) {
  // ❌ NO useEffect - widget NEVER fetches data on mount!
  const groupData = Stores.LlmProviderGroupWidget.groupProviders.get(group.id)
  const providers = groupData?.providers || []

  // Widget only updates when 'llm_provider.group_providers_changed' event fires
}
```

**Why This Breaks Testing:**

1. **Normal flow (when working):**
   - User saves assignment → API call succeeds
   - Store calls `emitGroupLlmProvidersChanged()`
   - Widget's event listener receives event
   - Widget updates state with new data

2. **After page reload (BROKEN):**
   - Page reload → destroys all React state and event listeners
   - Widget re-mounts → NO `useEffect` to load data
   - Widget subscribes to events → but no event fires on mount
   - Widget stays empty forever

**Evidence from Test Failures:**

Error context HTML from `test-results/.../error-context.md`:
```yaml
- strong [ref=e307]: LLM Providers
- generic [ref=e309]: (0)                    # ← Widget shows 0 providers
- generic [ref=e316]: No providers assigned  # ← Widget shows empty state
```

Despite successful API calls, widgets remain stuck at "(0)" / "No providers assigned" after page reload.

**Timeout Escalation Anti-Pattern:**

If you find yourself increasing timeouts from 1s → 5s → 10s, this indicates a fundamental architectural problem, not a timing issue. No amount of waiting will help because the events that trigger updates will never fire after a page reload.

**Possible Solutions:**

1. **Fix the widget architecture (RECOMMENDED):**
   - Add `useEffect` to fetch data on mount
   - Example: `useEffect(() => { Stores.MyWidget.loadData(groupId) }, [groupId])`
   - Maintain event-driven updates for real-time changes

2. **Alternative test strategy:**
   - Don't reload page during tests
   - Manually trigger events after save operations
   - Use API polling instead of event-driven updates

3. **Document and skip:**
   - Mark affected tests as known failures
   - Document the architectural limitation
   - Wait for widget refactor before fixing tests

**Affected Components (FIXED):**
- `LLMProviderGroupWidget.tsx` - Provider assignments in user groups
- `GroupSystemMcpServersWidget.tsx` - MCP server assignments in user groups
- `McpServerGroupsAssignmentCard.tsx` - Group assignments on MCP server detail page
- `ProviderGroupAssignmentCard.tsx` - Group assignments on provider detail page (already had useEffect)

**Reference:**
- Helper file: `tests/e2e/05-llm/helpers/group-provider-helpers.ts`
- Widget: `src/modules/llm-provider/widgets/LLMProviderGroupWidget.tsx`
- Store: `src/modules/llm-provider/widgets/LLMProviderGroupWidget.store.ts`

---

## Paths

- **Reference Project:** `/home/pbya/projects/ziee-ref`
- **Active Project:** `/home/pbya/projects/ziee`
- **Backend:** `/home/pbya/projects/ziee/src-app/server`
- **Frontend:** `/home/pbya/projects/ziee/src-app/ui`

---

## Resources

**Documentation:**
- [UI Meta-Framework Architecture](./.claude/META_FRAMEWORK_ARCHITECTURE.md) - Complete frontend patterns
- [Backend Architecture](./.claude/BACKEND_ARCHITECTURE.md) - Rust/Axum patterns
- [Testing Guide](./.claude/TESTING_GUIDE.md) - E2E and integration testing
- [Development Guide](./.claude/DEVELOPMENT_GUIDE.md) - Running, building, porting

**External:**
- [Rust Book](https://doc.rust-lang.org/book/)
- [Axum Documentation](https://docs.rs/axum/)
- [React Documentation](https://react.dev/)
- [Playwright Documentation](https://playwright.dev/)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)

---

**Last Updated:** 2025-01-08
