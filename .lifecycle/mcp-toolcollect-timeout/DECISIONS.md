# DECISIONS — mcp-toolcollect-timeout

All human/product inputs resolved up front. No unresolved markers remain.

### DEC-1: What is the timeout budget source for connect/handshake + tool-collection?
**Resolution:** Reuse the existing per-server `server.timeout_seconds` (DB column, default 30), floored to `max(1)` — for the stdio handshake budget (ITEM-3/ITEM-4/ITEM-5) and for the outer auto/always-mode collection awaits (ITEM-1/ITEM-2). Add NO new settings row, migration, or permission. Where the value isn't directly at a call site, it is threaded from the `McpServer` row already in scope there.
**Basis:** convention — `mcp/client/http.rs:1097` already does `server.timeout_seconds.max(1) as u64` for the HTTP client's overall timeout, and the tool-call helpers already consume it. Adding a settings row would flip the tier to HEAVY (new migration + permission) for a tunable that already exists and is admin-editable per server.

### DEC-2: Is the timeout budget a fixed constant or admin-configurable (Configurable-settings rule)?
**Resolution:** Admin-configurable, via the EXISTING `server.timeout_seconds` per-server column (editable in the MCP server drawer). No new tunable is introduced by this fix, so no new settings row is warranted; the operational knob already exists and is reused.
**Basis:** convention — satisfies the "prefer admin-configurable" rule by reusing the established per-server tunable rather than hardcoding; matches http.rs / helpers.rs usage.

### DEC-3: On timeout, skip-the-server (warn+continue) or fail-the-send?
**Resolution:** Skip the server (warn + continue) at the outer collection layer (ITEM-1/ITEM-2), indistinguishable from the existing connect/list Err arms; the LLM is still called with the reachable servers' tools. The inner stdio layer (ITEM-3/ITEM-4) returns Err so the breaker records the failure.
**Basis:** convention + the bug — the existing per-server error paths already warn+continue; the defect is solely the missing timeout. §6 (surface + skip, never swallow into a hang) and the diagnosed empty-assistant-message symptom.

### DEC-4: Does the inner stdio timeout use a distinct error variant or the existing serve()-failure shape?
**Resolution:** Reuse the existing `errors::upstream_error(name, UpstreamFailure::Unreachable, msg)` shape (message noting the timeout), identical to the current serve()-failure arms.
**Basis:** codebase — returning the same Err value is what lets it flow unchanged through `McpSession::new` → `create_session_tracked` → `record_connection_failure` (INV-3); a new variant would require touching the breaker path. No product ambiguity.

### DEC-5: How does INV-3 hold on the tool-collection path when the OUTER timeout wins the race with the inner one?
**Resolution:** The blind audit (F1) found the outer `tokio::time::timeout` (same budget as the inner stdio handshake timeout, but started earlier) always elapses first on the collection path, cancelling `get_or_create_with_context` before `create_session_tracked → record_connection_failure` can run — so the breaker never opened. Fix: the outer connect-timeout arm itself calls `session_manager.record_connection_failure(server_id, &err)`. `record_connection_failure` is made `pub(crate)` and its body split into a free fn `record_failure_into` (unit-testable without a Config-bearing manager). Budgets stay at `timeout_seconds` (no margin needed) — recording at the outer arm makes INV-3 hold regardless of which timer wins, with no double-count (if the inner wins, `get_or_create` returns `Ok(Err)`, not `Err(_elapsed)`, so the outer arm's record does not fire).
**Basis:** codebase + audit F1 — reuses the existing breaker API; simplest robust fix.

### DEC-6: How does INV-3 hold for ALWAYS-mode, and how is a cancelled always-mode tool call handled?
**Resolution:** Always-mode builds sessions via `McpSession::new`/`new_with_sampling` directly, bypassing `create_session_tracked`, so it never touches the breaker. Fix (F4): the always-mode connect-timeout arm AND the build-error arm both call `record_connection_failure(server.id, &err)`. Additionally (F7), on a per-tool `call_tool` timeout the loop `break`s instead of reusing the session — the in-flight JSON-RPC request was cancelled mid-flight and a late response could be mismatched against a later request on the same transport, so the session is abandoned rather than reused.
**Basis:** codebase + audit F4/F7.

### DEC-7: Disposition of the lower-severity audit findings (F5 accumulation, F6 write-guard, F8 sandbox-guard-drop).
**Resolution:** WONTFIX, with rationale. F5 (N stalling servers cost up to N×timeout on the FIRST turn, serial collection): inherent to serial collection; the breaker fix (DEC-5/6) makes SUBSEQUENT turns short-circuit, so the unbounded-repeat cost the audit worried about is resolved; parallelizing collection is out of scope for a LIGHT hardening fix. F6 (auto path holds the per-session write guard across the bounded `list_tools`): pre-existing (was held across the previously-unbounded call) and now strictly improved by the bound — not a regression. F8 (sandboxed handshake-timeout returns before assigning `_sandbox_inflight`/`_vm_session`, so guards drop at the early return): verified to release the inflight/VM guard and drop the transport (no leak); byte-identical teardown to the existing `serve()`-Err `?` path.
**Basis:** convention + audit; each is either inherent, pre-existing-and-improved, or verified-safe.
