# WSL2 sandbox — source-code deep read (Plan 1 §3, HIGH-1 mitigation design)

**Date:** 2026-05-22  •  **Successor to:** `wsl2-sandbox-prior-art-2026-05-22.md`  •
**Inputs:** `/tmp/codex-source-read.md`, `/tmp/anthropic-source-read.md`, `/tmp/wsl-source-read.md` (1883 lines total of code-cited analysis).

## Method

Three parallel source reads:

1. **`openai/codex`** — `codex-rs/windows-sandbox-rs/` (host↔sandbox-user IPC) + `codex-rs/linux-sandbox/` (WSL2 mode).
2. **`anthropic-experimental/sandbox-runtime`** — Linux bwrap + nested-namespace + mandatory-deny + AppArmor + egress proxy.
3. **`microsoft/WSL`** — the WSL source itself: net-namespace setup, AF_VSOCK availability, `wsl.exe` relay, `/mnt/c` plan9, `wsl --import` parsing.

Goal: **ground the HIGH-1 mitigation decision in code, not speculation,** and extract any lift-able patterns for HIGH-2..MED-3.

---

## Headline decision: switch the agent transport to AF_VSOCK

The Microsoft WSL source confirms three facts that change the picture:

### 1. WSL2 distros share one network namespace (and we now know exactly where)

```cpp
// microsoft/WSL: src/linux/init/main.cpp:2283
// "Create a child process in a new mount, pid, and UTS namespace
//  (with a shared IPC namespace)."
clone3 with CLONE_NEWNS | CLONE_NEWPID | CLONE_NEWUTS
// — no CLONE_NEWNET. NetworkManager.cpp:328 explicitly calls the
//   single netns "the Linux root network namespace".
```

This is the source-level confirmation of HIGH-1: there is one netns for the whole utility VM. Any distro's `connect("127.0.0.1", port)` reaches every other distro's listening socket.

### 2. AF_VSOCK is exposed unprivileged inside the WSL2 guest

```cpp
// microsoft/WSL: src/linux/init/util.cpp:307-325 (UtilBindVsockAnyPort)
SocketFd = socket(AF_VSOCK, Type, 0);
SocketAddress->svm_family = AF_VSOCK;
SocketAddress->svm_cid = VMADDR_CID_ANY;
SocketAddress->svm_port = VMADDR_PORT_ANY;
bind(SocketFd, (const struct sockaddr*)SocketAddress, SocketAddressSize);

// util.cpp:572-621 (UtilConnectVsock)
SocketFd = socket(AF_VSOCK, Type, 0);
SocketAddress.svm_cid = VMADDR_CID_HOST;
SocketAddress.svm_port = Port;
connect(SocketFd, (const struct sockaddr*)&SocketAddress, sizeof(SocketAddress));
```

WSL's own `init` opens vsock with plain BSD calls — no CAP_NET_ADMIN, no kernel module load, no admin. Our existing agent already supports `Listen::Vsock(port)` (Step 1 of the cross-platform plan generalized this for macOS libkrun). **No agent-side change required.**

### 3. The Windows-side hvsocket is firewalled per-VM by HCS

```cpp
// microsoft/WSL: src/windows/service/exe/HcsVirtualMachine.cpp:245-254
auto tokenUser = wil::get_token_information<TOKEN_USER>(m_userToken.get());
ConvertSidToStringSidW(tokenUser->User.Sid, &userSidString);
std::wstring securityDescriptor =
    std::format(L"D:P(A;;FA;;;SY)(A;;FA;;;{})", userSidString.get());
```

Translation: the SDDL `D:P(A;;FA;;;SY)(A;;FA;;;<user-sid>)` grants full access to `SY` (LocalSystem) and the launching user's SID. Every other Windows account on the same host is locked out at the kernel level. Per-user multi-tenant safe.

### 4. WSL uses `HV_GUID_VSOCK_TEMPLATE` — no registry registration needed

```cpp
// microsoft/WSL: src/windows/common/hvsocket.cpp:22-29
// HV_GUID_VSOCK_TEMPLATE with Data1 = port
constexpr GUID HV_GUID_VSOCK_TEMPLATE = {
    0x00000000, 0xfacb, 0x11e6, …
};
// Set ServiceId.Data1 = port to address a specific listener.
```

The standard Hyper-V vsock template; the per-app GUID registration under `HKLM\…\GuestCommunicationServices` is **not** required for the template GUID. Plan 9 (`/mnt/c`) uses exactly this mechanism — we'd be following WSL's own trusted-plane pattern.

### Implication

Switching the WSL2 transport from `tcp:127.0.0.1:<port>` to `vsock:<port>` does three things at once:

| Property | Before (TCP loopback) | After (AF_VSOCK + hvsocket) |
|---|---|---|
| Cross-distro reachability | **Open** (single netns; verified `main.cpp:2283`) | **Structurally impossible** (vsock is point-to-point host↔this-guest) |
| Cross-user reachability on Windows | N/A (TCP doesn't cross VM boundary) | Blocked by HCS DACL (`HcsVirtualMachine.cpp:245-254`) |
| Protocol-layer auth needed | Required (currently missing) | **Optional** — OS already enforces it |
| Agent code changes | — | None (vsock listener already shipped in Step 1) |
| Host-side code changes | — | New `vsock_client.rs`: dial AF_HYPERV/HV_PROTOCOL_RAW to (VmId, port) |
| Open question | — | How to resolve the WSL VmId at runtime (HCS API: `HcsEnumerateComputeSystems` filter by distro name) |

**Recommendation: adopt vsock for the WSL2 host↔agent channel.** It removes a class of vulnerability instead of patching one. macOS already does this (vsock-bridged unix socket via libkrun). Backends converge.

---

## Alternative considered: eliminate the in-distro agent entirely

Codex's WSL2 mode has **no agent**. The user runs Codex CLI inside the WSL distro; per-command, Codex execvs bwrap in its own process tree:

```rust
// openai/codex: codex-rs/linux-sandbox/src/launcher.rs:148
unsafe {
    libc::execv(program.as_ptr(), argv_ptrs.as_ptr());
}
// — no listening socket, no port, no authenticator inside the distro.
```

For us, the equivalent would be: drop the persistent agent; per `execute_command`, the Windows host spawns `wsl.exe -d <distro> --exec /opt/ziee/sandbox-bwrap-launcher <argv>`, captures stdio, kills on timeout. Eliminates HIGH-1 by removing the attack surface.

**Why we should still prefer vsock + warm agent:**

- Per-call `wsl.exe --exec` startup is ~50-150 ms on Win11 (cold-cache distro launch, NT process creation, plan9 file-server activation). For a turn that fires several `execute_command`s in quick succession (R `install.packages` then immediate run), this compounds.
- Codex's no-agent design works because they run *inside* the distro. We run on Windows and call in. Our equivalent is structurally more like Docker Desktop (which uses hvsocket-bridged named pipes) than Codex.
- The macOS backend has the same pattern (warm libkrun VM + agent); WSL2 with vsock + warm agent gives **architectural symmetry** across mac/win.

If we ever decide the warm-agent cost outweighs the latency win, the per-call path is a 2-day refactor: drop the registry/lifecycle, replace `ensure_distro` with a thin `wsl.exe` invocation builder.

---

## Implementation sketch for the vsock switch

**No agent change.** Step 1 (`sandbox-guest-agent/src/main.rs::parse_listen`) already accepts `--listen vsock:<port>`. Default is already `Listen::Vsock(1024)`.

**Windows-side changes (4 file edits, ~150 lines):**

1. **New** `src-app/server/src/modules/code_sandbox/backend/hvsocket.rs` (Windows-only, `cfg(target_os = "windows")`): wrap the Win32 socket calls for AF_HYPERV. We need `WSAStartup`, `socket(AF_HYPERV, SOCK_STREAM, HV_PROTOCOL_RAW)`, `SOCKADDR_HV` (VmId + ServiceId), `connect`, return a tokio-compatible stream (use `tokio::net::TcpStream::from_std` over a `std::net::TcpStream` we wrap via `windows_sys`).

2. **`wsl2.rs:229-244`** (agent spawn): change `--listen tcp:127.0.0.1:<port>` → `--listen vsock:<port>` (port chosen per-flavor — picks from `1024-65535` range, persisted on the `DistroHandle`).

3. **`wsl2.rs:469-500`** (the run-time connect): replace `TcpStream::connect(("127.0.0.1", h.tcp_port))` with `hvsocket::connect(h.vm_id, h.vsock_port)`. The handle gains `vm_id: GUID` + `vsock_port: u32`.

4. **`wsl2.rs:213-217`** (post-import): query the WSL VM's GUID via the HCS API. Two options:
   - **Preferred**: spawn `wsl.exe -d <distro> -- cat /proc/sys/kernel/random/boot_id` and rely on a deterministic per-distro mapping — but WSL2 uses a SHARED utility VM, so all distros have the same VmId. Get it once at backend init, cache it.
   - **Cleanest**: call `HcsEnumerateComputeSystems(L"WSL")` via `hcs.dll`, filter for the WSL2 utility VM, extract `Id`. No third-party deps; ~30 lines of FFI.

**Plus removal**: the existing TCP fallback in the agent (`Listen::Tcp`) stays — it's used by tests (e.g. a `--listen tcp:127.0.0.1:<port>` for a local-host test rig where vsock isn't available). Mark it as test-only in a comment.

---

## Other lift-able patterns (HIGH-2..MED-3 mitigations grounded in source)

### From Anthropic `sandbox-runtime` (`src/sandbox/sandbox-utils.ts:11-40`):

The verbatim mandatory-deny lists — ready to port to Rust:

```typescript
export const DANGEROUS_FILES = [
  '.gitconfig', '.gitmodules', '.bashrc', '.bash_profile',
  '.zshrc', '.zprofile', '.profile', '.ripgreprc', '.mcp.json',
] as const

export const DANGEROUS_DIRECTORIES = ['.git', '.vscode', '.idea'] as const

export function getDangerousDirectories(): string[] {
  return [
    ...DANGEROUS_DIRECTORIES.filter(d => d !== '.git'),
    '.claude/commands',
    '.claude/agents',
  ]
}
```

The application logic at `linux-sandbox-utils.ts:166-284`:
- ripgrep finds every match within `cwd` (max-depth 3).
- For each match: `--ro-bind /dev/null <abspath>` (files) or `--ro-bind <empty-dir> <abspath>` (dirs).
- **`.git/hooks` and `.git/config` are masked ONLY when `.git` is a directory** (worktree-safe — they delegate elsewhere when `.git` is the worktree file).

**Lift**: add a const + loop to `build_bwrap_argv` in `sandbox.rs` right after the workspace `--bind` (line 362-364), with `.git`-as-directory gating.

### From Anthropic — `--new-session` is missing from our bwrap argv

```typescript
// src/sandbox/linux-sandbox-utils.ts — present in their argv
'--new-session', '--die-with-parent',
```

We have `--die-with-parent` (sandbox.rs:327) but **not** `--new-session`. The latter is a defense against TIOCSTI-style TTY injection (a sandboxed process writing to its controlling TTY to inject keystrokes that the parent's shell will execute when bwrap exits). One-line fix at sandbox.rs:316.

### From Microsoft WSL — `/etc/wsl.conf` boot command runs as root and reliably

```cpp
// microsoft/WSL: src/linux/init/config.cpp:1002
execl("/bin/sh", "sh", "-c", Command.c_str(), nullptr);
```

For HIGH-4 (sysctl persistence): write `/etc/wsl.conf` with `[boot] command = aa-load /etc/apparmor.d/bwrap && sysctl --system` during provision. WSL runs this every VM boot, before any user shell. The `[boot] systemd = true` alternative additionally re-enables the standard systemd-sysctl service but adds startup latency.

### From Microsoft WSL — `wsl --import` uses libarchive's bsdtar

```cpp
// microsoft/WSL: src/windows/wslservice/exe/main.cpp:904-915
// Uses bsdtar (libarchive) with its default secure-by-default
// path traversal handling. `.tar.zst` works because libarchive
// auto-detects compression.
```

Confirms our Step 3 choice of `.tar.zst` is correct. The Windows CLI's `--format` flag only restricts *export* (`WslClient.cpp:233-247` whitelist), not import — import accepts any libarchive-supported format.

### From Codex — three secondary IPC lessons we should lift even with vsock

1. **Length-prefixed framing with explicit version + frame cap**: `ipc_framed.rs:137-166` uses `u32 LE length + JSON body, 8 MiB cap, IPC_PROTOCOL_VERSION` field in envelope. Our `sandbox-vm-protocol` already has length-prefixed binary frames; **add a version field** + a per-frame size cap (we have a 1 MiB output cap but no protocol-level frame cap).
2. **`MaxInstances = 1` analog**: `runner_pipe.rs:87` sets the named-pipe instance count to 1. The agent should `accept()` exactly one vsock connection per request and close the listener slot afterward (or use a `Semaphore::new(MAX_CONCURRENT_EXECS)` to bound, which we already do at `wsl2.rs:80-83`).
3. **`OsRng`, not `SmallRng`**: if we ever add a defense-in-depth shared-secret layer (under the vsock DACL), use `OsRng::default()` for the 32-byte token, never `SmallRng` (Codex itself uses `SmallRng::from_entropy()` at `sandbox_users.rs:360-372` — questionable; we should be stricter).

### From Anthropic — what we deliberately DON'T need

- **Nested-namespace + `apply-seccomp` PID-1 reaper** (`vendor/seccomp-src/apply-seccomp.c`): Anthropic needs this because their `socat` egress proxy runs *inside* the sandbox; a workload could otherwise ptrace socat and degrade the filter. We have no in-sandbox helpers, so `bwrap --seccomp <fd>` is sufficient. Add a doc comment at `sandbox.rs:1-6` explaining the deliberate divergence.
- **AppArmor profile shipped in-tree**: Anthropic doesn't ship one either — just documents the workaround. We already do the same in CLAUDE.md. The narrow profile from HIGH-2 still wants writing, but we don't need to ship-by-default.

---

## Revised priority list (factoring in source reads)

| # | Original audit | Source-read refinement | Effort | Confidence |
|---|---|---|---|---|
| 1 | HIGH-1 — agent auth | **Adopt AF_VSOCK** (point-to-point structural fix; no agent change). Add `hvsocket.rs`, switch backend to dial vsock. | M (2-3 days) | **HIGH** — WSL source confirms unprivileged availability + HCS DACL + cross-distro impossibility. |
| 2 | HIGH-4 — sysctl persistence | Write `/etc/wsl.conf` with `[boot] command=` (confirmed reliable in `config.cpp:1002`). | S (1 day) | HIGH |
| 3 | HIGH-3 — reaper + agent orphan | Add `Frame::Shutdown` to protocol; `stop_agent` sends + waits; correct reaper comment. | S | MED |
| 4 | HIGH-2 — narrow AppArmor profile | Write the profile (Claude Code's recipe); load via `/etc/wsl.conf` `[boot] command=aa-load …` from #2. | S | HIGH |
| 5 | **NEW** | Port Anthropic's `DANGEROUS_FILES` / `DANGEROUS_DIRECTORIES` mask logic to `build_bwrap_argv`. Worktree-safe `.git` gating. | S | HIGH |
| 6 | **NEW** | Add `--new-session` to `build_bwrap_argv`. One line. | XS | HIGH |
| 7 | MED-2 | WSL `--version` probe for CVE-2025-53788 (≥ 2.5.10 / 2.6.1). | S | HIGH |
| 8 | MED-1 | Drop `/mnt/<drive>` 9p bind (existing WIN-TODO, security argument strengthens it). | M | MED |
| 9 | MED-3 | Replace WSLENV provisioning with temp-file + `wsl.exe cp` (no env crossing). | XS | MED |
| 10 | LOW-1/2/3 | Comment fixes, mirrored-mode warn-log, threat-model header. | XS | LOW |
| 11 | **NEW** | Add version field + per-frame cap to `sandbox-vm-protocol` envelope (Codex pattern). | XS | LOW |
| 12 | **NEW** | Doc comment at `sandbox.rs:1-6` explaining why we don't ship Anthropic's nested-ns + apply-seccomp pattern. | XS | LOW |

## Bottom line

The HIGH-1 mitigation **is** adopt vsock — not shared-secret-over-TCP. Microsoft's own WSL `init` does it; the kernel boundary is the auth; cross-distro reachability becomes structurally impossible; our agent already supports it (Step 1). The remaining work is on the Windows-side dial path (~150 lines of `windows-sys` AF_HYPERV calls + a one-time HCS query for the WSL VM's GUID).

The secondary lifts (mandatory-deny masks, `--new-session`, `[boot] command=` for sysctl persistence) are short, high-value, low-risk additions grounded in code we just read.

## Source artifacts

Saved locally for future reference (NOT in-repo):
- `/tmp/codex-source-read.md` (604 lines)
- `/tmp/anthropic-source-read.md` (604 lines)
- `/tmp/wsl-source-read.md` (675 lines)

Cloned repos (also `/tmp/`):
- `/tmp/codex-research/codex/` (openai/codex)
- `/tmp/sandbox-runtime/` (anthropic-experimental/sandbox-runtime)
- `/tmp/microsoft-wsl/` (microsoft/WSL — used for the source citations above)
