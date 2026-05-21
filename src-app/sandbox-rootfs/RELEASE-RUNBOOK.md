# Sandbox rootfs release runbook

This runbook covers (a) the **bootstrap** — cutting the first release tag
so the nightly CI workflow has artifacts to fetch — and (b) the **ongoing
release flow** for revision bumps and schema changes.

## Bootstrap (one-time)

```bash
./scripts/bootstrap-first-rootfs-release.sh
```

What it does:

1. Builds both `minimal` and `full` flavors via
   `cargo run --bin ziee-chat -- build-sandbox-rootfs --flavor <flavor>`.
2. Computes sha256 for each artifact.
3. Cosign-signs each artifact (skip with a warning if `cosign` isn't
   installed).
4. Creates a GitHub release tagged `sandbox-rootfs-v{schema}.{revision}-{arch}`
   (defaults: `v1.r0-x86_64`).
5. Updates `src-app/server/src/modules/code_sandbox/known_revisions.toml`
   with the (schema, revision, arch, flavor, sha256) tuples. The server
   binary embeds this file via `include_str!`; `fetch-sandbox-rootfs`
   verifies downloads against it before mounting.

After the script finishes, **commit the updated `known_revisions.toml`**
and push. The change is what lets `fetch-sandbox-rootfs --version=latest`
work for end users.

### Override defaults

```bash
SCHEMA=1 REVISION=r1 ARCH=x86_64 \
  ./scripts/bootstrap-first-rootfs-release.sh
```

Use `--dry-run` to see what would happen without actually creating the
release or modifying `known_revisions.toml`.

## Ongoing releases

Subsequent releases are automated. The flow:

1. **Patch:** Pin updates / security backports stay on the same schema
   but bump the revision (e.g. `v1.r0` → `v1.r1`). Edit
   `src-app/sandbox-rootfs/pins/apt-snapshot` (or pip/R/npm pins) and
   commit. The `sandbox-rootfs-pr.yml` workflow builds + tests the
   change.
2. **Tag:** When ready to publish, push a tag matching
   `sandbox-rootfs-v*`:
   ```bash
   git tag sandbox-rootfs-v1.r1-x86_64
   git push origin sandbox-rootfs-v1.r1-x86_64
   ```
3. **CI publishes:** `sandbox-rootfs-release.yml` builds the rootfs,
   reproducibility-checks it (build twice, diff sha256), cosign-signs,
   and uploads to the GitHub release.
4. **Auto-PR:** The same workflow opens a PR against `main` updating
   `known_revisions.toml` with the new (schema, revision, sha256)
   tuple. Reviewer merges to make the new revision available to
   `fetch-sandbox-rootfs`.
5. **Nightly:** The next `sandbox-integration-nightly.yml` run fetches
   the new version and runs the full Tier-4 + Tier-6 suite against
   the main-branch server, opening a `sandbox-drift` issue on failure.

## Schema bumps

A schema bump is an **ABI-breaking** rootfs change (e.g. Python major
version bump, layout change, binary path move). Schema bumps require:

1. Bump the const in two places:
   - `src-app/server/src/modules/code_sandbox/mod.rs`:
     `SANDBOX_ROOTFS_SCHEMA_VERSION`
   - `src-app/sandbox-rootfs/compat.toml`: `current_schema`
2. Update `compat.toml`'s `[[schemas]]` table with the new entry's
   `ziee_server_min` / `ziee_server_max` range.
3. Cut a new release with the new schema (`SCHEMA=2 REVISION=r0
   ./scripts/bootstrap-first-rootfs-release.sh`).
4. Existing servers boot-probe their installed rootfs's
   `.ziee-sandbox-rootfs-schema` sentinel against the binary's
   `SANDBOX_ROOTFS_SCHEMA_VERSION`. A mismatch refuses to register the
   sandbox MCP row and logs the upgrade command.

## Yanks

If a published revision turns out to be broken (security CVE, broken
package), add it to `src-app/sandbox-rootfs/yanks.toml` and commit.
`fetch-sandbox-rootfs --version=latest` then skips yanked revisions
when resolving "latest". Operators who pinned an exact revision continue
working — yanks don't delete artifacts.

```toml
# src-app/sandbox-rootfs/yanks.toml
[[yanked]]
schema = 1
revision = "r2"
reason = "CVE-2026-XXXX in openssl"
yanked_at = "2026-06-15"
```

## Troubleshooting

### `gh release create` fails with "tag already exists"

The release was previously cut (or partially). Delete and re-run:

```bash
gh release delete sandbox-rootfs-v1.r0-x86_64 --yes --cleanup-tag
./scripts/bootstrap-first-rootfs-release.sh
```

### `cosign sign-blob` fails

Cosign needs network access to talk to the Sigstore Fulcio +
Rekor services. Confirm `cosign verify-blob` works against a known
good artifact first. As a fallback, omit cosign (the script warns
and proceeds without a `.cosign.bundle`). Operators who want sig
verification can install the bundle later.

### mmdebstrap fails with "/etc/subuid is empty: invalid idmap"

Without a subuid range, mmdebstrap can't operate in unprivileged
mode. `build.sh` auto-detects this and switches to `--mode=root`
via sudo. Confirm passwordless sudo works:

```bash
sudo -n true && echo "sudo ok" || echo "configure passwordless sudo first"
```

### The bootstrap script reports cosign missing but proceeds

That's intentional — cosign is a nice-to-have, not a blocker. The
release will exist but lack the `.cosign.bundle` file, which means
`fetch-sandbox-rootfs` will fall back to sha256-only verification.
Install cosign and re-run later to add signatures:

```bash
cosign sign-blob --bundle <bundle.cosign.bundle> <squashfs>
gh release upload sandbox-rootfs-v1.r0-x86_64 <bundle.cosign.bundle>
```

## Verification

After a release, verify the contract end-to-end:

```bash
# Operator path
mkdir -p /tmp/rootfs-verify
gh release download sandbox-rootfs-v1.r0-x86_64 \
    --pattern '*-x86_64-minimal.squashfs' \
    --dir /tmp/rootfs-verify
gh release download sandbox-rootfs-v1.r0-x86_64 \
    --pattern '*-x86_64-minimal.squashfs.sha256' \
    --dir /tmp/rootfs-verify
( cd /tmp/rootfs-verify && sha256sum -c *.sha256 )

# Server path (after the auto-PR merge)
ziee-chat fetch-sandbox-rootfs --version=latest --flavor=minimal
ziee-chat mount-sandbox-rootfs
# Boot the server; the hardening log should show pid_ns/cgroup/seccomp.
```
