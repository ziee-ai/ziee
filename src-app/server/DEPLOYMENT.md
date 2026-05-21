# Production deployment guide

This guide covers deploying ziee-chat to production with `code_sandbox`
enabled. The sandbox needs Linux, bwrap, squashfuse, and a mounted
sandbox rootfs.

## Three deployment patterns

| Pattern | Image | Rootfs mgmt | Best for |
|---|---|---|---|
| (A) Baked image | `ziee/server-with-sandbox` | Inside image | Single-server, simple ops |
| (B) Volume mount | `ziee/server` + external squashfs | Host volume, swap independently | Multi-server, rolling rootfs upgrades |
| (C) Self-fetch | `ziee/server` + entrypoint that runs `fetch-sandbox-rootfs` | Container fetches on first boot | Kubernetes init container, air-gapped with mirror |

All three need cgroup v2 delegation on the host. The rest of this guide
walks through each pattern.

## Host prerequisites (any pattern)

Linux kernel >= 5.10. Install:

```bash
sudo apt install bubblewrap squashfuse fuse3 libseccomp2
```

For cgroup v2 delegation (recommended — enables `memory.max` enforcement
per sandbox call instead of falling back to rlimits-only):

```bash
sudo mkdir -p /sys/fs/cgroup/ziee-sandbox.slice
echo "+memory +pids +cpu" | sudo tee \
    /sys/fs/cgroup/ziee-sandbox.slice/cgroup.subtree_control

# Chown to the uid the server runs as (default 10000 in the image)
sudo chown -R 10000:10000 /sys/fs/cgroup/ziee-sandbox.slice
```

Verify your kernel has the delegated controllers:

```bash
cat /sys/fs/cgroup/cgroup.controllers   # must include memory, pids
```

## Pattern A — baked image

Cleanest setup. The rootfs is COPYed into the image at build time;
the entrypoint mounts the squashfs via squashfuse on container start.

### Build

```bash
docker build \
    -f src-app/server/Dockerfile.prod \
    --build-arg ROOTFS_TAG=sandbox-rootfs-v1.r0-x86_64 \
    --build-arg ROOTFS_FLAVOR=full \
    -t ziee/server-with-sandbox:0.1.0-v1.r0 \
    .
```

The build fetches the published rootfs from GitHub Releases and verifies
its sha256 against the embedded `known_revisions.toml`. Build fails if
the tag doesn't exist or sha256 doesn't match — there's no "skip
verification" escape hatch.

### Run

See `src-app/docker-compose.prod.yaml` for a reference compose file.
Minimum docker invocation:

```bash
docker run -d --name ziee \
    --cgroup-parent=ziee-sandbox.slice \
    --cap-add=SYS_ADMIN \
    --security-opt seccomp=unconfined \
    --security-opt apparmor=unconfined \
    --device /dev/fuse \
    -e DATABASE_URL=postgresql://... \
    -e CODE_SANDBOX_CGROUP_PARENT=/sys/fs/cgroup/ziee-sandbox.slice \
    -p 8080:8080 \
    ziee/server-with-sandbox:0.1.0-v1.r0
```

Why each option matters:
- `--cgroup-parent` — places the container's cgroup under the delegated
  slice so per-call sandbox scopes can be created.
- `--cap-add SYS_ADMIN` — bwrap needs to call `unshare()` /
  `mount(MS_BIND, ...)`. Without this, EPERM at sandbox start.
- `--security-opt seccomp=unconfined` — docker's default seccomp profile
  BLOCKS `unshare(CLONE_NEWUSER)`. The sandbox absolutely needs user
  namespaces; this disables docker's profile (the SERVER then applies
  its OWN seccomp filter inside bwrap, so the sandboxed processes are
  still confined).
- `--security-opt apparmor=unconfined` — AppArmor on Ubuntu/Debian
  hosts has a docker-default profile that blocks bwrap mounts. Disable
  per-container.
- `--device /dev/fuse` — squashfuse needs the FUSE device node to mount
  the rootfs.

Verify boot:

```bash
docker logs ziee | grep "code_sandbox: hardening"
# Expected: pid_ns: on, cgroup_v2: on (delegated), seccomp: on
```

If you see `cgroup_v2: off-needs-delegation` — re-check the host
prereqs above. The server still runs with rlimits-only enforcement.

If you see `pid_ns: off-fallback-dev-bind` — your kernel + container
config prevents nested `--unshare-pid --proc /proc`. Common on Docker
Desktop or rootless docker. Falls back to dev-bind /proc (info leak;
no escape).

## Pattern B — volume-mount

Smaller image; rootfs lives on the host. Easier to swap rootfs
revisions without rebuilding the server image.

### Prep the rootfs on the host

```bash
# Install the CLI from the released ziee-chat binary
sudo ziee-chat fetch-sandbox-rootfs --version=latest --flavor=full
sudo ziee-chat mount-sandbox-rootfs

# Verify
ls /var/lib/ziee/sandbox-rootfs/current/usr/bin/python3
cat /var/lib/ziee/sandbox-rootfs/current/.ziee-sandbox-rootfs-schema
```

### Run

```bash
docker run -d --name ziee \
    --cgroup-parent=ziee-sandbox.slice \
    --cap-add=SYS_ADMIN \
    --security-opt seccomp=unconfined \
    --security-opt apparmor=unconfined \
    --device /dev/fuse \
    -v /var/lib/ziee/sandbox-rootfs:/var/lib/ziee/sandbox-rootfs:ro \
    -e CODE_SANDBOX_ROOTFS_PATH=/var/lib/ziee/sandbox-rootfs/current \
    -e CODE_SANDBOX_CGROUP_PARENT=/sys/fs/cgroup/ziee-sandbox.slice \
    -e DATABASE_URL=postgresql://... \
    -p 8080:8080 \
    ziee/server:0.1.0
```

### Upgrading the rootfs

Zero-downtime rootfs upgrade (within the same schema):

```bash
# On the host:
sudo ziee-chat fetch-sandbox-rootfs --version=v1.r2 --flavor=full
sudo ziee-chat mount-sandbox-rootfs

# The `current` symlink is swapped atomically. New sandbox calls
# use the new rootfs; in-flight calls finish on the old one.
sudo ziee-chat gc-sandbox-rootfs --keep=2   # remove old squashfs files
```

Server restart NOT required.

For a schema bump (`v1.x` → `v2.x`), you must also upgrade the server
binary in lock-step. The server's boot probe refuses to register the
sandbox MCP row on a schema mismatch.

## Pattern C — self-fetch via init container

For Kubernetes or air-gapped deploys where you want the container to
fetch its own rootfs from a mirror at startup. Conceptually:

```yaml
# k8s pseudocode
spec:
  initContainers:
    - name: fetch-rootfs
      image: ziee/server:0.1.0
      command:
        - ziee-chat
        - fetch-sandbox-rootfs
        - --version=latest
        - --flavor=full
      env:
        - name: CODE_SANDBOX_ROOTFS_MIRROR
          value: https://internal-mirror.example.com/ziee-sandbox-rootfs
      volumeMounts:
        - name: sandbox-rootfs
          mountPath: /var/lib/ziee/sandbox-rootfs
  containers:
    - name: ziee
      image: ziee/server:0.1.0
      # ... same as Pattern B
      volumeMounts:
        - name: sandbox-rootfs
          mountPath: /var/lib/ziee/sandbox-rootfs
          readOnly: true
  volumes:
    - name: sandbox-rootfs
      emptyDir: {}   # or persistent for multi-restart caching
```

## Common failure modes

### `code_sandbox: SANDBOX_NOT_INITIALIZED`

Either `code_sandbox.enabled: false` in config, OR the boot probes
failed. Check the startup log:
```
code_sandbox: hardening = { rlimits: on, bwrap: ?, pid_ns: ?, cgroup_v2: ?, seccomp: ? }
```
- `bwrap: MISSING` → install bubblewrap on the host AND make sure the
  container can see it (`docker exec ziee which bwrap`)
- `pid_ns: DISABLED` → bwrap can't create namespaces. Add SYS_ADMIN
  cap + seccomp:unconfined to the container.
- `rootfs schema version mismatch` → upgrade or pin to a compatible
  release.

### `bwrap: Can't mount proc on /proc: Permission denied`

The container blocks nested `mount(2)`. Either:
- Pattern A1/A2 settings above (SYS_ADMIN + seccomp:unconfined), OR
- Accept `pid_ns: off-fallback-dev-bind` mode (info leak, no escape).

### `squashfuse: failed to exec fusermount`

Missing `fuse3` package on the host OR `--device /dev/fuse` not passed.

### `cgroup_v2: off-needs-delegation`

The host's cgroup slice isn't writable by the server uid. Re-run the
prep commands at the top of this doc and confirm with:
```bash
sudo -u ziee touch /sys/fs/cgroup/ziee-sandbox.slice/test \
    && rm /sys/fs/cgroup/ziee-sandbox.slice/test
```

## Security checklist before opening to users

- [ ] Cgroup delegation working (`cgroup_v2: on (delegated)` in log)
- [ ] Seccomp enabled (`seccomp: on` — requires `code_sandbox_seccomp`
      cargo feature at server build time AND libseccomp2 on the host)
- [ ] Rootfs is from a signed release (`cosign verify-blob` against the
      `.cosign.bundle` from the GitHub release)
- [ ] Server runs as non-root uid (defaults to 10000 in
      `Dockerfile.prod`)
- [ ] cap_drop matches Pattern A1, NOT `privileged: true`
- [ ] Postgres credentials in env, not baked into images
- [ ] Persistent volume for `/var/lib/ziee/data` (workspace artifacts)
- [ ] Backup / rotation policy for the workspace volume
- [ ] Network egress from sandboxed code restricted to allowed
      destinations (the sandbox has `--share-net`; bwrap doesn't
      isolate egress — use docker's network policy or a firewall)
