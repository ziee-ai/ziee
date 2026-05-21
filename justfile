# ziee-chat dev workflows.
#
# Most sandbox targets are thin wrappers around the equivalent
# `ziee-chat <subcommand>` CLI in src-app/server/src/main.rs. Run the
# CLI directly if you prefer (e.g. `cargo run --bin ziee-chat --
# build-sandbox-rootfs --flavor minimal`).

# default: list targets
default:
    @just --list

# ─── Sandbox rootfs ──────────────────────────────────────────────────

# Build the sandbox rootfs locally (10-15 min first time).
# Flavors: minimal | full
sandbox-build flavor="full":
    cd src-app/server && cargo run -q --bin ziee-chat -- build-sandbox-rootfs --flavor {{flavor}}

# Mount a built squashfs for the TEST HARNESS only.
#
# Production servers auto-mount the rootfs lazily on the first
# `execute_command` call (see modules/code_sandbox/runtime_mount.rs)
# — there is no `ziee-chat mount-sandbox-rootfs` CLI command. But
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

# Fetch a published rootfs from GitHub Releases (v1 stub: prints
# instructions; v2 will perform real sha256 + cosign verification).
sandbox-fetch version="latest" flavor="minimal":
    cd src-app/server && cargo run -q --bin ziee-chat -- fetch-sandbox-rootfs \
        --version={{version}} --flavor={{flavor}}

# Remove cached rootfs versions, keeping the N most recent.
sandbox-gc keep="2":
    cd src-app/server && cargo run -q --bin ziee-chat -- gc-sandbox-rootfs --keep={{keep}}

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

# Run the full backend test suite (no bwrap-ignored).
test:
    cd src-app/server && \
        bash -c 'source tests/.env.test && cargo test --test integration_tests -- --test-threads=1'

# ─── UI ─────────────────────────────────────────────────────────────

# Run the frontend dev server.
ui:
    cd src-app/ui && npm run dev
