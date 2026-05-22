# macOS code-sandbox backend (libkrun microVM) — build + validation runbook

Plan 1 §2. On macOS the sandbox runs bwrap **inside a per-flavor libkrun
microVM**. This is the "Linux-everywhere" strategy: the rootfs, bwrap, and the
exact `build_bwrap_argv` hardening are reused unchanged; only *where* bwrap runs
differs. This runbook covers building the pieces and validating on Apple Silicon.

## Components (all already in the tree)

| Piece | Crate | Runs | Built/validated |
|---|---|---|---|
| Host backend | `server` → `code_sandbox/backend/mac_vm.rs` (`cfg macos`) | macOS host | on Mac |
| VM launcher | `sandbox-vm-launcher` (`ziee-sandbox-vm-launcher`) | macOS host (becomes the VM) | on Mac |
| Guest agent | `sandbox-guest-agent` (`ziee-sandbox-agent`) | inside the VM (Linux) | cross-compiled to the guest arch |
| Wire protocol | `sandbox-vm-protocol` | both | unit-tested on Linux ✅ |

Data flow: server → spawns **launcher** (per flavor) → libkrun boots, runs the
**agent** as guest init → server connects to the bridged unix socket → sends the
bwrap argv (`Frame::Exec`) → agent runs bwrap, streams `Stdout`/`Stderr`/`Exit`.

## 1. Install libkrun (dev)

```sh
brew install libkrun        # provides libkrun.dylib + libkrunfw under /opt/homebrew/lib
```

For release, bundle `libkrun.dylib` + `libkrunfw.dylib` into the app under
`Contents/Frameworks` (the launcher's `build.rs` already sets
`-rpath @executable_path/../Frameworks`).

## 2. Build the host pieces (on the Mac)

```sh
cd src-app/sandbox-vm-launcher && cargo build --release   # links -lkrun
cd ../server && cargo build --release                     # mac_vm.rs compiles in
```

## 3. Build the guest agent (cross-compile to the guest arch — aarch64 Linux)

```sh
rustup target add aarch64-unknown-linux-musl
cd src-app/sandbox-guest-agent
cargo build --release --target aarch64-unknown-linux-musl   # static musl = no guest libc deps
```

## 4. Assemble the guest root  (NEW artifact — extends §4 rootfs release)

A minimal Linux root mounted by libkrun as the guest `/`. Must contain:

- `/usr/bin/ziee-sandbox-agent`  — the agent built in step 3 (the VM entrypoint).
- `/usr/bin/bwrap`               — bubblewrap for the guest arch.
- `/etc/ziee-sandbox-passwd`, `/etc/ziee-sandbox-group` — the synthetic identity
  (same contents as `SyntheticIdentity` in `sandbox.rs`):
  - passwd: `sandboxuser:x:1001:1001:Sandbox User:/home/sandboxuser:/bin/bash`
  - group:  `sandboxuser:x:1001:`
- busybox (or coreutils) + `mount` so the agent can mount `/proc`, the squashfs,
  and the virtio-fs workspace.

The **sandbox squashfs** (the R/torch toolchain) is NOT part of the guest root —
it's fetched per flavor and added as a read-only virtio-blk disk; the agent
mounts it at `/sandbox-rootfs`.

## 5. Point the server at the pieces

```sh
export ZIEE_SANDBOX_VM_LAUNCHER=/path/to/ziee-sandbox-vm-launcher
export ZIEE_SANDBOX_GUEST_ROOT=/path/to/guest-root
```
(Defaults assume the app-bundle layout; override for dev.)

## 6. First-run validation checklist (the `MAC-TODO`s in mac_vm.rs)

Run with `code_sandbox.enabled: true` and trigger an `execute_command`; then verify:

1. **VM boots** — the launcher process starts and `krun_start_enter` doesn't
   return early (check stderr for `launcher: … failed`).
2. **vsock bridge direction** — `krun_add_vsock_port2(.., listen=true)` is the
   assumption (libkrun listens on the host unix socket, forwards to the guest
   port where the agent listens). If the server can't connect to the socket,
   **flip the `listen` flag** in `sandbox-vm-launcher/src/main.rs`.
3. **Guest device name** — the agent mounts the squashfs from `/dev/vda`
   (`ROOTFS_DEVICE`). Confirm libkrun assigns the added disk there; adjust if
   it's `/dev/vdb`/etc.
4. **virtio-fs tags** — workspace tag `workspace` matches between launcher
   (`krun_add_virtiofs`) and agent (`WORKSPACE_MOUNT`).
5. **exit code + output** round-trip through `Frame::Exit` / `Stdout`/`Stderr`.
6. **Workspace path mapping** — `guest_workspace_path` maps the host
   per-conversation dir under `/workspace`; confirm bwrap's `--bind` resolves.

## Known follow-ups (deferred, marked `MAC-TODO` in code)

- **Conversation attachments**: `build_bwrap_argv` derives attachment bind
  sources from the host `workspace_root`; for the guest these must map under
  `/workspace`. Handle once attachments are exercised on macOS.
- **In-guest cgroup v2 + seccomp**: currently `CgroupMode::None` /
  `SeccompMode::NotLinked` for the guest caps (rlimits via prlimit still apply
  inside bwrap). Add guest cgroup delegation + a guest-compiled seccomp filter.
- **VM sizing → §6**: `VM_VCPUS`/`VM_RAM_MIB` are constants; wire to the
  runtime-configurable resource limits.
- **Lazy kill-on-idle**: `VmHandle.last_used` is tracked; wire a reaper (mirror
  `mod.rs`'s workspace reaper) to stop VMs idle past `vm_idle_evict_secs`.
- **Single-flight boot**: currently the global `VMS` lock is held across boot;
  fine for rare boots, revisit if cross-flavor contention shows up.

## Security gaps from prior-art audit (microsandbox / libkrun / Apple `container`)

Audited 2026-05 against microsandbox, libkrun/krunvm/krunkit, and Apple's
Containerization framework. What we already got right, and the gaps to close.

**Aligned with prior art (keep):**
- VM boot in a **separate launcher process** (not an in-server fork) — required
  because `krun_start_enter` `exit()`s + seizes stdio.
- **bwrap-in-VM, non-root (uid 1001), read-only squashfs as a block disk** —
  matches the libkrun maintainers' "layer container-isolation inside the VM, run
  as non-root" guidance (discussion #538). The squashfs is `/dev/vda` read-only.
- **vsock `listen=true`** = host-dials / guest-listens — confirmed the correct
  flag for our host-drives-the-agent model (still verify at first run).
- **In-guest rlimits** (pids/as/fsize/nofile via the prlimit wrapper in
  `build_bwrap_argv`) + a hard VM RAM ceiling — limits are enforced, not just
  requested.
- **Native aarch64 in the VM on Apple Silicon** (no Rosetta needed) — *ensure
  the fetched squashfs flavor + guest bwrap/agent are aarch64*.

**Gaps to close (ordered by relevance to our threat model — prompt-injection
exfiltration, host-FS pollution):**
1. **Workspace virtio-fs shares the whole `workspace_root` — ACCEPTABLE, not a
   regression** (re-assessed). bwrap inside the guest is the per-conversation
   boundary: it `--bind`s only the per-conversation subdir into the sandbox
   mount namespace, so the sandboxed command can't see `/workspace` or other
   conversations regardless of what's shared. It only matters under a *bwrap
   escape* — and then the exposed set (all of `workspace_root`) is identical to
   the Linux backend (a bwrap escape there lands on the host with full
   `workspace_root` access), except the VM keeps the attacker contained behind
   libkrun too. So sharing the whole root is fine and avoids prohibitive
   per-conversation copy-in/out. Residual (low): the generic virtio-fs→host-fs
   escape surface — keep `workspace_root` on a dedicated mount/subvolume so a
   virtio-fs traversal bug can't reach the wider host fs.
2. **Guest root via host-dir virtio-fs.** `krun_set_root` shares a host
   directory as `/`; libkrun + Apple both recommend a **read-only EXT4/raw block
   image** for an untrusted guest root (smaller escape surface, no qcow2
   auto-open footgun). Ship the guest root as a block image instead.
3. **TSI egress = guest reaches everything the VMM can.** With no net device,
   libkrun enables Transparent Socket Impersonation: the guest shares the VMM
   process's network context — it can reach **host-localhost services** (our own
   API/DB), an SSRF/exfil surface beyond Linux's `--share-net`. Fix: run the
   launcher with a restricted network, or switch to virtio-net + a filtering
   proxy (microsandbox-style allow/deny + DNS + TLS policy is the mature model).
4. **Launcher inherits the server's env (secrets in the VMM).** We spawn the
   launcher without `env_clear()`, so the VMM process holds `DATABASE_URL`/JWT/
   API keys. The guest workload itself is clean (`krun_set_exec` envp=[] +
   bwrap `--clearenv`), but a VMM-escape would reach them. Fix: spawn the
   launcher with a minimal/cleared env.
5. **No orphan-on-crash teardown on macOS.** Linux uses `PR_SET_PDEATHSIG` so
   FUSE/VMs die with the server even on SIGKILL; `kill_on_drop` only covers
   graceful drop. macOS has no PDEATHSIG — have the launcher watch its parent
   (poll `getppid()` or kqueue `EVFILT_PROC`/`NOTE_EXIT`) and exit if the server
   dies, else a server crash leaks VMs.
6. **No agent liveness / hung-guest detection.** microsandbox heartbeats at 1 Hz
   so the host can enforce `idle_timeout`/`max_duration` and reap hung guests.
   We track `last_used` but have no reaper and no heartbeat — wire both.
