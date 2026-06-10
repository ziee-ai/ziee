# Audit — Round 5 (Final Sweep)

Branch: `feat/project-improvements`
Worktree: `/home/pbya/projects/ziee-chat-feat-project-improvements`

Round 5 confirmed **3** findings (all `real=true`), re-verified against the
current source in this report. The audit has **NOT** converged: one **high**
and two **medium** findings remain.

## Summary

| ID | Severity | Category | File | Title |
|---|---|---|---|---|
| r5-final-sweep-b-01 | high | bug | `tests/memory_mcp/mod.rs` | "Read-only" user in write-gate test actually holds `memory::write` via the default Users group |
| r5-final-sweep-a-01 | medium | incomplete-fix | `modules/file/chat_extension/file.rs` | Track A recency-drop in `process_content_for_llm` is dead code — old text attachments are re-inlined every turn |
| r5-final-sweep-b-02 | medium | consistency | `modules/memory/handlers.rs` | Admin token thresholds bypass handler validation; CHECK violations return 500 not 400 |

Counts by severity: **high 1, medium 2, low 0, nits 0.**

---

## r5-final-sweep-b-01 — Write-gate test does not exercise a denial (HIGH)

**File:** `src-app/server/tests/memory_mcp/mod.rs:164-237`
**Category:** bug · **Severity:** high

`test_write_tools_denied_for_read_only_user` builds its "read-only" user with
`create_user_with_permissions(&server, "mcp_readonly", &["memory::read"])`
(mod.rs:172-177). That helper (`harness_inner.rs:581-680`) mirrors real
registration and **re-adds the system default group** (lines 656-674: it
selects `groups WHERE is_default = true` and inserts a `user_groups` row).
Migration `00000000000061` (line 34) appends `memory::write` to that default
`Users` group's permission array.

The `RequirePermissions` extractor loads **all** of a user's groups via
`get_user_groups` (`extractors.rs:122` → `repository.rs:341-356`, an
unfiltered INNER JOIN with no `is_default` filter), so `auth.groups` carries
both the per-test `["memory::read"]` group **and** the default `Users` group.
The per-tool gate at `handlers.rs:181` calls `has_permission(.., "memory::write")`
→ `check_permission_union` (`checker.rs:9-27`), which is `true` if **any**
active group carries the permission. The default group carries
`memory::write`, so `remember`/`forget` are **not** denied and the assertions
at mod.rs:193-196 and 214-217 (`msg.contains("permission denied") &&
msg.contains("memory::write")`) fail at runtime.

Corroboration: sibling test `test_initialize_returns_server_info`
(mod.rs:34-38) grants only `["memory::write"]` yet passes the handler's
`RequirePermissions<(MemoryRead,)>` gate — only possible because the
auto-assigned default group also supplies `memory::read`. The codebase even
ships `create_user_with_only_permissions` (`harness_inner.rs:719-748`) whose
doc comment describes this exact trap, confirming the author picked the wrong
helper. The production write gate itself is correct; only the test fails to
demonstrate denial.

**Fix:** Swap the helper to `create_user_with_only_permissions`, which strips
default-group membership (verified at `harness_inner.rs:736-744`), in
mod.rs:172-177:

```rust
let user = crate::common::test_helpers::create_user_with_only_permissions(
    &server,
    "mcp_readonly",
    &["memory::read"],
)
.await;
```

With only the `["memory::read"]` group: `remember`/`forget` require
`memory::write` → absent → denied (assertions at 193-196, 214-217 hold);
`recall` requires `memory::read` → present → passes the read gate (assertion at
233-236 holds, then falls through to `MEMORY_DISABLED`). No production change.

---

## r5-final-sweep-a-01 — Track A recency-drop is dead code (MEDIUM)

**File:** `src-app/server/src/modules/file/chat_extension/file.rs:239-244`
**Category:** incomplete-fix · **Severity:** medium

The drop branch
(`if tool_capable && !is_image { return Ok(None); }`, file.rs:242-244) gates on
`tool_capable = model_supports_tools(&context.metadata).await` (file.rs:239-241).
During history replay the `context` is the `transform_context` built in
`streaming.rs:277-285` with `metadata: HashMap::new()` (line 283), forwarded
verbatim into `convert_history_to_messages_with_extensions` (streaming.rs:288-291)
and on to `process_content_for_llm`. `model_supports_tools`
(`available_files.rs:155-201`) is metadata-only with **no DB-only fallback**:
it needs `model_tools_capable` (line 164), or `model_id` (line 172), or
`provider_type` + `model_name` (lines 190-191). With an empty map all three are
absent and it returns `false` unconditionally (line 200).

Critically, `transform_context` is built and consumed **before** the
metadata-bearing `stream_context` (streaming.rs:303-326) and **before**
`call_before_llm_call` runs — so nothing ever seeds its `metadata`. Hence
`tool_capable` is always `false` on this path and the drop is never taken.

Net effect: on every turn of a tool-capable multi-turn conversation,
`before_llm_call` injects the manifest (correct — it uses the real
`stream_context` metadata via `ensure_model_tools_capable`) **and** every old
text attachment is *also* re-inlined in full through history replay — the
exact double-spend the manifest was meant to eliminate. Output correctness is
preserved (the model just sees redundant content); images and
non-tool-capable models are intentionally unaffected. It is a
token-efficiency / context-bloat regression, hence medium not high. No unit or
integration test exercises this branch (no multi-turn old-attachment-drop
assertion in `tests/agentic_chat/mod.rs`; no `#[cfg(test)]` in file.rs).

**Fix (option a — matches the existing pattern):** Seed
`transform_context.metadata` in `streaming.rs:277-285` with the same
`provider_type` / `model_name` / `model_id` / `provider_id` keys already in
scope (they are inserted into `context_metadata` a few lines later at
streaming.rs:303-316), so `model_supports_tools` can resolve during replay:

```rust
let mut transform_metadata = std::collections::HashMap::new();
transform_metadata.insert("provider_type".to_string(),
    serde_json::json!(provider_for_task.provider_type()));
transform_metadata.insert("model_name".to_string(), serde_json::json!(model_name));
transform_metadata.insert("model_id".to_string(),
    serde_json::json!(model_id.to_string()));
transform_metadata.insert("provider_id".to_string(),
    serde_json::json!(provider_id.to_string()));
let transform_context = StreamContext {
    conversation_id,
    branch_id,
    message_id: None,
    user_id,
    pool: pool.clone(),
    metadata: transform_metadata,
    iteration,
};
```

Hand-format to the narrow style — do **not** rustfmt. To avoid duplicating the
four inserts, build `context_metadata` first and clone the relevant subset, or
factor a small helper. This also makes the `provider_id` / `provider_type`
reads inside `process_content_for_llm` (file.rs:246-257) resolvable instead of
erroring with "Provider ID not in context" should a non-image attachment ever
reach the non-dropped fallback. Then delete the now-misleading "short-circuits
to false" comment at file.rs:103-106. Add a regression test (the
`agentic_chat` stub-chat / `STUB_PLAN` harness is the right place): turn 1 with
a file attachment, turn 2, then assert via the stub's request capture that the
turn-2 request does **not** re-inline the turn-1 bytes for a tool-capable model,
while a non-tool-capable model keeps the inline and images are kept on both.

---

## r5-final-sweep-b-02 — Admin token thresholds bypass handler validation (MEDIUM)

**File:** `src-app/server/src/modules/memory/handlers.rs`
**Category:** consistency · **Severity:** medium

`update_admin_settings` validates `default_top_k` (handlers.rs:428-434) and
`cosine_threshold` (handlers.rs:435-443) in-handler with clean 400s, but
`summarize_after_tokens` and `summarizer_keep_recent_tokens` are passed
straight to the repository. Their DB CHECK constraints (and the
trigger-only-downward COALESCE asymmetry) therefore surface as a raw 500
rather than a 400 `VALIDATION_ERROR`, inconsistent with the two siblings above.

**Fix:** Mirror the existing checks. After handlers.rs:443, return
`bad_request("VALIDATION_ERROR", ..)` when `summarize_after_tokens` is outside
`500..=1_000_000` or `summarizer_keep_recent_tokens` is under `100`. After the
prior fetch at handlers.rs:484, compute effective values (body field or prior
fallback) and return 400 when effective `keep_recent >= effective after`,
catching the asymmetric COALESCE / trigger-only-downward case before the DB
CHECK fires. Keep `AppError(..).into()` and the narrow hand-formatting. Update
`summarization_test.rs:85-92` to assert a clean 400 and add cases for the
out-of-range single fields and the asymmetric path.

---

## Verdict

**CONVERGED: NO** — 1 high + 2 medium remain. After applying the three fixes
above and re-running the affected module tests (`memory_mcp::`, `memory::`,
`agentic_chat::`), a Round 6 confirmation sweep is warranted.
