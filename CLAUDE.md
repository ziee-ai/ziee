# Ziee Chat - Developer Documentation Hub

Essential documentation for developing Ziee Chat, a full-stack application with Rust backend and React meta-framework frontend.

---

## Quick Start

```bash
# Backend
cd src-app/server
CONFIG_FILE=config/dev.yaml cargo run

# Frontend
cd src-app/ui
npm run dev
```

Access at http://localhost:5173

---

## Development Environment

### Docker Compose

**Location:** `/home/pbya/projects/ziee-chat/src-app/docker-compose.yaml`

**IMPORTANT:** When working with database schema changes:
1. The docker-compose file is in `src-app/` directory, NOT the project root
2. To reset the database after migration changes:
   ```bash
   cd /home/pbya/projects/ziee-chat/src-app
   docker compose down
   docker compose up -d
   ```
3. The PostgreSQL build container is named `ziee-chat-postgres-build-1`
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
     `execute_command` (bubblewrap + rsync from inside the distro);
     no further host setup needed.
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
- Anti-patterns to avoid
- Initialization system
- Error handling
- Loading states

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
   - File location: `/home/pbya/projects/ziee-chat/src-app/server/tests/.env.test`

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

- **Reference Project:** `/home/pbya/projects/ziee-chat-ref`
- **Active Project:** `/home/pbya/projects/ziee-chat`
- **Backend:** `/home/pbya/projects/ziee-chat/src-app/server`
- **Frontend:** `/home/pbya/projects/ziee-chat/src-app/ui`

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
