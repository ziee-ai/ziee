# Plan 4 G7 / G8 — disposition re-evaluation (2026-05-23)

The original Plan 4 marked G7 (`MemoryDenyWriteExecute`) and G8 (time
namespace) as **"Out of scope (documented, not done)"**. After re-evaluating
in light of what shipped on `feat/sandbox-post-merge-followups` (MED-1 +
§6 VM sizing + the merged settings page), the disposition stands. This
file captures the trade-off so a future contributor doesn't re-derive it.

---

## G7 — MemoryDenyWriteExecute (MDWX)

**What it would buy.** Block any memory page from being simultaneously
`PROT_WRITE` and `PROT_EXEC`. Mitigates JIT spraying / shellcode injection
via writable code pages — a real and well-understood attack class.

**Concrete implementation shape.** Three syscall rules added to the
existing seccomp policy in `sandbox-seccomp/src/lib.rs`, using
`SCMP_CMP_MASKED_EQ` to test the `prot` argument bit pattern:

```rust
// PROT_WRITE = 0x2, PROT_EXEC = 0x4 → combined mask = 0x6.
// Reject any call asking for both.
let wx = ScmpArgCompare::new(2, ScmpCompareOp::MaskedEqual(0x6), 0x6);
ctx.add_rule_conditional(ScmpAction::Errno(EPERM),
    ScmpSyscall::from_name("mmap")?,         &[wx])?;
ctx.add_rule_conditional(ScmpAction::Errno(EPERM),
    ScmpSyscall::from_name("mprotect")?,     &[wx])?;
ctx.add_rule_conditional(ScmpAction::Errno(EPERM),
    ScmpSyscall::from_name("pkey_mprotect")?,&[wx])?;
```

Small surface, low overhead. Optional `mremap` rule for the
`MREMAP_DONTUNMAP` + prot-change shape; skipped for first cut.

**Why it's deferred.** The `full` flavor's payload includes **Node.js**,
whose V8 engine relies on writable+executable JIT pages (or W^X with a
specific `mprotect` toggle pattern). Enforcing MDWX globally would break
every Node-based workload in the sandbox; the `minimal` flavor (just
python3) would survive but the `full` flavor would not. Workloads using
PyPy / Numba / `numba @jit` would also break — uncommon in the default
`full` package set but plausible for R+pandas-style mixed workflows.

**What would unblock it.** Either:
1. A per-flavor opt-out (the seccomp policy lives in the shared
   `sandbox-seccomp` crate; the bwrap argv already references the policy
   via a runtime fd — making MDWX a build-time const that flavor recipes
   can override is a ~20-line change). Default off; admins enable for
   flavors known not to JIT.
2. A `code_sandbox_settings.seccomp_mdwx_enabled BOOLEAN DEFAULT FALSE`
   knob + a §6 UI toggle. Simpler from a config standpoint, but the
   admin needs to know which flavors tolerate it.

**Recommended disposition.** Stay deferred. Implement when a real
prompt-injection-via-shellcode pattern surfaces in audit logs, OR when
a user explicitly requests it for a non-JIT-heavy flavor.

---

## G8 — Time namespace (`CLONE_NEWTIME`)

**What it would buy.** Per-sandbox clock offset so the workload can't
measure wall-clock with the same precision the host sees. Mitigates
timing side-channel attacks (Spectre-style branch-prediction probes,
network timing oracles). Marginal in practice — modern timing attacks
mostly rely on `rdtsc` (CPU-level, not affected by `CLOCK_*` namespaces)
or precise event counters (which time-ns also doesn't gate).

**Concrete implementation shape.** bwrap (as of 0.10) has **no
`--unshare-time` flag** — last upstream discussion in containers/bubblewrap#536
left it at "happy to accept a patch." Two options without that:

1. **Wrap bwrap in our own `unshare(CLONE_NEWTIME)` before exec.** The
   parent process unshares, then execs bwrap; the child inherits the new
   time namespace. ~10 lines in `sandbox::run_in_sandbox`'s pre_exec hook
   on Linux. macOS/WSL2 backends would need the agent (which is the
   bwrap parent in those backends) to do the same — another 10 lines in
   `sandbox-guest-agent`.
2. **Patch bwrap upstream** and depend on a custom build. Higher cost,
   distributes the fix.

**Why it's deferred.** The audit's verdict ("minor") is accurate. We
already enforce strong primary defenses: cgroup CPU caps, prlimit
`--cpu`, wall-clock SIGKILL — collectively bound any timing-based
loops the workload could mount. The remaining timing channels (`rdtsc`,
`clock_gettime(CLOCK_MONOTONIC_RAW)`) bypass time-ns by design. The
attack class that time-ns mitigates (cross-tenant timing oracles) is
not part of our threat model — we're single-tenant per conversation,
and the `--clearenv` + workspace-isolation defenses cover the realistic
exfiltration path.

**What would unblock it.** A documented multi-tenant deployment where
two unrelated conversations share a flavor VM and the operator wants to
defend against one inferring timing about the other. None today.

**Recommended disposition.** Stay deferred. Reconsider if/when the
sandbox is offered as a managed multi-tenant service. The implementation
sketch above is small enough to land in a single commit when needed.

---

## Disposition

| ID | Status | Re-open condition |
|---|---|---|
| G7 MDWX | Deferred | A real shellcode-injection signal in audit logs, OR a non-JIT flavor explicitly requests it |
| G8 time-ns | Deferred | Multi-tenant managed-service deployment |

Both are "out-of-scope, documented" — exactly the disposition the original
Plan 4 set. This doc replaces the implicit decision with an explicit one,
with the implementation sketch preserved for the next contributor.
