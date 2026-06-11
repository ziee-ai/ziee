# Audit Round 3 — Confirmed Findings

Branch: `feat/project-improvements` (Track A files-MCP / Track B inline memory / Track C sandbox + frontend)
Scope: re-verification of Round-3 candidate findings against current source.
Status: **4 findings confirmed real.** All claims re-reproduced end-to-end against the code at audit time.

## Summary

| ID | Severity | Category | File | Title |
|----|----------|----------|------|-------|
| r3-verify-stub-and-tests-01 | high | missing-test | `tests/agentic_chat/mod.rs:339` | `inline_self_save` test never opts the user into extraction → `remember` tool never attached, test fails-when-correct |
| r3-verify-stub-and-tests-02 | high | bug | `modules/memory/chat_extension/extension.rs:18` | Memory ext (order 90) sets `attach_memory_mcp` AFTER MCP ext (order 30) built its tool list → inline `remember` can never attach |
| r3-final-sweep-01 | low | consistency | `modules/mcp/chat_extension/mcp.rs:1852` | `approval_mode=Disabled` + `enable_mcp=true` + mixed builtin/third-party turn surfaces an approval prompt in a "Disabled" conversation |
| r3-final-sweep-02 | nit | consistency | `modules/code_sandbox/repository.rs:108` | Sandbox `get_conversation_files` `project_refs` has no `user_id` re-validation, unlike the files-MCP resolver |

Severity counts: **high 2, medium 0, low 1, nit 1.**

The two `high` findings are tightly coupled: they are the two reasons the same Track-B test (`inline_self_save_persists_memory_without_continuation`) cannot pass. Both must be fixed for the inline self-save path to work and for the test to go green.

---

## r3-verify-stub-and-tests-01 — `inline_self_save` test never opts the user into extraction (high, missing-test)

**File:** `src-app/server/tests/agentic_chat/mod.rs:339` (test), `:174-187` (`enable_memory` helper)

**Evidence (reproduced):**
- `inline_self_save_persists_memory_without_continuation` (mod.rs:339-394) is a plain `#[tokio::test]` (not `#[ignore]`d) and calls only `enable_memory(&server, &user)` at mod.rs:344.
- `enable_memory` (mod.rs:174-187) PUTs `/memory/admin-settings {enabled:true}` and nothing else — it never sets per-user `extraction_enabled`.
- `extraction_enabled` defaults to **FALSE**: confirmed `migrations/00000000000056_create_memory_system.sql:63` → `extraction_enabled BOOLEAN NOT NULL DEFAULT FALSE`. The row is auto-created OFF via `get_or_init_user_settings`.
- In `memory/chat_extension/memory.rs:96-131`, the `attach_memory_mcp` metadata flag is inserted **only** when `opted_in = get_or_init_user_settings(..).extraction_enabled` is true (gate at memory.rs:115). Without opt-in the flag is never set.
- `auto_attach_builtin_ids` (mcp.rs:116-134) therefore returns empty, and with `enable_mcp` defaulting false (`#[serde(default)] pub enable_mcp: bool`, mcp extension.rs:45-46 — and `send_and_collect` at mod.rs:152-171 omits it), the early return at mcp.rs:1258-1261 (`!enable_mcp && builtin_ids.is_empty()`) fires. The `remember` tool is never delivered to the stub.
- Result: `stub.requests_with_tool("remember")` is 0, failing `== 1` (mod.rs:364-369); no `user_memories` row is written, failing the conversation-scoped assertion (mod.rs:391-394).
- The required opt-in is confirmed in the real-LLM test: `tests/memory/real_llm_test.rs:130-132` PUTs `/memory/settings {extraction_enabled:true}`. Route exists at `memory/routes.rs:29-31` (`update_user_settings`).

**Fix:** After `enable_memory(&server, &user)` (mod.rs:344) and before `send_and_collect`, opt the user into extraction, mirroring the real-LLM test:

```rust
let resp = reqwest::Client::new()
    .put(server.api_url("/memory/settings"))
    .header("Authorization", format!("Bearer {}", user.token))
    .json(&json!({ "extraction_enabled": true }))
    .send().await.expect("opt user into extraction");
assert!(resp.status().is_success(), "enable extraction: {}",
    resp.text().await.unwrap_or_default());
```

Preferred: fold this into `enable_memory` (or add a sibling `enable_inline_save` helper) so admin-enable + per-user opt-in always travel together. **Necessary but not sufficient** — this only un-gates `attach_memory_mcp`; the test still fails until r3-verify-stub-and-tests-02 (extension ordering) is also fixed.

---

## r3-verify-stub-and-tests-02 — Memory extension order 90 sets the attach flag after MCP order 30 already read it (high, bug)

**File:** `src-app/server/src/modules/memory/chat_extension/extension.rs:18`

**Evidence (reproduced):**
- Orders confirmed: file = **20** (file extension.rs:20), MCP = **30** (mcp extension.rs:16), memory = **90** (memory extension.rs:18).
- `extension_registration.rs:18` does `entries.sort_by_key(|e| e.order)` and registers ascending, so within one tool-loop iteration each extension's `before_llm_call` runs sequentially against the same `&mut stream_context`.
- The MCP extension reads the auto-attach flags at order 30 (`auto_attach_builtin_ids(&context.metadata)`, mcp.rs:1257; flag check at mcp.rs:127/130) — **before** the memory extension inserts `attach_memory_mcp` at order 90 (memory.rs:118). MCP builds its tool list before the flag exists.
- Cross-iteration carry-over can't save it: `let mut context_metadata = std::collections::HashMap::new();` at `core/services/streaming.rs:303` sits **inside** the `loop {` opened at streaming.rs:217, so metadata is rebuilt empty each iteration.
- The file flag works only because file (20) precedes MCP (30): file sets `attach_files_mcp` the same way (file.rs:131) and MCP reads it in time. The memory case is the asymmetric one.
- Net effect: `attach_memory_mcp` is never true at the moment MCP would attach the memory server's `remember` tool, so the tool is never offered to the model. (Currently masked by the r3-...-01 opt-in gate short-circuiting first; once that opt-in is applied, this ordering bug is what keeps the test red.)

**Fix (Option A, preferred — one line):** Lower the memory extension order to sit between file (20) and MCP (30), e.g. **25**, in memory/chat_extension/extension.rs:18, with a comment explaining it must precede MCP (order 30) so `attach_memory_mcp` is set before MCP's `before_llm_call` reads it. Retrieval/summary system-block injection (memory.rs:75-90) is unaffected (it just inserts a system message; 25 still lands it after assistant/file). `after_llm_call` extraction is order-independent.

**Option B (preserves retrieval at 90):** Split the attach-flag-setting block (memory.rs:96-131) into a tiny separate extension at order < 30, leaving retrieval + summary at order 90. More code; only worth it to keep the order-90 system-block comment literally true.

Keep the hand-narrow ~80-col Rust style; do **not** rustfmt. After applying both Option A and finding -01's opt-in, verify `inline_self_save_persists_memory_without_continuation` asserts `stub.requests_with_tool("remember") == 1` and the conversation-scoped row persists.

---

## r3-final-sweep-01 — Disabled-approval conversation can still surface an approval prompt (low, consistency)

**File:** `src-app/server/src/modules/mcp/chat_extension/mcp.rs:1852`

**Evidence (reproduced):**
- The Disabled early-return is gated by `&& !has_builtin_call` (mcp.rs:1852-1853), so a turn containing a built-in call skips the unconditional `return Ok(ExtensionAction::Complete)`.
- `before_llm_call` attaches/lists third-party servers based only on `send_request.enable_mcp` (mcp.rs:1258-1278); `approval_mode` is not consulted there, so with `enable_mcp=true` the model is genuinely told about third-party tools even in a Disabled conversation.
- For a non-builtin tool, the `ApprovalMode::Disabled => true` arm (mcp.rs:1913) routes it into `tools_needing_approval`, which creates a pending approval record (`create_tool_approval`, mcp.rs:1958-1972) and fires `send_approval_required_event` (mcp.rs:1980).
- The mixed turn is realizable: `auto_attach_builtin_ids` (mcp.rs:116-134) flags files/memory built-ins independently of `enable_mcp`, and a model can emit a built-in call (read_file/remember) plus a third-party call in one turn.
- Untested: `tests/mcp/approval_test.rs::test_approval_mode_disabled` only asserts settings persistence (no chat message); the agentic_chat Disabled-path tests all use `enable_mcp=false` (forcing `mcp_servers=Some(vec![])`), so the `enable_mcp=true` + Disabled + mixed case is genuinely unexercised.

**Not a security bypass** (correctly scoped low/consistency): the third-party tool does not auto-execute — it pauses for approval (the *more* conservative outcome); the built-in result is persisted first so the provider request stays protocol-valid; the tool resolves only on explicit approve/deny.

**Fix:** Enforce one explicit contract. Recommended: in the `ApprovalMode::Disabled` arm, do **not** route the third-party tool to `tools_needing_approval`; instead synthesize a denial-style `McpContentData::ToolResult { is_error: Some(true), content: "MCP is disabled for this conversation; tool not executed", .. }.to_message_content()` (mirroring the pattern at mcp.rs:2061-2071) pushed into `tool_results`, and skip both `tools_to_execute` and `tools_needing_approval` for it. This preserves the Disabled contract (no prompt ever surfaces), keeps every `tool_use` paired with a `tool_result`, and matches the no-builtin Disabled path. Add an integration test (tests/mcp or tests/agentic_chat): with `approval_mode=Disabled` + `enable_mcp=true`, drive a turn emitting both a built-in and a third-party call, asserting no pending approval record, no `approval_required` SSE event, the built-in executed, and the third-party got a denial `tool_result`. If the team instead prefers to keep blocking via the approval queue, document that at mcp.rs:1910-1913 and still add the test asserting the prompt is intended.

---

## r3-final-sweep-02 — Sandbox `get_conversation_files` `project_refs` lacks `user_id` re-validation (nit, consistency)

**File:** `src-app/server/src/modules/code_sandbox/repository.rs:108`

**Evidence (reproduced):**
- The `project_refs` CTE (repository.rs:108-113) selects `pf.file_id` from `project_files JOIN project_conversations` and unions it into `file_refs`; the final SELECT (repository.rs:119-131) joins `files f ON fr.file_id = f.id` with **no** `f.user_id` predicate. `get_conversation_files` takes only `conversation_id` (repository.rs:67-70).
- The parallel path `file/available_files.rs::resolve_available_files` funnels the project+attachment union through `Repos.file.get_by_ids_and_user(&union_ids, user_id)` (available_files.rs:433), dropping non-owned ids. So the read-time ownership posture diverges between the two resolvers — the factual claim holds.

**Not a live cross-tenant leak** (correctly classified nit): (a) project file attach enforces `file.user_id != caller => 404` (file/project_extension/handlers.rs:78); (b) conversation attach enforces both project + conversation ownership by the same caller; (c) projects are strictly per-user — so a project file is always owned by the conversation owner. Additionally every sandbox entry point calls `assert_owns_conversation` before `get_conversation_files`. The single-argument query is a **deliberate design choice** documented at handlers.rs:962-964: "`get_conversation_files` was deliberately loosened to a single-argument query so the ownership policy lives in one place at the handler boundary."

**Fix (Option A, preferred — matches documented architecture):** Leave the query single-argument; add a one-line comment above the `project_refs` CTE pointing at the handler-boundary policy (mirroring handlers.rs:962-964). Comment-only — no test needed.

**Option B (only if standardizing on read-time defense-in-depth in both resolvers):** Change the signature to `get_conversation_files(&self, conversation_id: Uuid, user_id: Uuid)` and add `AND f.user_id = $2` to the final SELECT (the handler already computes the owner via `assert_owns_conversation` → `get_conversation_user_id`). This contradicts the explicit design note at handlers.rs:962-964, so take it only if the team decides to standardize; if so, add a sandbox integration test asserting a foreign-owned file id is excluded.

---

## Verdict

The audit has **NOT** converged: two `high`-severity, mutually-coupled findings remain on the Track-B inline self-save path (the missing opt-in in the test, and the extension ordering bug). Both must be fixed before `inline_self_save_persists_memory_without_continuation` can pass and the inline `remember` flow works at all. The remaining two findings (low + nit) are consistency items with the conservative/safe behavior already in place; address at the team's discretion.
