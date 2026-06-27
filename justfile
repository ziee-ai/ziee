# ziee dev workflows.
#
# Sandbox rootfs is built by src-app/sandbox-rootfs/build.sh (maintainer/CI
# task). Rootfs cache management (list / evict) is in the admin UI, not a CLI.

# default: list targets
default:
    @just --list

# ─── Sandbox rootfs ──────────────────────────────────────────────────

# Build the sandbox rootfs locally (10-15 min first time).
# Flavors: minimal | full
sandbox-build flavor="full":
    src-app/sandbox-rootfs/build.sh --flavor {{flavor}}

# Mount a built squashfs for the TEST HARNESS only.
#
# Production servers auto-mount the rootfs lazily on the first
# `execute_command` call (see modules/code_sandbox/runtime_mount.rs)
# — there is no `ziee mount-sandbox-rootfs` CLI command. But
# `just check-sandbox` runs Tier-4/6 tests that exercise bwrap
# directly against a mounted rootfs, bypassing the server. This
# recipe is for that case.
#
# Idempotent: re-running against the same squashfs is a no-op.
sandbox-mount:
    @bash -euo pipefail -c '\
        sqfs=$(ls -t .ziee-cache/sandbox-rootfs/*.squashfs 2>/dev/null | head -1); \
        if [ -z "$sqfs" ]; then \
            echo "no squashfs found; run \`just sandbox-build\` first" >&2; \
            exit 1; \
        fi; \
        mnt="${sqfs%.squashfs}"; \
        mkdir -p "$mnt"; \
        mountpoint -q "$mnt" || squashfuse "$sqfs" "$mnt"; \
        ln -sfn "$(basename "$mnt")" .ziee-cache/sandbox-rootfs/current; \
        echo "mounted: $mnt"'

# Tear down dev sandbox state (unmount + rm). Safe to run anytime.
sandbox-clean:
    @bash -euo pipefail -c '\
        CACHE=.ziee-cache/sandbox-rootfs; \
        if [ -d "$CACHE" ]; then \
            for d in "$CACHE"/*/; do \
                [ -d "$d" ] || continue; \
                fusermount -u "$d" 2>/dev/null || true; \
            done; \
            rm -rf "$CACHE"; \
            echo "cleaned $CACHE"; \
        fi'

# NOTE: there is no manual "fetch" recipe — the server auto-fetches the
# matching rootfs from GitHub Releases (sha256 + sigstore verify) on the
# first `execute_command`. Just boot the server with code_sandbox.enabled.

# Cache management (list / evict cached rootfs) is in the admin UI:
# Settings → Sandbox Environments. There is no gc CLI.

# Run the bwrap-dependent integration tests.
# Requires: bwrap + rootfs mounted at .ziee-cache/sandbox-rootfs/current.
sandbox-test:
    cd src-app/server && \
        cargo test --test integration_tests -- --test-threads=1 --ignored code_sandbox::

# ─── Pre-push checks ────────────────────────────────────────────────
# CI is build-and-publish-only. The maintainer's responsibility is to
# run these BEFORE pushing or tagging. `just check` is the equivalent
# of what a PR-time CI job WOULD run if we had one.

# Run everything before pushing changes that touch the sandbox.
# Skips bwrap tests if no rootfs is mounted (prints a hint).
check: check-schema-sync check-sandbox-unit
    @echo "✓ pre-push checks passed (cheap layer)"
    @echo
    @echo "Run \`just check-sandbox\` next if you've mounted a rootfs"
    @echo "(builds + runs Tier 4 + 6 — takes ~1 min)."

# Catches the case where `compat.toml::current_schema` and
# `SANDBOX_ROOTFS_SCHEMA_VERSION` in mod.rs drift apart. A mismatch
# breaks every operator's boot probe — cheap to check, expensive to
# debug after the fact.
check-schema-sync:
    @bash -c 'set -eu; \
        toml=$(grep -E "^current_schema" src-app/sandbox-rootfs/compat.toml | grep -oE "[0-9]+"); \
        src=$(grep "pub const SANDBOX_ROOTFS_SCHEMA_VERSION" src-app/server/src/modules/code_sandbox/mod.rs | grep -oE "[0-9]+"); \
        if [ "$toml" != "$src" ]; then \
            echo "FAIL: compat.toml current_schema=$toml but mod.rs SANDBOX_ROOTFS_SCHEMA_VERSION=$src" >&2; \
            echo "      Bump both or neither." >&2; \
            exit 1; \
        fi; \
        echo "✓ schema version sync: $toml"'

# Tier 1 + 2 + 3 — no rootfs needed (~30 s).
check-sandbox-unit:
    cd src-app/server && cargo test --lib code_sandbox::
    cd src-app/server && \
        cargo test --test integration_tests -- --test-threads=1 code_sandbox::

# Tier 4 + 6 — needs `just sandbox-mount` first (~1 min).
check-sandbox:
    @bash -c 'set -eu; \
        if [ ! -d .ziee-cache/sandbox-rootfs/current/usr ]; then \
            echo "no rootfs mounted. Run \`just sandbox-mount\` first." >&2; \
            exit 1; \
        fi'
    cd src-app/server && \
        ZIEE_SANDBOX_ROOTFS=$(pwd)/../../.ziee-cache/sandbox-rootfs/current \
        cargo test --test integration_tests -- --test-threads=1 --ignored \
            code_sandbox::tier4_ code_sandbox::tier6_

# Tier 5 — real LLM via Anthropic. Costs ~$0.30 per run. Sources
# the API key from tests/.env.test.
check-sandbox-llm:
    cd src-app/server && \
        bash -c 'source tests/.env.test && \
        ZIEE_SANDBOX_ROOTFS=$(pwd)/../../.ziee-cache/sandbox-rootfs/current \
        cargo test --test integration_tests -- --test-threads=1 --ignored \
            chat::sandbox_real_llm'

# Run-everything before tagging a sandbox-rootfs-v* release. Includes
# the rootfs build + reproducibility check. Takes ~15 min.
check-release-ready: check check-rootfs-reproducibility
    @echo "✓ release-ready"

# Stand up a local http "registry" + run the full fetch+verify+install
# path against it. Validates the operator install flow WITHOUT cutting
# a real release tag. Sets signed=false so cosign is skipped; real
# keyless cosign needs an actual GitHub Actions run.
#
# Requires: bwrap, squashfuse, python3.
dev-release flavor="minimal" port="8765":
    scripts/dev-release.sh {{flavor}} {{port}}

check-rootfs-reproducibility:
    @bash -c 'set -eu; \
        echo "==> Building rootfs twice and comparing sha256"; \
        src-app/sandbox-rootfs/build.sh --flavor minimal; \
        sqfs1=$(ls -t .ziee-cache/sandbox-rootfs/*minimal*.squashfs | head -1); \
        sha1=$(sha256sum "$sqfs1" | cut -d" " -f1); \
        rm "$sqfs1"; \
        src-app/sandbox-rootfs/build.sh --flavor minimal; \
        sqfs2=$(ls -t .ziee-cache/sandbox-rootfs/*minimal*.squashfs | head -1); \
        sha2=$(sha256sum "$sqfs2" | cut -d" " -f1); \
        if [ "$sha1" != "$sha2" ]; then \
            echo "FAIL: reproducibility check failed" >&2; \
            echo "  first:  $sha1" >&2; \
            echo "  second: $sha2" >&2; \
            exit 1; \
        fi; \
        echo "✓ reproducible build: $sha1"'

# ─── Server ──────────────────────────────────────────────────────────

# Run the backend server (dev config).
server:
    cd src-app/server && CONFIG_FILE=config/dev.yaml cargo run

# Run the full backend test suite, including tier-4 (45 tests that
# dispatch through SandboxBackend::exec_raw_argv on all platforms).
# Tier-4/6 fetch the real published rootfs from the
# `ziee-ai/sandbox-rootfs` GitHub release on demand (cached under
# `.ziee-cache/sandbox-rootfs`), so there is no local build step — just
# network access on the first run. Pin a different release via
# `ZIEE_SANDBOX_TEST_TAG`.
test:
    cd src-app/server && \
        bash -c 'source tests/.env.test && cargo test --test integration_tests -- --test-threads=1'

# Same as `test` but also runs tier-6 e2e + tier-5 LLM tests (tier-5 is
# `#[ignore]` — needs ANTHROPIC_API_KEY and costs ~$0.30/run). Tier-6
# boots a real server that fetches + cosign-verifies + mounts the rootfs
# from GitHub. Use this in CI / pre-release validation, not routine dev
# cycles.
test-all:
    cd src-app/server && \
        bash -c 'source tests/.env.test && cargo test --test integration_tests -- --test-threads=1 --include-ignored'

# ─── UI ─────────────────────────────────────────────────────────────

# Run the frontend dev server.
ui:
    cd src-app/ui && npm run dev

# Run @ant-design/cli diagnostics (doctor + lint + usage) against the UI.
# Outputs in src-app/ui/docs/antd-diagnostics/<date>/. See FRONTEND_DEPS.md.
antd-check:
    cd src-app/ui && ./scripts/antd-diagnose.sh

# ─── Desktop (Tauri) app ────────────────────────────────────────────

# Run the desktop app in dev mode (Tauri window + hot-reload Vite).
desktop-dev:
    cd src-app/desktop/tauri && npx tauri dev

# Build a release bundle (.dmg on macOS, .deb/.AppImage on Linux,
# .msi on Windows). Output: src-app/desktop/tauri/target/release/bundle/.
desktop-build:
    cd src-app/desktop/tauri && npx tauri build

# Same but unsigned + debug profile, for faster iteration.
desktop-build-debug:
    cd src-app/desktop/tauri && npx tauri build --debug

# Install npm deps for both UI workspaces in one shot. With npm
# workspaces (root /package.json), `npm install` hoists shared deps
# to the repo's root node_modules and dedupes across workspaces.
desktop-deps:
    npm install

# Check for dependency drift between core UI and desktop UI
# package.json files. Exits non-zero if any shared dep version
# differs (with the exception of intentionally split deps configured
# in /.syncpackrc.json). Wire into CI to catch drift early.
sync-check:
    npx --no-install syncpack lint

# Auto-fix mismatched dependency versions across workspaces by
# rewriting the lower-version package.json to match the higher.
# Run sync-check after to confirm clean.
sync-fix:
    npx --no-install syncpack fix-mismatches

# Pin sqlx to the 0.8.x line in the workspace's `src-app/Cargo.lock`.
# pgvector 0.4 accepts sqlx `>= 0.8, < 0.10` and cargo's resolver picks
# 0.9 for it independently of the rest of the tree (which is pinned to
# 0.8.x by ziee's `sqlx = "0.8.6"` + macros feature). Without this pin,
# the two sqlx versions coexist and `HalfVector: sqlx::Type<Postgres>`
# trait impls (which live in 0.9) don't apply to ziee's 0.8 query
# builders → ~12 compile errors in the server lib.
#
# Run after every `cargo update`. No-op if already pinned.
workspace-cargo-pin-sqlx:
    @bash -c 'cd src-app && \
        if cargo tree -i sqlx@0.9.0 2>/dev/null | grep -q pgvector; then \
            echo "==> pinning pgvector→sqlx onto 0.8.6"; \
            cargo update -p sqlx@0.9.0 --precise 0.8.6; \
        else \
            echo "==> sqlx already pinned to 0.8.x in lock"; \
        fi'

# Run desktop tests across all three layers (L1 + L2 + L3 + the
# legacy Playwright suite). L2 and L3 are heavy and gated on built
# artifacts — invoke them individually if you don't have a fresh
# bundle.
#
# Layer breakdown:
#   L1 — Tauri command unit tests via tauri::test::mock_builder
#        (`tests/tauri_commands_test.rs`). Fast, no external deps.
#   L2 — Spawn-binary smoke that boots the production binary against
#        an isolated tempdir, hits /api/health, then SIGTERMs
#        (`tests/spawn_binary_smoke.rs`, `#[ignore]`). Needs the
#        release binary built. Real embedded-PG bringup ~30s.
#   L3 — WebDriver E2E against the fully-bundled .app via
#        tauri-driver (`desktop/ui/tests/tauri-driver/smoke.mjs`).
#        Needs `cargo install tauri-driver` + `safaridriver --enable`
#        on macOS. See that dir's README.md for setup.
desktop-test: desktop-test-rust desktop-test-e2e

desktop-test-rust: workspace-cargo-pin-sqlx
    cd src-app/desktop/tauri && cargo test

desktop-test-e2e:
    cd src-app/desktop/ui && npm run test:e2e

# L1 only — fast iteration on Tauri command tests.
desktop-test-l1: workspace-cargo-pin-sqlx
    cd src-app/desktop/tauri && cargo test --test tauri_commands_test

# L2 — spawn-binary smoke. Builds release binary if missing, then
# runs the ignored test.
desktop-test-l2: workspace-cargo-pin-sqlx
    cd src-app/desktop/tauri && cargo build --release --bin ziee-desktop
    cd src-app/desktop/tauri && cargo test --test spawn_binary_smoke -- --ignored --test-threads=1

# L3 — tauri-driver WebDriver smoke against the bundled .app.
# Preconditions enforced by the script itself; see
# desktop/ui/tests/tauri-driver/README.md for the install steps.
desktop-test-l3:
    cd src-app/desktop/ui && npm run test:tauri-driver

# Scan every desktop override file under src-app/desktop/ui/src/modules/
# and report whether it matches its core equivalent. Output prefixes:
#   =  matches core verbatim (override could be deleted if no longer needed)
#   ≠  diverges from core (review intent — DELIBERATE DIVERGENCE header
#      means it's intentional; otherwise it's drift to re-sync)
#   +  desktop-only (no core equivalent — leave alone)
#
# Skips desktop-only module roots whose existence is the point.
desktop-drift-check:
    @bash -c 'set -e; \
        cd src-app/desktop/ui/src; \
        find modules -type f \( -name "*.tsx" -o -name "*.ts" \) \
            | grep -vE "modules/(desktop-base|window|file-dialog|memory|llm-providers|desktop-loader)" \
            | sort \
            | while read f; do \
                core="../../../ui/src/$f"; \
                if [ -f "$core" ]; then \
                    if diff -q "$f" "$core" >/dev/null 2>&1; then \
                        echo "= $f"; \
                    elif grep -q "DELIBERATE DIVERGENCE" "$f"; then \
                        echo "≠ $f  (intentional — has DELIBERATE DIVERGENCE marker)"; \
                    else \
                        echo "≠ $f  (drifted — re-sync from $core)"; \
                    fi; \
                else \
                    echo "+ $f  (desktop-only)"; \
                fi; \
            done'

# ─── Self-contained release builds ──────────────────────────────────
#
# Mac: produces target/aarch64-apple-darwin/release/ziee — a single
# binary that embeds the sandbox launcher + libkrun dylibs + guest root
# (see build_helper/sandbox_runtime.rs). Requires Docker (for the
# cross-compiled guest agent + Alpine guest root) and brew with the
# pinned libkrun/libkrunfw/libepoxy/virglrenderer/molten-vk versions.
build-mac:
    cd src-app/server && cargo build --release --target aarch64-apple-darwin

# Linux: produces target/{x86_64,aarch64}-unknown-linux-musl/release/ziee
# — a static-musl binary. Operator's runtime prereqs: `apt install
# bubblewrap squashfuse fuse3`. LIBSECCOMP_LIB_PATH override matches
# Alpine's musl layout (the cargo-zigbuild image is Alpine-based).
build-linux arch="x86_64":
    cd src-app/server && \
        LIBSECCOMP_LIB_PATH=/usr/lib \
        cargo zigbuild --release --target {{arch}}-unknown-linux-musl

# Static-analysis "self-contained" checks (no /opt/homebrew refs,
# musl-only ldd, no native-tls in dep graph). Run after each build.
verify-mac:
    cd src-app/server && cargo test --release --target aarch64-apple-darwin --test macos_self_contained -- --nocapture
verify-linux arch="x86_64":
    cd src-app/server && cargo test --release --target {{arch}}-unknown-linux-musl --test linux_self_contained -- --nocapture

# Dynamic "self-contained" checks. Mac boots a libkrun VM with brew
# poisoned (DYLD_*=/nonexistent). Linux runs the binary inside
# `gcr.io/distroless/static` and `alpine + sandbox prereqs`.
test-mac:
    cd src-app/server && cargo test --release --target aarch64-apple-darwin --test macos_brewless_boot -- --ignored --nocapture
test-linux arch="x86_64":
    cd src-app/server && cargo test --release --target {{arch}}-unknown-linux-musl --test linux_distroless_boot -- --ignored --nocapture

# ─── Remote Access (Feature: ngrok tunnel exposure) ─────────────────
#
# Tiered test recipes for the Remote Access module. Tier 1+2 are the
# default gate; Tier 5 (bundle inclusion) is fast and high-signal;
# Tier 6 is the Playwright E2E; Tier 8 needs real ngrok credentials.

# Tier 1 (unit) + Tier 2 (integration). Default gate; ~30 s.
# Unit tests live in-source under the desktop crate's modules;
# integration tests live in `desktop/tauri/tests/remote_access/` and
# share the TestServer harness from the server crate via #[path].
check-remote-access-unit:
    cd src-app/desktop/tauri && cargo test --lib -p ziee-desktop remote_access:: magic_link:: tunnel_auth::
    cd src-app/desktop/tauri && \
        bash -c 'source ../../server/tests/.env.test 2>/dev/null || true; \
            cargo test --test integration_tests -- --test-threads=1 remote_access::'

# Tier 6 — Playwright E2E for the Remote Access desktop page. Both
# the admin settings page AND the phone-side magic-link consumer
# live in the desktop bundle now (single-bundle architecture); the
# spec mocks ngrok so no network is needed.
check-remote-access-e2e:
    cd src-app/desktop/ui && npx playwright test remote-access.spec.ts

# Tier 8 — real ngrok integration. Reads NGROK_AUTH_TOKEN (and
# optionally NGROK_TEST_DOMAIN) from .ngrok-test-credentials.env.
# Opens a real tunnel, HTTPs through it, then stops. Costs ~2 min
# of an ngrok session.
check-remote-access-real-ngrok:
    @bash -c 'set -euo pipefail; \
        if [ -f .ngrok-test-credentials.env ]; then \
            source .ngrok-test-credentials.env; \
        fi; \
        if [ -z "${NGROK_AUTH_TOKEN:-}" ]; then \
            echo "NGROK_AUTH_TOKEN not set; create .ngrok-test-credentials.env or export it" >&2; \
            exit 1; \
        fi; \
        cd src-app/desktop/tauri && \
            source ../../server/tests/.env.test 2>/dev/null || true; \
            cargo test --test integration_tests -- --test-threads=1 --ignored \
                remote_access::real_ngrok'

# All Remote Access tests in one shot (skips the real-ngrok tier
# unless credentials are present).
check-remote-access-all: check-remote-access-unit check-remote-access-e2e
    @echo "✓ remote-access: unit + integration + e2e all green"

# ─── Hub Registry (Feature: hub v2 install-from-hub flows) ──────────
#
# Recipes for the `feat/hub-registry` work. All recipes hard-fail if
# their prerequisites (Postgres on 54321, tests/.env.test, etc.) are
# missing — no interactive prompts. Names are `hub-`-prefixed (or end
# in `-hub`) to avoid collision with the existing sandbox recipes
# (`check`, `test`).

# Postgres connection string for the dedicated hub build DB. Kept
# separate from the docker-compose-managed `postgres` DB so sqlx's
# in-process migrations (run by build.rs) can wipe + recreate schema
# without touching the other crates' build DB.
HUB_DB_URL := "postgresql://postgres:password@127.0.0.1:54321/hubreg_build"

# Verify the dedicated build DB exists; create it if not. Hard-fails
# if Postgres on 127.0.0.1:54321 isn't reachable.
ensure-build-db:
    @bash -c 'set -euo pipefail; \
        if ! PGPASSWORD=password psql -h 127.0.0.1 -p 54321 -U postgres -tAc "SELECT 1" >/dev/null 2>&1; then \
            echo "ERROR: Postgres not reachable on 127.0.0.1:54321. Start docker-compose first:" >&2; \
            echo "  cd src-app && docker compose up -d" >&2; \
            exit 1; \
        fi; \
        if ! PGPASSWORD=password psql -h 127.0.0.1 -p 54321 -U postgres -tAc \
                "SELECT 1 FROM pg_database WHERE datname='\''hubreg_build'\''" | grep -q 1; then \
            echo "==> creating hubreg_build database"; \
            PGPASSWORD=password psql -h 127.0.0.1 -p 54321 -U postgres -c "CREATE DATABASE hubreg_build;" >/dev/null; \
        fi; \
        echo "✓ build DB ready: hubreg_build"'

# Compile the server workspace + tests against the isolated build DB.
# Build.rs wipes + re-runs migrations on every cargo build, so this
# also validates that all migrations apply cleanly.
check-hub: ensure-build-db
    @cd src-app && DATABASE_URL="{{HUB_DB_URL}}" cargo check -p ziee --all-targets

# Run `tsc --noEmit` against both UI workspaces (core + desktop).
# Hard-fails on the first error; second workspace runs only if the
# first compiled. Caller can run `npm install` from the repo root if
# the workspace's node_modules are missing.
tsc:
    @cd src-app/ui && npx tsc --noEmit
    @cd src-app/desktop/ui && npx tsc --noEmit

# Run the FULL hub-related integration test suite end-to-end. Saves
# the log per CLAUDE.md memory ("ALWAYS Save Full Test Logs").
# Hard-fails (exit non-zero) if tests/.env.test is missing — the test
# harness reads HUGGINGFACE_API_KEY from it.
test-hub: ensure-build-db
    #!/usr/bin/env bash
    set -euo pipefail
    cd src-app/server
    if [ ! -f tests/.env.test ]; then
        echo "ERROR: src-app/server/tests/.env.test is missing." >&2
        echo "       Copy from tests/.env.test.example and fill in real values:" >&2
        echo "         cp tests/.env.test.example tests/.env.test" >&2
        echo "       (HUGGINGFACE_API_KEY is required for the model download tests)." >&2
        exit 1
    fi
    log="hub-strict-int-$(date +%Y%m%d-%H%M%S).log"
    echo "Writing log to src-app/server/$log"
    source tests/.env.test
    # cargo test only takes one positional filter — pass the OR-set
    # via `--` so the runner does the matching.
    DATABASE_URL="{{HUB_DB_URL}}" \
        cargo test --test integration_tests -- \
            --test-threads=1 \
            hub:: assistant:: mcp:: llm_model:: \
            2>&1 | tee "$log"

# OpenAPI two-step regen: the SERVER binary emits the core UI spec, and the
# DESKTOP binary emits the desktop UI spec (server routes + the desktop-only
# routes — remote_access / magic_link / tunnel_auth / host_mount). Each binary
# now ALSO emits its `src/api-client/types.ts` directly (Rust port of the former
# `generate-endpoints.ts`; see `server/src/openapi/emit_ts.rs`), so there is no
# longer a separate Node/tsx codegen step. The whole set is the single
# source-of-truth flow for API types.
#
# NOTE: do NOT `cp` the server spec onto the desktop one — that drops every
# desktop-only route and leaves the desktop client (and `tsc`) missing them.
openapi-regen: check-hub
    @cd src-app && DATABASE_URL="{{HUB_DB_URL}}" CONFIG_FILE=server/config/openapi-gen.yaml \
        cargo run --bin ziee -- --generate-openapi ui/openapi
    @cd src-app && DATABASE_URL="{{HUB_DB_URL}}" CONFIG_FILE=server/config/openapi-gen.yaml \
        cargo run -p ziee-desktop -- --generate-openapi desktop/ui/openapi

# The "all green" hub compile gate — check + tsc + openapi-regen, but
# NOT test-hub (slow, needs creds, separate recipe).
ci-hub: check-hub tsc openapi-regen
    @echo "✓ hub: compile + tsc + openapi-regen all green"
# ─── Desktop auto-updater ────────────────────────────────────────────

# Always-on updater tests (no Docker needed):
#   Tier 1 — frontend store unit (vitest)
#   Tier 2 — manifest builder unit (node --test)
#   Tier 3 — signing round-trip (Rust; auto-generates an ephemeral key,
#            signs via `tauri signer`, verifies with minisign-verify,
#            asserts a tampered artifact fails). Needs the build DB on
#            :54321 to compile (same as every desktop cargo recipe).
check-updater: workspace-cargo-pin-sqlx
    node --test scripts/updater/build-latest-json.test.mjs
    cd src-app/desktop/ui && npm run test:unit -- src/modules/updater/stores/Updater.store.test.ts
    cd src-app/desktop/tauri && cargo test --test updater_signing_test -- --test-threads=1
    @echo "✓ updater: Tier 1 (store) + Tier 2 (manifest) + Tier 3 (signing) green"

# Tier 4 — release/Pages workflow exercised locally with `act` (Docker)
# against a temp bare repo, plus a dockerized actionlint over both
# workflows. Self-asserting: the workflow fails if the published
# latest.json is wrong, so act's exit code is the signal. Needs Docker
# + act (external deps; own recipe like the sandbox external-dep tiers).
check-updater-ci:
    @echo "==> actionlint (dockerized) over updater workflows"
    docker run --rm -v "{{justfile_directory()}}":/repo --workdir /repo \
        rhysd/actionlint:latest -color \
        .github/workflows/desktop-release.yml \
        .github/workflows/desktop-updater-pages-test.yml
    @echo "==> act: run desktop-updater-pages-test.yml (publishes to a temp bare repo, self-asserts)"
    act workflow_dispatch \
        -W .github/workflows/desktop-updater-pages-test.yml \
        --bind --rm
    @echo "✓ updater Tier 4: workflow + manifest + gh-pages publish verified via act"

# Everything testable locally for the updater (Tiers 1-3 + 4).
check-updater-all: check-updater check-updater-ci
    @echo "✓ updater: all locally-runnable tiers green"

# ─── Server distribution + update notification ───────────────────────

# Always-on server-update tests:
#   - backend unit (semver / config default / release-JSON parse)
#   - install.sh shellcheck + --dry-run URL/method/arch asserts; the install
#     test's Part B additionally checks distro detection inside ubuntu/fedora/
#     alpine containers when Docker is up.
# Needs the build DB on :54321 to compile the server (like every cargo recipe).
check-server-update: workspace-cargo-pin-sqlx
    cd src-app && cargo test -p ziee --lib server_update::
    docker run --rm -v "{{justfile_directory()}}":/mnt -w /mnt koalaman/shellcheck:stable \
        scripts/install.sh scripts/install.test.sh packaging/postinstall.sh packaging/preremove.sh
    sh scripts/install.test.sh
    @echo "✓ server-update: backend unit + install.sh (shellcheck + dry-run + distro) green"

# Server-update HTTP integration test: mock GitHub via SERVER_UPDATE_API_MIRROR,
# assert auth-gate + the update-available path through /api/server-update/status.
# Needs the build/test DB on :54321.
check-server-update-int:
    cd src-app/server && cargo test --test integration_tests server_update:: -- --test-threads=1
    @echo "✓ server-update integration green"

# Lint the server release workflow (dockerized actionlint).
check-server-release-ci:
    docker run --rm -v "{{justfile_directory()}}":/repo --workdir /repo \
        rhysd/actionlint:latest -color .github/workflows/server-release.yml
    @echo "✓ server-release.yml lint clean"

