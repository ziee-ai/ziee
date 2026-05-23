# WSL2 sandbox — prior-art audit (Plan 1 §3 Windows backend)

**Date:** 2026-05-22  •  **Branch:** `feat/sandbox-cross-platform`  •  **Target:**
`src-app/server/src/modules/code_sandbox/backend/wsl2.rs` (660 lines) +
`src-app/sandbox-guest-agent/src/main.rs` (the universal in-guest executor).

## Method

Three parallel research streams: (a) open-source WSL2-based sandboxes; (b) WSL2
threat model + escape vectors + CVEs; (c) production AI-tool code-execution
architecture on Windows. ~30 sources cross-checked; Codex sandbox source read
locally where line citations were needed.

## Where we sit, in one paragraph

Cursor + GitHub Copilot/VS Code both ship the **same architecture as ours** —
"on Windows we run our Linux sandbox inside WSL2." Cursor states publicly that
"building an equivalent native Windows sandbox is significantly harder" and they
explicitly punted. **Anthropic Claude Code has no Windows sandbox at all** today
(`sandbox-runtime/README.md`: "Windows: Not yet supported"; commands run
directly via PowerShell/Git Bash on the host). **OpenAI Codex CLI is the only
peer that built a deep native-Windows sandbox** — ~12,000 lines of Rust
(`codex-rs/windows-sandbox-rs/`) with dedicated low-priv user accounts,
write-restricted tokens, capability SIDs, WFP egress filters, private desktop,
and named-pipe IPC — but their fallback WSL2 mode is just bwrap-inside-WSL2 like
ours. **Most other tools have nothing** (Continue, Aider, Open Interpreter,
AutoGen-local, Jupyter, every shell-MCP server). Our bet is validated; the gaps
below are *within* the WSL2+bwrap design, not against it.

## Findings — prioritized

Severity: **HIGH** = exploitable in a realistic threat model.
**MED** = chained condition / future-regression vector. **LOW** = correctness /
operational.

---

### HIGH-1 — Cross-distro reachable agent + no authentication ⚠

**This is the headline finding. All three research streams converged on it.**

**Evidence (verified):**
- Microsoft's `localhost.cpp` source confirms localhost forwarding "shared
  across the utility VM, not per-distro"
  ([source](https://github.com/microsoft/WSL/blob/master/src/linux/init/localhost.cpp),
  [doc](https://github.com/microsoft/WSL/blob/master/doc/docs/technical-documentation/localhost.md)).
- [microsoft/WSL#4304](https://github.com/microsoft/WSL/issues/4304) (closed
  "bydesign"): "Multi WSL2 distributions use the same network namespace."
  Distros cannot bind the same port — they collide.
- Microsoft Learn confirms WSL2 distros share the utility VM's network
  namespace (their own threat-model wording).
- Direct precedent: **[CVE-2025-9074](https://www.mindpatch.net/posts/docker-escape-ssrf/)**
  (Docker Desktop, CVSS 9.3, Aug 2025) — dockerd reachable cross-VM from any
  container via the WSL2 internal subnet. **Same shape of bug as ours.**
- Our agent (`sandbox-guest-agent/src/main.rs:151-160`) accepts unauthenticated
  TCP and exec's whatever `ExecRequest.argv` arrives. No shared secret, no peer
  cred check.

**Exposure:** any process running in *any other WSL2 distro the user has
installed* (their default `Ubuntu`, `Kali`, `docker-desktop`,
`rancher-desktop`, any earlier ziee flavor that hasn't been `--terminate`d)
can `connect("127.0.0.1", <our-port>)` and submit an arbitrary bwrap argv. The
agent runs as root in the distro → attacker gets root-equivalent FS access
inside our flavor + the bound `/mnt/<drive>` workspace.

This is *also* the pivot vector for ANY process with normal user-level code
execution on the Windows host (malicious VSCode extension, compromised
`npm install` in the user's regular WSL Ubuntu, etc.).

**Industry comparison:**
- Codex CLI uses **named pipes with DACL pinned to the sandbox SID + client
  PID verification** ([`runner_pipe.rs:56-103`](https://github.com/openai/codex/blob/main/codex-rs/windows-sandbox-rs/src/elevated/runner_pipe.rs)).
- Docker Desktop uses **named pipes** (`\\.\pipe\docker_engine`) with Windows
  ACLs.
- Podman Machine uses **gvproxy/hvsock + SSH key auth**.
- We are the only one shipping unauthenticated TCP loopback.

**Mitigation options** (in order of strength):

1. **AF_VSOCK between Windows host and the WSL2 VM** (Hyper-V vsock, via HCS
   API). Highest isolation; no other distro on the same host can connect.
   Requires GUID registration in `HKLM\SOFTWARE\Microsoft\Windows
   NT\CurrentVersion\Virtualization\GuestCommunicationServices\<GUID>`.
   **NOTE:** [CVE-2025-21756](https://www.wiz.io/vulnerability-database/cve/cve-2025-21756)
   ("Attack of the Vsock", Apr 2025, kernel vsock UAF guest→host root) argues
   for requiring a patched kernel before going this route — not blocker but
   needs a runtime version probe.
2. **Per-launch shared secret + HMAC challenge.** Generate 32B random token at
   `ensure_distro` time, write to `/run/ziee-sandbox/token` inside the distro
   via `wsl.exe`, agent reads + requires it on every connection. Smallest
   change; lands at `sandbox-guest-agent/src/main.rs:151-160` (auth before
   `handle_conn`) + `wsl2.rs:229-244` (token generation/plumbing).
3. **Unix-socket inside the distro + stdio relay through `wsl.exe`.** Eliminates
   TCP entirely. The Windows-side `wsl.exe` child is the only thing that can
   reach the socket. Peer-cred check on the Unix socket boundary.

**Recommended fix:** option 2 (shared secret) as the immediate stopgap +
option 3 (Unix socket via wsl.exe stdio) as the actual structural fix. The
agent's transport is already abstracted (Step 1 generalized `Listen::Vsock |
Listen::Tcp`); adding `Listen::Stdio` is a natural extension.

---

### HIGH-2 — Global AppArmor disable is broader than needed

**Evidence:**
- `wsl2.rs:303-305` runs
  `sysctl -w kernel.apparmor_restrict_unprivileged_userns=0` globally inside
  the distro.
- Ubuntu's own threat-model
  ([blog](https://ubuntu.com/blog/ubuntu-23-10-restricted-unprivileged-user-namespaces)):
  the policy is the only barrier against in-userns kernel exploits on noble.
- [Qualys oss-sec, Mar 27 2025](https://seclists.org/oss-sec/2025/q1/253):
  three working bypasses found, proving the policy actively mitigates real
  exploits.
- **Claude Code's docs explicitly recommend the narrow alternative**
  ([code.claude.com/docs/en/sandboxing](https://code.claude.com/docs/en/sandboxing)):
  ```
  profile bwrap /usr/bin/bwrap flags=(unconfined) {
    userns,
    include if exists <local/bwrap>
  }
  ```
  Per-binary `userns` grant only to `/usr/bin/bwrap`, keeping the kernel-level
  restriction in place for everything else.

**Exposure:** any unprivileged binary inside the distro can spawn userns —
including a sandbox-escaped process that broke out of bwrap into the distro.

**Mitigation:** replace the global `sysctl -w` in `wsl2.rs:294-313` with
`aa-load <bwrap profile>` (writing the narrow profile). Ship the profile in
the rootfs build so it's already present before provisioning runs.

---

### HIGH-3 — Reaper + agent-orphan misunderstanding

**Evidence:**
- Microsoft FAQ + [microsoft/WSL#13291](https://github.com/microsoft/WSL/issues/13291),
  [#8854](https://github.com/microsoft/WSL/issues/8854),
  [#8161](https://github.com/microsoft/WSL/issues/8161): `wsl --terminate
  <distro>` stops the in-VM init for that distro but does **not** free the
  utility VM's RAM. Only `wsl --shutdown` (entire utility VM, all distros)
  does.
- [microsoft/WSL#1037](https://github.com/Microsoft/WSL/issues/1037):
  `PR_SET_PDEATHSIG` only works in-distro (between Linux processes). Killing
  the Windows-side `wsl.exe` relay child does NOT necessarily kill the
  in-distro agent — `kill_on_drop(true)` at `wsl2.rs:242` only kills the
  relay. The agent talks to us over a *separate* TCP socket and can outlive
  the relay.
- Our reaper comment at `wsl2.rs:127-128` ("Terminate the distro too so its
  slice of the shared WSL2 VM's RAM is freed") is **wrong** for the same
  reason.

**Exposure:** orphaned in-distro agent serving any TCP peer reachable inside
the utility VM (cross-references HIGH-1). Reaper is operationally
ineffective.

**Mitigation:**
- Add a `Frame::Shutdown` to `sandbox-vm-protocol`; `stop_agent`
  (`wsl2.rs:558-562`) sends it + waits ≤2s, then `wsl --terminate`.
- Use `wsl --shutdown` only when probe (`wsl -l --running`) shows we own
  every running distro.
- Fix the misleading comment at `wsl2.rs:126-128`.
- Add a 60s self-shutdown to the agent itself if no command is in flight and
  the relay's stdin (when using stdio transport from HIGH-1 mitigation) has
  hit EOF.

---

### HIGH-4 — `/etc/sysctl.d/99-ziee-sandbox.conf` does NOT re-apply on VM restart

**Evidence:**
- [microsoft/WSL#4232](https://github.com/microsoft/WSL/issues/4232) (open
  since 2019): sysctl.conf values not applied on container start. No
  systemd-sysctl service in default WSL2 (systemd is opt-in via
  `/etc/wsl.conf`).
- Our `provision_distro` (`wsl2.rs:273-332`) writes `/etc/sysctl.d/...` AND a
  sentinel `/etc/ziee-sandbox-provisioned`. On VM restart, the sentinel
  still exists → provision is skipped → sysctls are NOT applied → bwrap
  `--unshare-user` fails on first `execute_command`.

**Exposure:** silent, intermittent breakage. The user's first
`execute_command` after a Windows reboot (or any `wsl --shutdown`) fails;
they have no actionable diagnostic.

**Mitigation:**
- Split `provision_distro` into `provision_persistent` (sentinel-guarded:
  apt + agent + identity + profile) and `apply_runtime_state` (run on EVERY
  `ensure_distro`: AppArmor profile load + sysctl re-apply).
- Write `/etc/wsl.conf` with `[boot] systemd=true` (or `command="aa-load
  /etc/apparmor.d/bwrap; sysctl --system"`) so the state re-applies at VM
  boot. Lands at `wsl2.rs:294-313`.
- Optional belt: `Wsl2Backend::run` re-probes
  `/proc/sys/kernel/unprivileged_userns_clone` before each `execute_command`
  and re-applies if missing.

---

### MED-1 — 9p attack surface on the `/mnt/<drive>` workspace bind

**Evidence:**
- [McAfee Labs research](https://www.mcafee.com/blogs/other-blogs/mcafee-labs/hunting-for-blues-the-wsl-plan-9-protocol-bsod/):
  crafted 9p messages from Linux side BSOD'd the Windows host kernel
  (`p9rdr.sys`). Direction: guest→host kernel.
- [Microsoft plan9 docs](https://github.com/microsoft/WSL/blob/master/doc/docs/technical-documentation/plan9.md):
  WSL2 `/mnt/<drive>` is served by plan9 over an hvsocket with a Windows
  kernel-mode redirector driver.
- [CVE-2026-43053](https://windowsnews.ai/article/cve-2026-43053-linux-xfs-crash-recovery-metadata-cleanup-risk-wsl-azure.417461):
  recent class confirming plan9/shared-folder drivers as the documented VM
  escape pathway in WSL2.
- We bind `/mnt/<drive>/...` into bwrap (`wsl2.rs:340-350`) with `--bind`
  (writable). Sandboxed code has free file ops against the 9p path.

**Exposure:** any 9p client/server bug in the WSL2 Linux kernel or
`p9rdr.sys` is reachable from inside the sandbox via normal file ops on
`/home/sandboxuser`. Materially worse than the Linux backend (no 9p) and
materially worse than macOS (virtio-fs, separate codebase).

**Mitigation:** the existing `WIN-TODO (perf)` at `wsl2.rs:340` already
flags moving the workspace onto the distro's ext4 and relaying file contents
via `tools/files.rs`. The **security argument** strengthens this: it's not
just a perf win, it eliminates a documented kernel-escape surface. Promote
WIN-TODO to TODO, schedule the work. Also: this echoes Claude Code's
explicit guidance to treat `/mnt/c/` as outside the sandbox boundary.

---

### MED-2 — No WSL version probe (CVE-2025-53788 mitigation)

**Evidence:**
- [CVE-2025-53788](https://msrc.microsoft.com/update-guide/vulnerability/CVE-2025-53788)
  (CVSS ~7.0): WSL2 kernel TOCTOU LPE; fixed in WSL ≥ 2.5.10 / 2.6.1.
- Our `probe_host` (`wsl2.rs:354-387`) checks only that `wsl --status`
  contains `"2"` — it does not parse `wsl --version`.

**Exposure:** users on un-updated WSL get direct guest→host SYSTEM EoP from
inside the sandbox.

**Mitigation:** parse `wsl --version` output; refuse registration on
< 2.5.10 (2.5 channel) or < 2.6.1 (2.6 channel). Lands at
`Wsl2Backend::probe_host` (`wsl2.rs:361-387`). Surface as a new
`ReadyError::WslVersionTooOld { found, required }` (parallel to the
Step 5 variants).

---

### MED-3 — `WSLENV` provisioning is a future-regression vector

**Evidence:**
- `wsl2.rs:317-324` sets `WSLENV=ZIEE_PASSWD:ZIEE_GROUP` to ferry the
  synthetic identity. Today these are non-secret literals.
- The mechanism is the
  [documented way to leak env vars into WSL](https://learn.microsoft.com/en-us/windows/wsl/filesystems#share-environment-variables-between-windows-and-wsl-with-wslenv).
  A future maintainer adding any credential to `WSLENV` silently leaks it.

**Exposure:** future regression only. Not exploitable today.

**Mitigation:** unit test at the `wsl2.rs` module asserting `WSLENV` value
is exactly `"ZIEE_PASSWD:ZIEE_GROUP"` and the env vars are exactly the
synthetic literals. Eliminate `WSLENV` entirely by writing the
passwd/group via a temp file → `wsl.exe cp` (no env crossing needed).

---

### LOW-1 — `kernel.unprivileged_userns_clone=1` may be a no-op

This sysctl is a Debian/Ubuntu downstream patch; WSL2's Microsoft kernel
may or may not include it. The `2>/dev/null || true` at `wsl2.rs:302`
already handles the missing-knob case. Comment at 300-302 is mildly
misleading; clarify.

---

### LOW-2 — Agent logs peer address on TCP

`sandbox-guest-agent/src/main.rs:155` logs the peer address. With TCP
transport (HIGH-1 unresolved), this is an info-leak about a potential
cross-distro attacker. Strip when the listener is TCP, OR delete with
the HIGH-1 stdio-transport switch.

---

### LOW-3 — `--share-net` + mirrored networking mode → sandbox reaches Windows-host `127.0.0.1`

**Evidence:** Microsoft Learn confirms mirrored mode (Win11 22H2+) bridges
`localhost` between the Windows host and the WSL2 VM. With `--share-net`,
sandboxed code can `curl http://127.0.0.1:5432` and hit unauthenticated
Windows-host dev services (Postgres, the Ziee server itself, etc.). The
Linux backend's `--clearenv`-based exfil defense doesn't help against
loopback-trust services.

**Mitigation:**
- Document this in `wsl2.rs:1-24` threat-model header (Linux's header
  mentions "host localhost" — WSL2's currently doesn't).
- `probe_host` reads `%USERPROFILE%\.wslconfig` and warns on
  `networkingMode=mirrored`.
- Long-term: `--unshare-net` + an egress-filtering proxy in the distro
  (Anthropic + Codex both ship one). Tracked separately as a
  cross-platform feature, not a WSL2-specific fix.

---

## Items confirmed safe / not exposed

- **`.tar.zst` artifact name (Step 3) is correct.** WSL `--import` natively
  supports `.tar.zst` from Windows 11
  ([microsoft/WSL#6056-class refs](https://github.com/microsoft/WSL/issues/6056)).
- **cgroup v2 inside WSL2** is the default since WSL 2.5.14; our
  `CgroupMode::None` + agent-side enforcement + prlimit backstop is correct.
  Older WSL versions (cgroup v1) gracefully fall through to the prlimit
  backstop.
- **Binding TCP to `127.0.0.1`** (not `0.0.0.0`) correctly prevents LAN-side
  attack. The HIGH-1 problem is intra-VM only.
- **Structured-JSON IPC immunizes us against
  [microsoft/vscode#316120](https://github.com/microsoft/vscode/issues/316120)**
  — the Copilot agent's "sandbox bypass via shell-quoting corruption"
  data-loss incident. Our agent receives frames, not inline shell
  strings; we cannot regress into it without removing the protocol. **Add
  a regression test** asserting no code path ever passes shell-meta argv
  through `wsl.exe -- bash -c <inline>` outside the provisioning step.

---

## Competitive comparison

| Tool | Windows isolation | Host↔executor auth | Public incidents |
|---|---|---|---|
| **Ziee (today)** | **WSL2 distro + bwrap** | **Raw TCP, no auth** ⚠ | None |
| Cursor | WSL2 + Linux sandbox | Not disclosed | User-visible regressions (git, creds) |
| Copilot / VS Code | WSL2 (no custom distro) | Inline `wsl.exe` argv | **#316120 — quoting-escape data loss** |
| **Codex (elevated)** | **Sandbox-user SIDs + WFP + private desktop** | **Named pipe + DACL + PID check** | None |
| Codex (WSL2 mode) | bwrap-in-WSL2 (like us) | (same) | (same) |
| Claude Code | **None** (PowerShell/Git Bash directly) | N/A | "Windows: Not yet supported" |
| Continue.dev | None (permission prompts) | N/A | — |
| Open Interpreter | None (`--safe` = semgrep, not isolation) | N/A | — |
| Aider | None (git worktrees as rollback) | N/A | — |
| AutoGen | Docker Desktop (HyperV/WSL2) | Docker named pipe | — |
| Semantic Kernel | Cloud-only (ACA Dynamic Sessions) | N/A | Prompt-injection→RCE in earlier releases |
| Jupyter | None | N/A | CVE-2025-53000, -2021-32797/32798 |
| Replit / Devin / Bolt | Cloud microVM | TLS to cloud | — |
| Dev Containers | Docker Desktop (WSL2 backend) | Docker named pipe (DACL'd) | — |

**Bottom line:** structurally identical to Cursor + Copilot (validated bet);
better than Claude Code / Continue / Aider / Open Interpreter (all have no
Windows sandbox); materially weaker than Codex's elevated mode (12k lines of
Rust we're not paying for). **Only Codex avoids unauthenticated transport**;
fixing HIGH-1 closes the one bug-class gap.

---

## Recommended fix order

| # | Finding | Effort | Where |
|---|---|---|---|
| 1 | **HIGH-1 — auth on agent transport** | M (1-2 days) | `sandbox-guest-agent/src/main.rs` + `wsl2.rs:229-244` |
| 2 | **HIGH-4 — sysctl persistence + AppArmor re-load on every boot** | S | `wsl2.rs:273-332` (split provision) |
| 3 | **HIGH-3 — reaper + agent-shutdown frame** | S | `sandbox-vm-protocol` (new frame) + `wsl2.rs:124-131,558-562` |
| 4 | **HIGH-2 — narrow AppArmor profile** | S | rootfs build + `wsl2.rs:294-313` |
| 5 | **MED-2 — WSL `--version` probe + new ReadyError** | S | `wsl2.rs:361-387` |
| 6 | **MED-1 — drop `/mnt/<drive>` 9p bind** | M (overlaps existing WIN-TODO) | `wsl2.rs:340-350` + new ext4 sync |
| 7 | **MED-3 — WSLENV regression test** | XS | new test in `wsl2.rs` |
| 8 | **LOW-1/2/3 — comment fixes + mirrored-mode warn-log + threat-model header update** | XS | `wsl2.rs:1-24, 300-302` |

**Plus one positive:** add the by-construction-immunity regression test for
the inline-`wsl.exe`-argv class of bugs that bit Copilot (#316120).

---

## Sources

[Full source list with ~30 URLs in the three agent transcripts; preserved
inline above by finding.]

Key references:

- microsoft/WSL source: [localhost.cpp](https://github.com/microsoft/WSL/blob/master/src/linux/init/localhost.cpp),
  [plan9.md](https://github.com/microsoft/WSL/blob/master/doc/docs/technical-documentation/plan9.md),
  [#4304](https://github.com/microsoft/WSL/issues/4304),
  [#4232](https://github.com/microsoft/WSL/issues/4232),
  [#13291](https://github.com/microsoft/WSL/issues/13291),
  [#8854](https://github.com/microsoft/WSL/issues/8854),
  [#1037](https://github.com/Microsoft/WSL/issues/1037).
- CVE family: [CVE-2025-9074](https://www.mindpatch.net/posts/docker-escape-ssrf/),
  [CVE-2025-53788](https://msrc.microsoft.com/update-guide/vulnerability/CVE-2025-53788),
  [CVE-2025-21756](https://www.wiz.io/vulnerability-database/cve/cve-2025-21756).
- OpenAI Codex Windows sandbox:
  [`windows-sandbox-rs/src/lib.rs`](https://github.com/openai/codex/tree/main/codex-rs/windows-sandbox-rs),
  [`runner_pipe.rs`](https://github.com/openai/codex/blob/main/codex-rs/windows-sandbox-rs/src/elevated/runner_pipe.rs).
- Anthropic Claude Code: [sandboxing docs](https://code.claude.com/docs/en/sandboxing),
  [sandbox-runtime](https://github.com/anthropic-experimental/sandbox-runtime).
- Cursor sandboxing: [blog](https://cursor.com/blog/agent-sandboxing).
- Copilot/VS Code: [trust + safety](https://code.visualstudio.com/docs/copilot/concepts/trust-and-safety),
  [#316120 quoting-escape](https://github.com/microsoft/vscode/issues/316120).
- McAfee 9p research: [WSL Plan 9 BSOD](https://www.mcafee.com/blogs/other-blogs/mcafee-labs/hunting-for-blues-the-wsl-plan-9-protocol-bsod/).
- Ubuntu noble AppArmor restriction: [Ubuntu blog](https://ubuntu.com/blog/ubuntu-23-10-restricted-unprivileged-user-namespaces),
  [Qualys bypasses](https://seclists.org/oss-sec/2025/q1/253).
