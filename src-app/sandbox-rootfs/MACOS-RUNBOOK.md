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
