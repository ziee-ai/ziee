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

# Mount a built squashfs and flip the `current` symlink atomically.
# Idempotent: re-running with the same version is a no-op.
sandbox-mount rootfs="":
    cd src-app/server && cargo run -q --bin ziee-chat -- mount-sandbox-rootfs \
        {{ if rootfs == "" { "" } else { "--rootfs=" + rootfs } }}

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
