# DECISIONS — sandbox-rootfs-list

### DEC-1: How is the "why not initialized" reason exposed — reuse the 503 (with details) or a dedicated 200 field?
**Resolution:** Return **200** for the LIST path with a dedicated `availability: SandboxAvailability` enum field on `VersionStatus`. The 503 is reserved for the mutating paths (install/set-pin/delete) that genuinely need a live pool.
**Basis:** codebase + brief — the brief prefers "a dedicated status/capability field rather than reusing the 503 for the list path"; `version_manager::status()` already tolerates `get_state()==None`, so a 200 degrade is the minimal, honest surface.

### DEC-2: How is `available_only` made unit-testable given `list_releases()` hits the network?
**Resolution:** Split a pure `build_degraded(availability, available: Vec<RootfsRelease>) -> VersionStatus` from the async `available_only(availability)` wrapper (which calls `list_releases().await.unwrap_or_default()`). Unit-test `build_degraded`; the handler calls `available_only`.
**Basis:** convention — mirrors the codebase's separation of pure builders from IO (e.g. `apply_project_context` is pure + unit-tested); avoids a network-dependent unit test.

### DEC-3: What runtime privilege does the container overlay grant, and what if real exec is still blocked here?
**Resolution:** `docker-compose.sandbox.yaml` grants **minimal caps** — `devices: ["/dev/fuse"]`, `cap_add: ["SYS_ADMIN"]`, `security_opt: ["apparmor:unconfined","seccomp:unconfined"]` — plus `ZIEE_CODE_SANDBOX_ENABLED: "true"`. The README documents `privileged: true` as the fallback for hosts that restrict unprivileged userns. Registration + the graceful list do NOT need these; only real sandboxed exec does. If exec is blocked in this environment, document exactly what is missing rather than forcing it.
**Basis:** user — chose "minimal caps default + documented privileged fallback" via AskUserQuestion.

### DEC-4: Which enum variant maps to each init early-return?
**Resolution:** disabled-in-config → `DisabledInConfig`; `probe_host()==None` → `HostUnsupported`; IMDS-reachable-and-not-opted-in → `CloudImdsRefused`; workspace-dir create failure → `WorkspaceInitFailed`; state-present-but-pool-None → `PoolMissing`; never-reached/unknown → `NotInitialized` (default). `Ready` on full success.
**Basis:** codebase — one variant per confirmed early-return site in `mod.rs::init` (:194/:213/:233/:244) + the `SANDBOX_POOL_MISSING` degenerate case in `live_pool()`.

### DEC-5: One PR or two?
**Resolution:** ONE PR (`fix/sandbox-rootfs-list` → `main`) covering both the app-code graceful-degrade and the docker opt-in.
**Basis:** brief — leaves it to the author; the two halves are the before/after of the same screen, small and cohesive.

### DEC-6: Frontend copy per availability variant?
**Resolution:** A `Record<SandboxAvailability, string>` in the section maps each variant to a short admin-facing sentence (e.g. disabled → "Code sandbox is disabled — set `code_sandbox.enabled: true` (and install bubblewrap on the host) to install and mount a rootfs."). Keep copy in the UI layer, not the server.
**Basis:** convention — the module's other notices keep copy inline in the component (i18n-friendly); the server sends only the machine-readable enum.
