# sandbox-rootfs

Build inputs for the **ziee code sandbox rootfs** — the Ubuntu-based
filesystem mounted inside `bwrap` for code execution.

The rootfs is shipped as a `squashfs` (zstd-19) binary artifact via
GitHub Releases on the `phibya/ziee-chat` repo. Sources live here.

## Quick start (dev)

From the repo root:

```bash
# Build the full (~1.6 GB compressed) flavor
just sandbox-build full

# Or the minimal (~150 MB) flavor for fast iteration on bwrap/cgroup/
# seccomp mechanics that don't need numpy/torch:
just sandbox-build minimal

# Mount + flip the `current` symlink
just sandbox-mount

# Run the bwrap-dependent tests
just sandbox-test
```

You need: `bubblewrap`, `squashfuse`, `squashfs-tools`, and `docker`
on the host. apt-cacher-ng is optional but drops rebuild time from
~15 min → ~3-5 min.

```bash
docker run -d --name=apt-cacher-ng -p 3142:3142 sameersbn/apt-cacher-ng
just sandbox-build full  # auto-detects port 3142
```

## Versioning

Two coordinates:

| Coord | Meaning | Bumps when |
|---|---|---|
| `schema` | ABI break | Python major changes, binary paths the server expects move, layout changes |
| `revision` | Rebuild | Security patches, pin bumps within the same schema |

The server binary embeds `SANDBOX_ROOTFS_SCHEMA_VERSION`. At boot it
reads the rootfs's `.ziee-sandbox-rootfs-schema` (a single integer)
and refuses to enable on mismatch. Revisions matching the schema are
always accepted.

Release tag format: `sandbox-rootfs-v1.r3-x86_64`. Decoupled from the
server's `v0.x.y` tags so the rootfs can ship out-of-band.

## Layout

```
src-app/sandbox-rootfs/
├── README.md              # this file
├── Dockerfile             # build recipe (full + minimal flavors)
├── build.sh               # docker-based builder; outputs .squashfs
├── compat.toml            # schema ↔ server-version matrix (server include_str!s)
├── yanks.toml             # yanked revisions (PEP 592 pattern)
├── pins/                  # exact dependency pins for reproducibility
└── ci/                    # CI workflow bodies + smoke / reproducibility checks
```

## Bootstrap (one-time, before any release exists)

Before `just sandbox-fetch` has anything to fetch:

1. `just sandbox-build minimal` and `just sandbox-build full` locally.
2. Create the GitHub Release manually:
   ```bash
   gh release create sandbox-rootfs-v1.r0-x86_64 \
     --title "Sandbox rootfs v1.r0 (x86_64)" \
     --notes "Initial rootfs release. See src-app/sandbox-rootfs/compat.toml." \
     .ziee-cache/sandbox-rootfs/ziee-sandbox-rootfs-v1.r0-x86_64-minimal.squashfs \
     .ziee-cache/sandbox-rootfs/ziee-sandbox-rootfs-v1.r0-x86_64-full.squashfs
   ```
3. From this point on, every dev / CI run uses `just sandbox-fetch`.

## Threat model

The sandbox protects against prompt-injection-induced exfiltration,
accidental destructive commands, and host filesystem pollution. It
does NOT protect against Linux kernel 0-days. For multi-tenant SaaS
execution, escalate to gVisor or Firecracker.

See `.claude/plans/replicated-enchanting-allen.md` (Phase 3) for the
full threat model + the empirical validation table that justifies the
bwrap flag set.

## Cross-references

- [`RELEASE-RUNBOOK.md`](./RELEASE-RUNBOOK.md) — bootstrap script +
  ongoing release flow, schema bumps, yanks, troubleshooting.
- [`../server/DEPLOYMENT.md`](../server/DEPLOYMENT.md) — production
  deployment patterns: baked-in Docker image
  (`src-app/server/Dockerfile.prod`), volume-mount
  (`src-app/docker-compose.prod.yaml`), self-fetch via init container.
- [`../../CLAUDE.md`](../../CLAUDE.md) — test tier overview, dev
  workflow cheat sheet.
- [`../../scripts/bootstrap-first-rootfs-release.sh`](../../scripts/bootstrap-first-rootfs-release.sh)
  — one-time bootstrap of the first GitHub release tag.
