# Security Audit — Hardware Module

**Date:** 2026-05-23
**Scope:** `src-app/server/src/modules/hardware/` (~1,529 LOC) — HW detection (CPU/Memory/GPU/OS), real-time SSE usage monitoring, REST info endpoint
**Auditor:** Claude (general-purpose, ASVS-aligned review)
**Standard:** OWASP ASVS 4.0.3, Level 2 target
**Chapters in focus:** V4 (Access Control), V8 (Data Protection), V13 (API), V7 (Logging)
**Files examined:**

| File | LOC | Purpose |
|---|---|---|
| `mod.rs` | 81 | Module registration |
| `routes.rs` | 20 | Axum route wiring |
| `handlers.rs` | 149 | **Active** request handlers (REST + SSE) |
| `api.rs` | 110 | **DEAD** duplicate of handlers (never wired) |
| `monitoring.rs` | 158 | SSE broadcast loop + client registry |
| `detection.rs` | 863 | GPU/CPU/OS detection (calls `nvidia-smi`, `rocm-smi`, `intel_gpu_top`, `ioreg`, `sysctl`) |
| `permissions.rs` | 28 | `HardwareRead` / `HardwareMonitor` definitions |
| `types.rs` | 120 | DTOs |

---

## Executive Summary

The hardware module is **small and well-scoped** but ships **non-trivial DoS and information-disclosure risk** because of an unbounded SSE-client registry, a global `lazy_static` monitoring loop, complete absence of per-user / per-IP rate limiting, and an unauth-by-default OS/CPU/GPU listing that is gated only by `hardware::read` / `hardware::monitor`. Those two permissions are NOT in the default `Users` group seeded by migration 0001 — only `Administrators` (wildcard `*`) holds them out of the box — so the immediate exposure is bounded to admins. **However**, both permissions are normal RBAC permissions and any operator that adds them to a custom group (a reasonable thing to do for a "developer" or "ops" role) immediately inherits the DoS and side-channel surface.

The module reaches the host through **five distinct subprocess invocations** (`nvidia-smi`, `rocm-smi`, `intel_gpu_top`, `ioreg`, `sysctl`). All are spawned with **hardcoded program names and static argv arrays** — there is no user input flowing into argv and **no shell interpolation**, which is the right call. PATH-search is the only residual injection vector (an attacker who already owns `$PATH` has long since lost).

The module also leaks a **kernel version string** and **CPU brand string** unauth'd to anyone holding `hardware::read`, which is a moderate fingerprinting surface (CVE-matching tools can pivot from "Linux kernel 5.15.0-157-generic + Intel Xeon Platinum 8488C" to known kernel vulns), and on Linux it reads `/sys/class/drm` directly — bounded paths, no traversal, but the path strings themselves end up in responses (`AMD GPU {device_id_string}` is an `lspci`-equivalent leak).

Two correctness bugs (`available_ram = total_ram - used_ram` and `available_swap = total_swap - used_swap` in `monitoring.rs:102, 113`) can underflow if `sysinfo` ever returns inconsistent snapshots between calls, panicking the broadcast task in debug builds and silently wrapping to ~18 EB in release. This is a liveness bug more than a security one but it kills monitoring for *all* connected clients on one bad refresh.

A material amount of the audit is also taken up by `api.rs` — **110 lines of dead code** that exactly duplicates `handlers.rs` — and an unused-import re-export pattern (`#![allow(dead_code)]` at module top). Dead code is not a finding under ASVS but is flagged Info (F-09) because it doubles the surface a future refactor has to keep secure.

### Severity counts

| Severity | Count |
|---|---|
| Critical | 0 |
| High     | 2 |
| Medium   | 4 |
| Low      | 3 |
| Info     | 4 |

### Top three risks (act this sprint)

1. **F-01 — Unbounded SSE client registry + unbounded mpsc per client.** Any holder of `hardware::monitor` can open N parallel SSE streams; each registers a new `tokio::sync::mpsc::unbounded_channel()` in a process-wide `lazy_static! Mutex<HashMap>`. There is no per-user cap, no global cap, and no `keep_alive` to prune half-open connections. Combined with the global broadcast every 2 s, a single authenticated user (or a compromised account) can pin a `Mutex` on the broadcast hot path under contention from thousands of concurrent fake clients. (DoS, High)

2. **F-02 — Kernel version, CPU brand, GPU vendor/model, NVIDIA driver version exposed to every `hardware::read` holder.** This is the textbook fingerprinting surface (vuln-DB pivot point). The module surfaces `System::kernel_version()`, the CPU brand string verbatim, the NVIDIA driver version (used by CVE-2024-0090, CVE-2024-53869, etc.), and the GPU device path on Linux sysfs. Should be admin-only (`RequireAdmin`), not a normal RBAC permission. (Info disclosure, High — see ASVS V8.3.4 / V13.1.3)

3. **F-04 — Monitoring loop self-terminates on zero clients but the active-flag check races, allowing two parallel loops.** `start_hardware_monitoring()` checks `MONITORING_ACTIVE` then `drop`s the guard, then `tokio::spawn`s the loop body which sets the flag back to `false` when it observes zero clients. If client #1 disconnects (loop exits → flag=false) and client #2 reconnects before the spawn returns, the new spawn task starts a second loop while the first is still in `interval.tick().await`. Each broadcasts to the same registry. Effect: doubled CPU/sysinfo cost, doubled per-client message rate (downstream UI flicker). (Concurrency / resource amplification, Medium)

---

## Findings

### F-01 — Unbounded SSE client registry + unbounded per-client mpsc queue [HIGH]

- **Severity:** High
- **ASVS:** V13.1.3 (rate limiting / DoS), V11.1.4 (resource control)
- **CWE:** CWE-770 (Allocation of Resources Without Limits or Throttling), CWE-400 (Uncontrolled Resource Consumption)
- **Location:**
  - `src-app/server/src/modules/hardware/monitoring.rs:15-19` (the `lazy_static! SSE_CLIENTS: Mutex<HashMap<...>>`)
  - `src-app/server/src/modules/hardware/monitoring.rs:22-34` (`add_client` — no cap)
  - `src-app/server/src/modules/hardware/monitoring.rs:25` (`tokio::sync::mpsc::unbounded_channel()`)
  - `src-app/server/src/modules/hardware/handlers.rs:96-126` (handler `subscribe_hardware_usage` — no per-user limit)

**Description**

The SSE registry is structured as:

```rust
// monitoring.rs:15-19
lazy_static::lazy_static! {
    static ref SSE_CLIENTS:
        Mutex<HashMap<ClientId,
            tokio::sync::mpsc::UnboundedSender<Result<Event, axum::Error>>>>
        = Mutex::new(HashMap::new());
    static ref MONITORING_ACTIVE: Mutex<bool> = Mutex::new(false);
}

// monitoring.rs:22-34
pub fn add_client(client_id: ClientId)
    -> tokio::sync::mpsc::UnboundedReceiver<Result<Event, axum::Error>>
{
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    {
        let mut clients = SSE_CLIENTS.lock().unwrap();
        clients.insert(client_id, tx);   // <-- no cap, no per-user counter
    }
    println!("Added hardware monitoring client: {}", client_id);
    rx
}
```

And the handler:

```rust
// handlers.rs:96-126
pub async fn subscribe_hardware_usage(
    _auth: RequirePermissions<(HardwareMonitor,)>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, axum::Error>>>> {
    let client_id = Uuid::new_v4();
    let mut rx = add_client(client_id);          // <-- no per-user lookup
    start_hardware_monitoring().await;
    // ...
}
```

Three independent problems compound:

1. **No client cap.** A user with `hardware::monitor` can open thousands of SSE streams in parallel. Each costs one `mpsc` channel pair, one entry in the `HashMap`, one `Sse::new(stream)` task, one Tokio waker chain.
2. **`unbounded_channel()`.** If the SSE writer back-pressures (slow consumer, congested network), the broadcast loop keeps pushing 2 s ticks into the channel; nothing in this code drops messages. RAM grows linearly with `(slow_clients × backlog_depth × event_size)` until the OS OOM-kills.
3. **Single global `Mutex<HashMap>`.** Every 2 s, `broadcast_usage_update` does `let clients = SSE_CLIENTS.lock().unwrap(); clients.clone()` (monitoring.rs:130-133). Cloning the whole map under the lock holds the mutex for `O(N)` time. Concurrent `add_client` / `remove_client` calls serialize on it. At 10 k clients this becomes a measurable contention point.

The `RequirePermissions<(HardwareMonitor,)>` extractor (extractors.rs:36-160) hits the DB on every `from_request_parts` to load user + groups; that's a per-connect cost the attacker pays once, but once a stream is open it costs nothing on the attacker's side and ~constant CPU/RAM per stream on the server's.

**Exploitation**

Authenticated user U with `hardware::monitor` runs:

```bash
for i in $(seq 1 10000); do
  curl -N -H "Authorization: Bearer $TOKEN" https://host/api/hardware/usage-stream &
done
```

The 10 k goroutine/task explosion is hostile but recoverable. The real damage is that each held-open connection grows its `mpsc` queue forever if the TCP socket is throttled (e.g., behind a captive-portal tarpit). At ~80 bytes per `Event` × 30 events/min × 10 k clients × 30 min = ~700 MB of dangling channel buffer.

A second attacker variant: an authenticated user with `hardware::monitor` can mint short-lived streams in a loop:

```bash
while true; do
  curl -N -H "Authorization: Bearer $TOKEN" https://host/api/hardware/usage-stream \
    --max-time 1
done
```

This forces a fresh JWT validation + DB roundtrip + `Uuid::new_v4()` + `HashMap::insert`/`remove` (under the same global mutex) per request. It will pin the broadcast task and DB pool at the rate of `(your fd count) / 1 s`.

**Impact**

DoS on monitoring; with sufficient back-pressure, DoS of the entire process via OOM. Permission scope is `hardware::monitor` — held only by `Administrators` by default, so today's blast radius is "admin-only self-DoS", but as soon as the permission is delegated to a non-admin role it becomes a normal-user-DoS vector.

**Recommendation**

- Replace `unbounded_channel()` with `mpsc::channel(N)` (e.g., 64) and drop the oldest event on overflow.
- Add a per-user concurrent-connection cap (e.g., 4) keyed by `user.id` in a separate `HashMap<UserId, usize>`.
- Add a global cap (e.g., 256). Reject with `429 Too Many Requests`.
- Use `tokio::sync::RwLock` or `parking_lot::Mutex` so the read-path in `broadcast_usage_update` doesn't serialize against `add_client`.
- Add `Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))` so half-open connections actually drop.
- Move client ID minting + insertion **after** at least one successful write so back-pressure on `connected` event prevents wedged registrations.

---

### F-02 — OS / kernel / CPU / GPU model + driver version disclosure to non-root-admin holders [HIGH]

- **Severity:** High
- **ASVS:** V8.3.4 (sensitive data identification), V13.1.3 (excessive data exposure)
- **CWE:** CWE-200 (Information Exposure), CWE-201 (Insertion of Sensitive Information into Sent Data)
- **Location:**
  - `src-app/server/src/modules/hardware/handlers.rs:30-80` (entire `get_hardware_info`)
  - `src-app/server/src/modules/hardware/handlers.rs:38-42` (kernel + OS + arch)
  - `src-app/server/src/modules/hardware/detection.rs:138-178, 188-242` (NVIDIA driver version, CUDA version)
  - `src-app/server/src/modules/hardware/detection.rs:394-456` (AMD sysfs device path + ID)

**Description**

`GET /api/hardware` returns, to anyone holding `hardware::read`:

| Field | Source | Why it's sensitive |
|---|---|---|
| `operating_system.name` | `System::name()` | Distro fingerprint |
| `operating_system.version` | `System::os_version()` | "Ubuntu 22.04.4 LTS" — pivots to known CVE catalog |
| `operating_system.kernel_version` | `System::kernel_version()` | **The fingerprint** — e.g., "5.15.0-157-generic" is a one-shot lookup against Ubuntu USN bulletins |
| `operating_system.architecture` | `std::env::consts::ARCH` | Low |
| `cpu.model` | `cpus.first().brand()` | "Intel Xeon Platinum 8488C" → CVE-DB pivot for spec-execution / micro-arch flaws (Downfall, Reptar, etc.) |
| `cpu.cores`, `cpu.threads`, `cpu.base_frequency` | sysinfo | Indirect host capacity → inform sizing of resource-exhaustion attacks |
| `gpu_devices[].name` | NVML / sysfs | Useful for ML supply-chain inference (model size constraint) |
| `gpu_devices[].driver_version` | NVML `sys_driver_version()` | NVIDIA driver version maps directly to CVE-2024-0090, CVE-2024-53869, CVE-2025-23253, etc. |
| `gpu_devices[].compute_capabilities.cuda_version` | NVML | Same |

The default `Users` group in `migrations/00000000000001_initial_schema.sql:170-178` does **NOT** include `hardware::read`. So today, only the `Administrators` group (wildcard `*`) holds the permission. Disclosure to a root admin is acceptable.

The problem is the *shape* of the control: `hardware::read` is a normal hierarchical RBAC permission, suitable for delegation to any custom group. There is no signal in the schema that this permission is functionally root-only ("MUST NOT delegate"). An operator setting up an "engineering" or "ops" group will assume that adding `hardware::read` is a low-risk capability ("they just want to see what's installed"). It is not — it's the single most useful fingerprinting endpoint in the entire API.

ASVS V8.3.4 (L2): *"Verify that sensitive information contained within the URL or parameters such as personal data (e.g., name, surname, gender), session tokens, passwords or API keys is not logged."* and V13.1.3 (L2): *"Verify that the API endpoint enforces appropriate sensitive data exposure protections."*

**Exploitation**

Attacker compromises any account in a custom group an operator has delegated `hardware::read` to. Issues:

```bash
curl -H "Authorization: Bearer $TOKEN" https://host/api/hardware
```

Receives:
```json
{
  "hardware": {
    "operating_system": {
      "name": "Ubuntu",
      "version": "22.04.4 LTS",
      "kernel_version": "5.15.0-157-generic",
      "architecture": "x86_64"
    },
    "cpu": { "model": "Intel(R) Xeon(R) Platinum 8488C", "cores": 96, ... },
    "gpu_devices": [
      { "name": "NVIDIA H100 80GB HBM3", "driver_version": "535.183.01",
        "compute_capabilities": { "cuda_version": "12.2", ... } }
    ]
  }
}
```

Now the attacker:
1. Searches Ubuntu USN for unpatched advisories against `linux-5.15.0-157`.
2. Searches NVIDIA security bulletins for driver 535.x flaws.
3. Cross-references CVE-2024-0090 (NVIDIA driver privilege escalation) — *if* matched, the host is a one-shot kernel-mode exploit away.

**Impact**

Direct hand-off to known-exploit pivoting. Doesn't grant code execution by itself but is a force-multiplier for the next stage. ASVS deems this a Level-2 control gap.

**Recommendation**

- **Preferred:** Change the extractor to `RequireAdmin` (or a new `HardwareReadAdmin` permission that the README explicitly documents as "root-admin-only — do not delegate").
- **Alternative:** Tier the response. A lower permission (`hardware::summary`) returns coarse capacity (RAM tier, CPU class, "has GPU: true") without kernel/driver/model strings. The full detail stays behind admin.
- **Either way:** Stop returning `kernel_version` and `driver_version` to non-admins. Redact `cpu.model` to a coarse vendor + family ("Intel Xeon" not "Intel Xeon Platinum 8488C").
- Apply the same redaction to the SSE `update` event's `device_name` field (currently echoes the full GPU model on every 2 s tick).

---

### F-03 — Hardware monitoring loop has zero rate-limiting or back-pressure on input [HIGH→MED] — keep as **Medium**

- **Severity:** Medium
- **ASVS:** V13.1.3 (rate limiting)
- **CWE:** CWE-307 (Improper Restriction of Excessive Authentication Attempts) — analogously to "excessive connect attempts"
- **Location:** `handlers.rs:96-126`, `monitoring.rs:44-85`

**Description**

There is no per-IP, per-user, or per-endpoint rate limit anywhere in the server (this is a known global gap; see `.sec-audits/2026-05/14-core-infrastructure.md` F-01-class findings). For the hardware endpoints specifically the symptom is:

1. `GET /api/hardware` spawns a `System::new_all()` + `sys.refresh_all()` on every request. `refresh_all` walks every `/proc/<pid>/*` on Linux — for a host with 4 k processes this is ~tens-of-ms of syscalls per request. Plus the NVML init (`detect_gpu_devices`) which is non-trivial.
2. `GET /api/hardware/usage-stream` triggers `start_hardware_monitoring()` which spawns the global 2 s loop the first time.

A `hardware::read`-holder can issue:
```bash
ab -n 100000 -c 100 -H "Authorization: Bearer $TOKEN" https://host/api/hardware
```
and pin one `tokio` worker per connection in `refresh_all`. Combined with F-01, an authenticated attacker can degrade the server.

**Exploitation**

Same as above — N parallel requests, each costing tens of ms in kernel-side `/proc` reads.

**Impact**

CPU pressure, kernel syscall pressure, increased latency for legitimate traffic. Not a server kill but a noticeable degradation. Mitigated by F-02 if the permission is admin-only — but only by trust, not by code.

**Recommendation**

- Cache the static hardware-info response in-process for ~5 minutes. CPU model, RAM total, GPU model, kernel version don't change at runtime; recomputing them per request is waste.
- Apply a per-route rate limit (e.g., `tower-governor`) on `/api/hardware*` endpoints when the global rate-limit middleware is added (this is the recommended fix in `14-core-infrastructure.md`).

---

### F-04 — Monitoring `MONITORING_ACTIVE` flag has a TOCTOU race spawning duplicate loops [MEDIUM]

- **Severity:** Medium
- **ASVS:** V1.11.3 (concurrency), V11.1.6 (race conditions)
- **CWE:** CWE-362 (Race Condition), CWE-665 (Improper Initialization)
- **Location:** `src-app/server/src/modules/hardware/monitoring.rs:44-85`

**Description**

```rust
// monitoring.rs:44-85
pub async fn start_hardware_monitoring() {
    let mut monitoring_active = MONITORING_ACTIVE.lock().unwrap();
    if *monitoring_active {
        return;
    }
    *monitoring_active = true;
    drop(monitoring_active);           // <-- lock released here

    tokio::spawn(async {               // <-- spawn happens after release
        let mut interval = interval(Duration::from_secs(2));
        let mut sys = System::new_all();
        loop {
            interval.tick().await;
            let client_count = { SSE_CLIENTS.lock().unwrap().len() };
            if client_count == 0 {
                let mut monitoring_active = MONITORING_ACTIVE.lock().unwrap();
                *monitoring_active = false;     // <-- flag flipped back to false
                break;
            }
            sys.refresh_all();
            broadcast_usage_update(collect_hardware_usage(&mut sys)).await;
        }
    });
}
```

The intended invariant is "at most one loop runs". The actual behaviour:

1. Client A connects → `add_client(A)`, `start_hardware_monitoring()` → flag=true, spawns loop L1.
2. Client A disconnects → registry sees 0 clients → on the *next tick* L1 sets flag=false, breaks.
3. **Between L1 setting flag=false and L1's spawn task actually terminating** (Tokio runtime hasn't yet collected the task), client B connects → `add_client(B)`, `start_hardware_monitoring()` → flag is `false`, sets it back to true, spawns L2.
4. L1 is still alive (just past `break`), L2 is freshly spawned. Both eventually share the same `SSE_CLIENTS` registry; L2 will tick and broadcast normally, L1 will exit on next-loop-iteration which has already exited.

This case is benign in isolation (L1 exits immediately), but the race can compound: between L1's "I observed 0 clients" and L1's "I now set flag=false and break", an `add_client(C)` can land. Now client C is registered but L1 has already decided to exit, so until the next `start_hardware_monitoring()` call (which won't happen — the connection is already open!), there is NO broadcast loop and client C will only see the "Connected" event followed by silence forever.

The bug is on the disconnect-side timing: when L1 observes 0 clients, it must:
1. Re-take the registry lock
2. Re-check the count (still 0)
3. Set flag=false **while holding the registry lock**
4. Break

Without that atomicity, the race holds.

**Exploitation**

Not an exploitable security flaw on its own — it's a liveness bug — but it can be triggered probabilistically by an authenticated attacker connecting/disconnecting rapidly. The observable effect is "monitoring silently stops working", which has user-trust implications for an admin dashboard.

**Impact**

Silent loss of monitoring. Possible doubled loops with extra CPU cost.

**Recommendation**

Replace the two-mutex pattern with a single `AtomicBool` flipped via CAS, OR fold both flags under a single `Mutex<MonitoringState>` and never drop the lock between "I observed 0 clients" and "I set the flag false". `tokio::sync::Mutex` would be better here than `std::sync::Mutex` since the broadcast tick is async.

---

### F-05 — `available_ram` / `available_swap` arithmetic underflow [MEDIUM]

- **Severity:** Medium
- **ASVS:** V5.2.1 (sanitization), V11.1.6 (numeric)
- **CWE:** CWE-191 (Integer Underflow)
- **Location:** `src-app/server/src/modules/hardware/monitoring.rs:100-115`

**Description**

```rust
// monitoring.rs:100-115
let total_ram = sys.total_memory();
let used_ram = sys.used_memory();
let available_ram = total_ram - used_ram;     // <-- panic / wrap if used > total

let memory = MemoryUsage {
    used_ram,
    available_ram,
    used_swap: Some(sys.used_swap()),
    available_swap: Some(sys.total_swap() - sys.used_swap()),  // same risk
    usage_percentage,
};
```

`sysinfo` documents `used_memory()` as "the amount of memory currently in use" and `total_memory()` as "the total amount of RAM". On Linux these come from `/proc/meminfo`, but they are computed across two different `refresh_*` calls (refresh_memory snapshots independently). In rare circumstances (large balloon driver in a VM, swap thrashing, MEMORY_PRESSURE_NOTIFY) `used_memory()` reads at one tick and `total_memory()` at another tick can briefly cross, especially in containers where cgroup memory ceilings are dynamic.

If `used_ram > total_ram` even momentarily:

- **Debug builds:** `attempt to subtract with overflow` panic — kills the broadcast task. F-04 then matters: the loop dies, and no new loop is spawned until the next first-client-after-zero. Until then, every connected client stops getting `update` events.
- **Release builds:** Wraps to `~18 EB`. Marshalled as JSON, sent to UI, UI displays "available: 18,446,744,073,709,551,604 bytes" until the next tick. Cosmetic, but degrades trust.

Same applies to `total_swap() - used_swap()` on a host with no swap (`total_swap() == 0`, `used_swap()` arguably also 0, but kernel + sysinfo + cgroup can disagree).

**Exploitation**

Not directly exploitable; depends on host memory state. But trivial to make worse: trigger it deliberately in a debug build to break the loop.

**Impact**

In debug: broadcast loop dies → loss of monitoring. In release: garbage data shipped to clients.

**Recommendation**

Use `saturating_sub`:

```rust
let available_ram = total_ram.saturating_sub(used_ram);
let available_swap = sys.total_swap().saturating_sub(sys.used_swap());
```

---

### F-06 — Subprocess invocation: PATH-based binary resolution, no absolute path [LOW]

- **Severity:** Low
- **ASVS:** V14.1.5 (deployment hygiene), V5.3.8 (subprocess arg construction)
- **CWE:** CWE-426 (Untrusted Search Path), CWE-427 (Uncontrolled Search Path)
- **Location:**
  - `detection.rs:188` `Command::new("nvidia-smi")`
  - `detection.rs:206` `Command::new("nvidia-smi").args(&["--query-gpu=..."])`
  - `detection.rs:337` `Command::new("rocm-smi").args(&["--showuse", "--showmeminfo", ...])`
  - `detection.rs:474` `Command::new("intel_gpu_top").arg("-J").arg("-s").arg("1000")`
  - `detection.rs:617` `Command::new("ioreg").args(&["-c", "AGXAccelerator", "-r", "-d1"])`
  - `detection.rs:723, 851` `Command::new("sysctl").arg("-n").arg(...)`

**Description**

All subprocesses are spawned by **program name** (not absolute path), which means resolution goes through `$PATH`. The server is normally not run with a hostile `$PATH`, but:

- If the server process is started by an init system that injects an unusual PATH, OR
- If the server runs in a container where `/usr/local/bin` is writable by a less-trusted process,

an attacker could plant a malicious `nvidia-smi`, `rocm-smi`, or `sysctl` binary that the hardware module would execute. The attacker would need pre-existing FS write access to a PATH entry — this is largely a defence-in-depth concern, not a primary vector.

**Positive findings (and the reason this is LOW not HIGH):**

- **No shell.** All calls are `Command::new(...)` direct-exec. No `Command::new("sh").arg("-c")`, no shell escape.
- **No user input in argv.** Every `.args(&[...])` is a static array of literal strings. There is no way for a remote user to influence what flags `nvidia-smi` is called with.
- **No environment passthrough hardening missing.** (`std::process::Command` inherits env by default; nothing is wiped.) Not critical but worth noting — if `LD_PRELOAD` is set in the parent, it propagates to the child.

**Exploitation**

Local attacker with FS write to a PATH dir plants a shim. Not a remote attack.

**Impact**

Code execution as the server user. But only via a precondition (FS write to PATH) that already implies similar or greater compromise.

**Recommendation**

- For each binary, configure an absolute path through the config (e.g., `/usr/bin/nvidia-smi`, `/usr/bin/sysctl`). Default to the absolute path; fall back to PATH search only with a warning.
- Add `.env_clear()` followed by `.env("PATH", "/usr/bin:/usr/local/bin")` on each `Command` to keep the child env tight and remove `LD_PRELOAD`.

---

### F-07 — `println!()` instead of `tracing` for SSE lifecycle logging [LOW]

- **Severity:** Low
- **ASVS:** V7.1.1 (logging output), V7.1.2 (log level discipline)
- **CWE:** CWE-778 (Insufficient Logging)
- **Location:**
  - `monitoring.rs:32, 40, 52, 69` — all use `println!("Added hardware monitoring client: {}", client_id)` etc.
  - `handlers.rs:121` `println!("Hardware monitoring client disconnected: {}", client_id);`

**Description**

The rest of the codebase uses `tracing::info!` / `tracing::warn!` / `tracing::error!`. The hardware module reverts to `println!`. Effects:

1. **No structured fields** — the `client_id` (a UUID) ends up as raw text in stdout, not as a tagged field, so log aggregators can't index it.
2. **No log level filtering** — these always print, regardless of `RUST_LOG`.
3. **Stdout-only** — if the server is run as a systemd unit with `StandardOutput=journal`, these go to the journal; with `StandardOutput=null`, they're lost; with `StandardOutput=file`, they bypass tracing's rotation.
4. **No SSE-disconnect distinguishability** — both `add_client` and `remove_client` print at the same level. The "client disconnected" line (handlers.rs:121) is the only signal that a connection ended, but it's a debug detail, not an info-level event.

**Exploitation**

Not exploitable.

**Impact**

Defensive — slows post-incident forensics; obscures DoS attempts that would otherwise show as "10 k SSE connections from same `user_id` in 60 s" if structured.

**Recommendation**

Replace each `println!` with the matching `tracing::debug!` / `tracing::info!`. Include `user_id` (currently the handler discards `_auth.user.id` — pass it into `add_client` so logs identify which authenticated user owns each connection).

---

### F-08 — `hardware/types` endpoint reachable, returns 500/unreachable [LOW]

- **Severity:** Low
- **ASVS:** V13.1.1 (API surface minimization)
- **CWE:** CWE-489 (Active Debug Code)
- **Location:** `handlers.rs:141-149`, `routes.rs:16-19`

**Description**

```rust
// handlers.rs:141-143
pub async fn hardware_types() -> Json<HardwareUsageUpdate> {
    unreachable!("This endpoint is only for OpenAPI type generation")
}
```

This is registered at `GET /api/hardware/types`:
```rust
// routes.rs:16-19
.api_route("/hardware/types", get_with(hardware_types, hardware_types_docs),
```

The handler has **no auth extractor**. The route is wired into the live router (not behind a `cfg(debug_assertions)` flag or feature gate). Hitting `GET /api/hardware/types` on a production server will panic in handler context, which Axum catches as a 500 — but the panic still walks the stack, logs the panic message, and is gratuitous.

If an attacker has discovered the URL (it's in the OpenAPI spec which the UI fetches), they can spam it to fill logs / waste compute on stack-unwinding.

**Note:** The `hardware_types` endpoint takes NO permission extractor — so unlike every other route, this one is **anon-reachable** before it panics. The 500 happens after the request reaches the handler.

**Exploitation**

```bash
while true; do curl https://host/api/hardware/types; done
```

Fills logs with panic stacks. Each panic forces tracing-subscriber to format and emit the panic.

**Impact**

Log volume amplification; cosmetic; possible degradation of structured-log indices.

**Recommendation**

- Either gate the route behind `#[cfg(any(debug_assertions, feature = "openapi-gen"))]` so it's not present in release builds, OR
- Make `hardware_types` return `StatusCode::NOT_FOUND` instead of `unreachable!()`. (The dummy is only there to make `aide`'s OpenAPI generator pick up the `HardwareUsageUpdate` type — that goal doesn't require the route to actually be live.)

---

### F-09 — `api.rs` is 110 lines of dead duplicate code [INFO]

- **Severity:** Info
- **ASVS:** V14.2.2 (unused dependencies / dead code)
- **CWE:** CWE-1041 (Use of Redundant Code)
- **Location:** `src-app/server/src/modules/hardware/api.rs` (entire file)

**Description**

`api.rs` is declared in `mod.rs:8` (`pub mod api;`) and starts with `#![allow(dead_code)]`. It defines `get_hardware_info` and `subscribe_hardware_usage` — functions that are **never imported anywhere else in the codebase** (`grep -rn "hardware::api" src/` returns zero hits). `routes.rs` uses `super::handlers::*`, not `super::api::*`.

The functions in `api.rs` are exact behavioural duplicates of `handlers.rs:30-126` (line-by-line identical apart from a missing `subscribe_hardware_usage_docs` and the `hardware_types` endpoint).

**Impact**

- Future refactors that fix a security bug in `handlers.rs` (e.g., F-01 or F-02) will leave the bug intact in `api.rs`. If `api.rs` is ever wired into routes by accident, the fix is silently reverted.
- Double the surface to audit; doubles the surface to keep in sync.

**Recommendation**

Delete `src/modules/hardware/api.rs` and remove the `pub mod api;` line from `mod.rs:8`. The dummy `hardware_types` endpoint (F-08) already lives in `handlers.rs` and that's the only role `api.rs` could be filling.

---

### F-10 — SSE stream lacks `keep_alive`, half-open connections accumulate [INFO]

- **Severity:** Info
- **ASVS:** V11.1.4 (resource control)
- **CWE:** CWE-404 (Improper Resource Shutdown)
- **Location:** `handlers.rs:125`, `api.rs:109` (dead duplicate)

**Description**

The SSE response is created as `Sse::new(stream)`. There is no `.keep_alive(KeepAlive::new()...)`. This means:

- A TCP connection that died silently (NAT timeout, client crash, network partition) will not be observed by the server until the next 2-second broadcast pushes an event into the mpsc channel AND the channel's `send` returns `Err`. That happens when the receiver is dropped, which happens when the `async_stream::stream!` polls `rx.recv().await` and gets `None`, which only happens after the underlying `Sse` body writer hits a TCP error, which only happens on the *next* write.
- In other words, the server-side detection latency is ~2 seconds for a dead client. Not terrible, but worse: behind an idle-NAT (typical timeout 30 s for UDP, 5 min for TCP) the TCP send may simply queue in the kernel for minutes before the FIN/RST flows back, prolonging the half-open registration.

With `KeepAlive`, Axum injects an SSE comment line at a configurable interval that triggers detection.

**Recommendation**

```rust
use axum::response::sse::KeepAlive;
Ok((StatusCode::OK,
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))))
```

---

### F-11 — Linux `/sys/class/drm` reads not sandboxed against device-path strings in response [INFO]

- **Severity:** Info
- **ASVS:** V12.3.2 (filesystem traversal hygiene)
- **CWE:** CWE-552 (Files or Directories Accessible to External Parties)
- **Location:** `detection.rs:394-456` (AMD sysfs), `detection.rs:508-540` (Intel sysfs)

**Description**

```rust
// detection.rs:394-455
if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("card") && !name.contains("-") {
                let device_path = format!("/sys/class/drm/{}/device", name);
                if let Ok(vendor) = std::fs::read_to_string(format!("{}/vendor", device_path)) {
                    if vendor.trim() == "0x1002" {
                        let device_name = std::fs::read_to_string(format!("{}/device", device_path))
                            .ok()
                            .map(|d| format!("AMD GPU {}", d.trim()))
                            ...
```

The directory traversal is **bounded** to `/sys/class/drm` and uses `entry.path().file_name()` so it cannot escape via symlink — `std::fs::read_dir` follows symlinks but `entry.path()` returns the link target name not the link content. The `name.starts_with("card") && !name.contains("-")` filter then further constrains it.

So there is no path traversal vulnerability per se.

What *is* worth noting: the device ID read from `/sys/class/drm/cardN/device/device` is interpolated raw into the JSON response as `format!("AMD GPU {}", d.trim())`. The device ID is a 4-hex-digit PCI device ID set by the kernel — sysinfo here trusts the kernel completely, which is correct. But if some future change adds (say) the GPU's `subsystem_vendor` or `serial`, that would expose host-fingerprintable identifiers without an extra audit hop. Document the trust boundary in code.

**Recommendation**

Add a code comment near the sysfs reads: "// Output of this block is shipped verbatim to clients via the /api/hardware response — any field added must not contain serials, MAC addresses, or other host identifiers."

---

## ASVS Coverage Matrix

| Control | Target | Status | Finding | Notes |
|---|---|---|---|---|
| V1.4.2 — Trust boundaries documented | L2 | ⚠️ Partial | F-11 | Sysfs trust boundary is implicit |
| V4.1.1 — Permission enforced on every route | L2 | ✅ Pass | — | `RequirePermissions<(HardwareRead,)>`/`(HardwareMonitor,)` on the two real routes |
| V4.1.2 — Default-deny on auth missing | L2 | ✅ Pass | — | `RequirePermissions` extractor rejects with 401/403 |
| V4.1.5 — Re-authentication on long sessions | L2 | ⚠️ N/A here | — | SSE re-auths each NEW connection (no token rotation mid-stream — see below) |
| V4.2.1 — Sensitive data not exposed in API | L2 | ❌ Fail | F-02 | kernel/CPU/driver versions returned to any `hardware::read` holder |
| V4.3.1 — Admin-only ops protected | L2 | ❌ Fail | F-02 | `hardware::read` shape encourages delegation |
| V7.1.1 — Logging uses uniform infra | L2 | ❌ Fail | F-07 | `println!` instead of `tracing` |
| V7.1.3 — Logs include identity | L2 | ❌ Fail | F-07 | `user_id` not in SSE lifecycle logs |
| V8.3.4 — Sensitive data identified | L2 | ❌ Fail | F-02 | Kernel/driver/CPU model not classified as sensitive |
| V11.1.4 — Resource limits applied | L2 | ❌ Fail | F-01, F-10 | No client cap, no keep_alive |
| V11.1.6 — Race-condition free | L2 | ❌ Fail | F-04, F-05 | TOCTOU on `MONITORING_ACTIVE`; integer underflow |
| V12.3.2 — Path traversal prevented | L2 | ✅ Pass | F-11 (Info) | `/sys/class/drm` reads bounded |
| V13.1.1 — API surface minimized | L2 | ❌ Fail | F-08, F-09 | `hardware/types` route reachable; `api.rs` dead duplicate |
| V13.1.3 — Excessive data exposure | L2 | ❌ Fail | F-02 | Same as V4.2.1 |
| V14.1.5 — Subprocess hygiene | L2 | ⚠️ Partial | F-06 | No absolute paths, no env wipe — but no shell + no user input |
| V14.2.2 — Dead code removed | L2 | ❌ Fail | F-09 | `api.rs` (110 LOC) |

---

## Positive Findings

1. **Permission extractor on every live route.** Both `get_hardware_info` and `subscribe_hardware_usage` use `RequirePermissions<...>` correctly. The one route without it (`hardware_types`) is a dummy intended only for OpenAPI generation. (F-08 still flags that the dummy should be gated.)
2. **Default `Users` group does NOT grant `hardware::read` or `hardware::monitor`.** Confirmed by reading `migrations/00000000000001_initial_schema.sql:170-178`. Today the blast radius is admin-only — F-02's High rating reflects future delegation risk, not present exposure.
3. **All subprocess argv arrays are static literals.** No user input is ever passed to `nvidia-smi`, `rocm-smi`, `intel_gpu_top`, `ioreg`, or `sysctl`. No shell interpolation anywhere. This is the right pattern for subprocess hygiene; F-06 is purely a defence-in-depth concern.
4. **No subprocess output is reflected verbatim into responses (mostly).** `nvidia-smi` CSV is parsed field-by-field with type casts (`parse::<u32>()`, `parse::<u64>()`), so a hostile binary could not inject string payloads through the model name — except via `parts[1]` for the device `name` field which is shipped raw. Even then, the JSON serializer would safely escape any special chars, so the only attack surface is "very long device names" causing memory bloat — bounded by `nvidia-smi` itself.
5. **No raw filesystem path is constructed from request data.** The sysfs reads (`/sys/class/drm/...`) are entirely server-driven — no user input crosses into a path string.
6. **SSE auth is re-validated on every reconnect.** Because the UI uses fetch-based SSE with `Authorization: Bearer <token>`, the `RequirePermissions` extractor runs from scratch on each `subscribe_hardware_usage` invocation. There is no implicit "session credit" that survives JWT rotation. Token revocation propagates on next reconnect.
7. **The auth extractor checks `user.is_active` before granting permissions.** A deactivated user holding a still-valid (but unrotated) JWT is rejected at the extractor level — relevant for SSE because the auth check happens *only at connect*, so without the `is_active` check, a deactivated user with a long-lived token could keep an SSE stream alive.
8. **CPU temperature is intentionally not returned.** `monitoring.rs:95` sets `temperature: None` with the comment "sysinfo doesn't provide CPU temperature on all platforms". Temperature would otherwise be a side-channel for inferring host load patterns (and indirectly, what models are being served).
9. **No PII at all in the response surface.** No usernames, no IPs, no hostnames, no MAC addresses (modulo F-11's note about future drift). The module surfaces hardware capability only.

---

## Out of Scope / Deferred

- **Global rate-limit / body-limit middleware.** Tracked in `.sec-audits/2026-05/14-core-infrastructure.md`. The hardware module benefits from any solution there but doesn't own the fix.
- **JWT mid-stream rotation for SSE.** Because the hardware SSE only re-validates auth at connect, a token revoked while a stream is open continues to receive updates until the user disconnects. This is a global SSE pattern and is being tracked under the MCP/chat SSE audits. Not module-specific.
- **`tracing` subscriber configuration.** The decision to use `println!` is a hardware-module-local lapse (F-07), but the *overall* tracing/log-rotation/audit-log story is core infra.
- **Sandbox-related hardware paths.** Per scope, the sandbox's hardware-touching code (`code_sandbox/*`) is audited separately.
- **GPU metric privacy (Tier-5 LLM-in-sandbox cost-inference).** Beyond pure-host fingerprinting (F-02), GPU utilization can side-channel which model is being served (small models tickle the GPU at one cadence, 70B models at another). Out of scope for ASVS Level 2; flagged for Level 3 considerations.

---

## Recommended remediation order

1. **F-02** (kernel/CPU/driver disclosure) → wrap with `RequireAdmin` OR add a documented "do not delegate" warning on the permission. Trivial code change, high impact.
2. **F-01** (unbounded SSE) → bound the channel, add per-user cap, add keep_alive. Med code change, high impact.
3. **F-05** (underflow) → `saturating_sub`. One-line change. Do this first if you do nothing else.
4. **F-04** (monitoring loop race) → fold the two flags under one mutex.
5. **F-08, F-09, F-10** (dead code + dummy types route + keep_alive) → housekeeping, low effort.
6. **F-06, F-07, F-11** → defence-in-depth.

---

**End of report.**
