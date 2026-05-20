# ziee-chat dev workflows
#
# Most targets are thin wrappers around `src-app/sandbox-rootfs/`
# scripts so backend devs don't have to remember the exact flags.

# default: list targets
default:
    @just --list

# ─── Sandbox rootfs ──────────────────────────────────────────────────

# Build the sandbox rootfs locally (10-15 min first time).
# Flavors: minimal | full
sandbox-build flavor="full":
    src-app/sandbox-rootfs/build.sh --flavor {{flavor}}

# Mount a built squashfs and flip the `current` symlink atomically.
# Idempotent: re-running with the same version is a no-op.
sandbox-mount version="latest":
    @bash -euo pipefail -c '\
        CACHE=.ziee-cache/sandbox-rootfs; \
        mkdir -p "$CACHE"; \
        if [ "{{version}}" = "latest" ]; then \
            sqfs=$(ls -t "$CACHE"/ziee-sandbox-rootfs-*.squashfs 2>/dev/null | head -1); \
        else \
            sqfs="$CACHE/ziee-sandbox-rootfs-{{version}}.squashfs"; \
        fi; \
        if [ -z "$sqfs" ] || [ ! -f "$sqfs" ]; then \
            echo "no squashfs found; run \`just sandbox-build\` or \`just sandbox-fetch\` first" >&2; exit 1; \
        fi; \
        name=$(basename "$sqfs" .squashfs); \
        mnt="$CACHE/$name"; \
        mkdir -p "$mnt"; \
        if ! mountpoint -q "$mnt"; then \
            squashfuse "$sqfs" "$mnt"; \
            echo "mounted $sqfs at $mnt"; \
        else \
            echo "already mounted: $mnt"; \
        fi; \
        ln -sfn "$name" "$CACHE/current"; \
        echo "current → $name"'

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

# Fetch latest published rootfs from GitHub Releases.
# Stub for v1: documents the URL pattern. v2 wires this into a real
# `ziee-server fetch-sandbox-rootfs` subcommand with sha256 + cosign
# verification.
sandbox-fetch version="latest" flavor="minimal" arch="x86_64":
    @bash -euo pipefail -c '\
        echo "(v1 stub) Once releases exist, fetch with:" ; \
        echo "  gh release download sandbox-rootfs-v1.r0-{{arch}} \\"; \
        echo "    --pattern \"ziee-sandbox-rootfs-v1.r0-{{arch}}-{{flavor}}.squashfs*\" \\"; \
        echo "    --dir .ziee-cache/sandbox-rootfs/"; \
        echo "  just sandbox-mount v1.r0-{{arch}}-{{flavor}}"; \
        echo ""; \
        echo "For now, build locally:"; \
        echo "  just sandbox-build {{flavor}}"; \
        echo "  just sandbox-mount"'

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
