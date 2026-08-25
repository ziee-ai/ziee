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
