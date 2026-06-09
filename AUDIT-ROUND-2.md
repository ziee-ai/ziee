# Audit — Round 2 (verifying the Round-1 fixes)

Round-2 re-verification of the Round-1 fixes applied to the Track A/B/C +
frontend changes on `feat/project-improvements`. Every finding below was
reproduced against the **current** code in the worktree (and, where noted,
**empirically** by running the test). Findings that turned out not to be real
have already been dropped — only `real == true` items remain.

**This round did NOT converge.** One **high** (a shipped unit test that fails on
the very code it ships with) plus three **medium** items (one false-failure test
landmine and two security-relevant missing-test gaps) require action. The
remainder are low / nit doc-and-consistency cleanups.

## Summary

| # | id | severity | category | file | title |
|---|----|----------|----------|------|-------|
| 1 | r2-verify-track-c-fe-01 | **high** | incomplete-fix | `src-app/server/src/modules/chat/core/services/streaming.rs` | Per-result kept-window cap unreachable when result count ≤ keep_last; shipped test `caps_oversized_kept_results` FAILS |
| 2 | r2-verify-tests-01 | medium | bug | `src-app/server/tests/agentic_chat/mod.rs` | Exclusion test assertion #1 (`read_file >= 1`) fails-when-correct: built-ins reach the stub prefixed (`{id}__read_file`) |
| 3 | r2-fresh-sweep-01 | medium | missing-test | `src-app/server/tests/mcp/mod.rs` | No regression test that admin-configurable built-ins stay editable after the over-broad-guard fix |
| 4 | r2-fresh-sweep-02 | medium | missing-test | `src-app/server/tests/memory_mcp/mod.rs` | Per-tool write-permission gating (`remember`/`forget` require `memory::write`) is untested |
| 5 | r2-verify-mcp-loop-01 | low | incomplete-fix | `src-app/server/src/modules/mcp/chat_extension/mcp.rs` | `enabled`-guard added to only ONE of the two built-in auto-attach sites |
| 6 | r2-verify-track-a-01 | low | incomplete-fix | `src-app/server/src/modules/files_mcp/handlers.rs` | `grep_files` reports `truncated=true` for a corpus with exactly `GREP_MAX_MATCHES` (200) matches |
| 7 | r2-verify-track-a-02 | low | doc | `src-app/server/src/modules/file/chat_extension/file.rs` | Memo comment falsely claims `process_content_for_llm` reads the cached tool-capability boolean |
| 8 | r2-verify-track-c-fe-02 | low | incomplete-fix | `src-app/ui/.../shared/LlmModelLlamaCppSettingsSection.tsx` | New `contextLength` feature is dead code: the component's only call site is commented out |
| 9 | r2-fresh-sweep-03 | low | doc | `src-app/server/tests/memory/summarization_test.rs` | Stale module doc comment references the dropped summarizer columns |
| 10 | r2-fresh-sweep-04 | low | consistency | `src-app/server/src/common/tokens.rs` | Doc claims the chat trim transform shares `estimate_tokens`, but `streaming.rs` inlines a different (floor vs ceil) heuristic |
| 11 | r2-verify-tests-02 | nit | consistency | `src-app/ui/.../FilePreviewList.tsx` | Advisory `Alert` uses antd-v6 deprecated `message` prop while `SummarizerSection` uses the new `title` |
| 12 | r2-fresh-sweep-05 | nit | doc | `src-app/server/src/modules/file/available_files.rs` | `model_tools_capable` memo doc says "once per turn" but metadata (and thus the memo) is rebuilt every loop iteration |
| 13 | r2-fresh-sweep-06 | nit | consistency | `src-app/server/src/modules/files_mcp/handlers.rs` | `grep_files` truncation message always says "showing first N matches" even when the byte-budget cap stopped the scan |
| 14 | r2-fresh-sweep-08 | nit | consistency | `src-app/server/src/modules/memory_mcp/handlers.rs` | `app_error_to_jsonrpc` duplicated verbatim in `files_mcp`/`memory_mcp`; codebase split between INVALID_ARGS/INVALID_PARAMS/VALIDATION_ERROR |

Severity tally: **1 high · 3 medium · 6 low · 4 nit** (14 confirmed).

---

## 1. `streaming.rs` — kept-window cap unreachable when result count ≤ keep_last; shipped test FAILS (HIGH)

**id:** `r2-verify-track-c-fe-01` · **category:** incomplete-fix

**File:** `src-app/server/src/modules/chat/core/services/streaming.rs:1493` (early return), kept-cap loop `1513-1525`, failing test `1615`.

**What's wrong.** The Round-1 fix added a per-result cap on the *kept* window
(`MAX_KEPT_TOOL_RESULT_CHARS = 8000`), but placed it AFTER the function's early
return:

```rust
    if positions.len() <= keep_last {
        return;                       // <-- returns BEFORE the kept-cap loop
    }
    let clear_until = positions.len() - keep_last;
    for &(mi, bi) in &positions[..clear_until] { /* clear OLD results */ }

    // Bound the KEPT window too: keep-last is a fixed count, so even the
    // surviving results can blow the budget if a few are oversized. ...
    for &(mi, bi) in &positions[clear_until..] {
        ... if chars > MAX_KEPT_TOOL_RESULT_CHARS { *content = truncate... }
    }
```

Production constants are `KEEP_LAST_TOOL_RESULTS = 6` and
`CLEAR_TOOL_RESULTS_TOKEN_THRESHOLD = 30_000`. Whenever a conversation has
**≤ 6** tool calls whose outputs blow the 30K-token budget, `positions.len() <= 6`
hits the early return and the kept-window cap never runs — oversized recent
results pass through fully intact, exactly the case the fix's own comment claims
to handle.

**Empirically confirmed.** Running the shipped test on the shipped code:

```
$ cargo test --lib --no-default-features chat::core::services::streaming::trim_tests
test ...::caps_oversized_kept_results ... FAILED
panicked at server/src/modules/chat/core/services/streaming.rs:1630:13:
kept result 0 not bounded: 50000 chars
test result: FAILED. 4 passed; 1 failed; ...
```

`caps_oversized_kept_results` builds exactly 6 oversized results and calls
`clear_old_tool_results(&mut msgs, 100, 6)`; `6 <= 6` hits the early return and
no result is bounded. The fix is logically incomplete AND the test it ships with
does not pass.

**Fix.** Hoist the kept-cap out from behind the early return; `saturating_sub`
makes the old-clear loop a no-op when `count <= keep_last` while still running
the kept cap. Replace `1493-1525`:

```rust
    // Older results (everything before the keep-last window) get their
    // CONTENT replaced with a placeholder. saturating_sub gives 0 when there
    // are <= keep_last results, so nothing old is cleared in that case.
    let clear_until = positions.len().saturating_sub(keep_last);
    for &(mi, bi) in &positions[..clear_until] {
        if let ai_providers::ContentBlock::ToolResult { content, .. } =
            &mut messages[mi].content[bi]
        {
            *content = vec![ai_providers::ContentBlock::Text {
                text: "[tool result cleared to save context]".to_string(),
            }];
        }
    }

    // Bound the KEPT window too — must run even when result_count <= keep_last
    // (nothing was cleared above but the kept results can still be huge).
    for &(mi, bi) in &positions[clear_until..] {
        if let ai_providers::ContentBlock::ToolResult { content, .. } =
            &mut messages[mi].content[bi]
        {
            let chars: usize = content.iter().map(block_text_chars).sum();
            if chars > MAX_KEPT_TOOL_RESULT_CHARS {
                *content = vec![ai_providers::ContentBlock::Text {
                    text: truncate_kept_result(content),
                }];
            }
        }
    }
```

This removes the `if positions.len() <= keep_last { return; }` block entirely.
Verified it does NOT break `noop_when_fewer_than_keep_last` (its 2 kept results
are 4000 chars, under the 8000 ceiling) and makes `caps_oversized_kept_results`
pass; the other trim tests (`clears_old_keeps_recent_past_threshold`,
`small_kept_results_not_truncated`, `noop_under_threshold`) are unaffected
(`count 10 > keep_last 2`, `clear_until` unchanged). Re-run
`cargo test --lib --no-default-features chat::core::services::streaming::trim_tests`
to confirm all 5 pass. Hand-format ~80 col; no rustfmt.

---

## 2. `agentic_chat/mod.rs` — exclusion test assertion #1 fails-when-correct (MEDIUM)

**id:** `r2-verify-tests-01` · **category:** bug

**File:** `src-app/server/tests/agentic_chat/mod.rs:599-603` (assertion #1).

**What's wrong.** Built-in tool names reach the model **prefixed**
`{server_id}__read_file`, but the test asserts an exact-match on the bare name,
so the assertion fails even when the exclusion fix under test is correct. The
full chain (all reproduced from current code):

1. The built-in `files` server is pushed into the SAME `server_configs` list as
   third-party servers (`mcp.rs:1305-1309`) and iterated through the SAME
   tool-collection loop calling `convert_mcp_tool_to_ai_tool(server.id, &mcp_tool)`
   (`mcp.rs:1483`) — no separate bare-name path for built-ins.
2. That helper produces `format!("{}__{}", server_id, mcp_tool.name)`
   (`helpers.rs:90-91`), as the code's own doc at `mcp.rs:79` states.
3. The prefixed name flows verbatim to the wire (`tools.rs:19-28`/`33-43`,
   OpenAI `convert_tools` at `openai.rs:472` — no prefix stripping).
4. The stub records `tools[].function.name` verbatim (`stub_chat.rs:175-188`).
5. `requests_with_tool`/`has_tool` use exact equality `t == name`
   (`stub_chat.rs:66,127-132`); the `read_first_file` script gate also uses exact
   `t == "read_file"` (`stub_chat.rs:239`).

So the stub only ever sees `{files_server_id}__read_file`: the script gate is
false (stub never emits a `read_file` call) AND `requests_with_tool("read_file")`
counts 0 — assertion #1 (`>= 1`) fails even when the exclusion fix is correct.
The file header (`mod.rs:18`) confirms these tests are compile-verified but never
run, so the failure is latent. Assertions #2 (substring
`.contains("thirdparty_ping")`) and #3 (`thirdparty.hits() == 0`) are
prefix-aware and correctly test the exclusion behavior — only #1 is the landmine.

**Fix (minimal, this test only).** Replace assertion #1 at `mod.rs:599-603` with
a suffix-aware attachment check (don't assert the round-trip — the stub won't
emit the call because of the exact-match gate):

```rust
// 1. The built-in files server STILL auto-attaches. Tool names reach the
//    model prefixed `{server_id}__read_file`, so match by suffix.
assert!(
    stub.requests().iter().any(|r| {
        r.tool_names.iter().any(|t| t == "read_file" || t.ends_with("__read_file"))
    }),
    "built-in files server must auto-attach despite enable_mcp=false; requests={:?}",
    stub.requests()
);
```

**Preferred broader fix** (also unblocks the pre-existing
read_file/grep/remember round-trip tests that share this gap): make
`stub_chat.rs` prefix-aware — add `fn matches_tool(t, name) -> bool { t == name || t.ends_with(&format!("__{name}")) }`, use it in `has_tool` and every `script()`
gate, and have the stub EMIT the full prefixed name it observed (the chat loop
recovers the route via `full_name.splitn(2, "__")` at `mcp.rs:2856-2865`; a
bare-named call would route as `(server_id="read_file", tool_name="")` and fail).
Verify with `cargo test --test integration_tests agentic_chat::third_party_mcp_server_excluded_when_enable_mcp_false -- --test-threads=1`.

---

## 3. `mcp/mod.rs` — no regression test that admin-configurable built-ins stay editable (MEDIUM)

**id:** `r2-fresh-sweep-01` · **category:** missing-test

**File:** `src-app/server/tests/mcp/mod.rs` (guard at `repository.rs:1596-1604`).

**What's wrong.** Commit `52a5953b` narrowed the `update_system_mcp_server`
"built-in is immutable" guard so it now gates ONLY the deterministic
`files_mcp_server_id()`/`memory_mcp_server_id()` — the admin-configurable
built-ins (filesystem/fetch/browser/git/code_sandbox) must remain editable. That
narrowing has zero coverage on either side:

- `test_update_system_server` (`mod.rs:545`) edits a non-built-in
  ("original_system_server"); it never PUTs a built-in id.
- `tests/code_sandbox/tier2_built_in_protection.rs` asserts only the row flag /
  raw-SQL DELETE protection / upsert-preserves-fields via the repo; it never
  calls the `update_system_mcp_server` HTTP route (its own comment at lines 49-50
  wrongly claims that's "covered by the mcp module's own tests").
- A full grep of `tests/` shows NO test PUTs a built-in id to the update
  endpoint, for either the 200 (admin-configurable) or 400 (zero-config) case.

A future re-broadening of the guard would pass CI silently.

**Fix.** Add an integration test in `mcp/mod.rs` pinning BOTH sides:
- **(A) Editability:** as a `mcp_servers_admin::{read,edit}` admin, GET
  `/mcp/system-servers`, find the seeded `filesystem` (or `fetch`) row by name
  (migration 7 + 25, random UUIDs → must NOT be in the guard set), PUT a config
  change to `/mcp/system-servers/{id}`, assert **200** + persisted via a
  follow-up GET.
- **(B) Immutability:** PUT a trivial change to
  `/mcp/system-servers/{files_mcp_server_id()}` and assert **400** with
  `error_code == "BUILT_IN_SERVER"`; repeat for `memory_mcp_server_id()`. Those
  rows are upserted by an async `tokio::spawn` at module init, so poll the per-id
  GET in a short retry loop until present before the PUT (mirror the suite's
  existing async-row-wait pattern). Reference ids via
  `ziee::files_mcp::files_mcp_server_id()` / `ziee::memory_mcp::memory_mcp_server_id()`.

---

## 4. `memory_mcp/mod.rs` — per-tool write-permission gating is untested (MEDIUM)

**id:** `r2-fresh-sweep-02` · **category:** missing-test

**File:** `src-app/server/tests/memory_mcp/mod.rs` (gate at `handlers.rs:173-195`).

**What's wrong.** The memory_mcp JSON-RPC extractor only enforces `memory::read`
(`handlers.rs:39`, `RequirePermissions<(MemoryRead,)>`). The write gate for
`remember`/`forget` is enforced MANUALLY inside `dispatch_tool_call`:

```rust
let required_perm = match call.name.as_str() {
    "recall" => "memory::read",
    "remember" | "forget" => "memory::write",
    other => return Err(... method_not_found ...),
};
if !has_permission(user, groups, required_perm) {
    return Err((StatusCode::OK,
        JsonRpcError::invalid_params(format!(
            "permission denied: '{}' requires '{}'", call.name, required_perm))));
}
```

This manual match is the actual authorization boundary between a read-only user
and mutating memories — and it is untested. Every `create_user_with_permissions`
call in the test module grants `["memory::write"]` or
`["memory::read","memory::write"]`; NONE creates a user with only
`["memory::read"]` to assert `remember`/`forget` is denied. A regression
collapsing the per-tool gate (e.g. dropping the `required_perm` match so all
three tools authorize on `memory::read`) would not be caught.

**Fix.** Add a test creating a user with ONLY `memory::read`. Assert `remember`
and `forget` each return a JSON-RPC error (`-32602`, HTTP 200) whose message
contains `"permission denied"` and `"memory::write"`. Note `recall` returns a
`MEMORY_DISABLED` error by default (admin memory off) — don't assert it
*succeeds*, only assert its error does NOT contain `"permission denied"`:

```rust
#[tokio::test]
async fn test_write_tools_denied_for_read_only_user() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server, "mcp_readonly", &["memory::read"]).await;

    // remember -> permission-denied (-32602 at HTTP 200)
    let res = jsonrpc_call(&server, &user.token, "tools/call",
        json!({ "name": "remember", "arguments": { "content": "x" } }))
        .send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["result"].is_null());
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(msg.contains("permission denied") && msg.contains("memory::write"),
        "remember must be denied for read-only user; got: {body}");

    // forget -> same denial
    let res = jsonrpc_call(&server, &user.token, "tools/call",
        json!({ "name": "forget",
                "arguments": { "memory_id": uuid::Uuid::new_v4() } }))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(msg.contains("permission denied") && msg.contains("memory::write"),
        "forget must be denied for read-only user; got: {body}");

    // recall passes the read gate (then MEMORY_DISABLED by default); assert
    // it is NOT the permission-denied error.
    let res = jsonrpc_call(&server, &user.token, "tools/call",
        json!({ "name": "recall", "arguments": { "query": "anything" } }))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(!msg.contains("permission denied"),
        "recall must pass the read gate for a read-only user; got: {body}");
}
```

---

## 5. `mcp.rs` — `enabled`-guard added to only ONE of the two built-in auto-attach sites (LOW)

**id:** `r2-verify-mcp-loop-01` · **category:** incomplete-fix

**File:** `src-app/server/src/modules/mcp/chat_extension/mcp.rs:1863-1868`
(`after_llm_call`); fixed site at `1299-1303` (`before_llm_call`).

**What's wrong.** The Round-1 fix added an `if s.enabled` guard to the
`before_llm_call` built-in auto-attach but left the `after_llm_call` site
unguarded (confirmed in current code):

```rust
// before_llm_call (GUARDED, 1299):
if let Some(s) = crate::core::Repos.mcp.get_any_server(*id).await? {
    if s.enabled { builtin_servers.push(s); }
}

// after_llm_call (UNGUARDED, 1865):
for id in auto_attach_builtin_ids(&context.metadata) {
    if !accessible_servers.iter().any(|s| s.id == id) {
        if let Some(bs) = crate::core::Repos.mcp.get_any_server(id).await? {
            accessible_servers.push(bs);     // <-- no `if bs.enabled`
        }
    }
}
```

Both sites source the SAME ids via `auto_attach_builtin_ids(&context.metadata)`.
`get_any_server` (`repository.rs:1335`) SELECTs by id with no `enabled` filter, so
a disabled row is returned and pushed; the execution loop (`line 2049`) resolves
it and executes it approval-bypassed (`is_builtin → needs_approval=false`,
`1880-1881`). Severity is correctly **low**: built-ins are inserted
`enabled=true`, the upsert's `ON CONFLICT DO UPDATE` deliberately leaves
`enabled` untouched, and built-ins are immutable via the API — so no shipping
path disables a built-in today. But the fix is asymmetric to its own stated
defense-in-depth intent (disabled mid-loop, or a built-in `tool_use` reaching
`after_llm_call` without `before_llm_call` re-attaching, bypasses it).

**Fix.** Mirror the `before_llm_call` guard at `1865`:

```rust
if let Some(bs) = crate::core::Repos.mcp.get_any_server(id).await? {
    if bs.enabled {
        accessible_servers.push(bs);
    }
}
```

With both sites guarded, a disabled built-in is never pushed into
`accessible_servers`, so the execution loop's `find(|s| s.id == server_id)`
returns None and the tool hits the existing "Server not found" path.

---

## 6. `files_mcp/handlers.rs` — `grep_files` reports `truncated=true` at exactly 200 matches (LOW)

**id:** `r2-verify-track-a-01` · **category:** incomplete-fix

**File:** `src-app/server/src/modules/files_mcp/handlers.rs:508-511`
(`GREP_MAX_MATCHES = 200` at line 31).

**What's wrong.** The break site flags truncation at the push-site `>=` with no
lookahead:

```rust
if matches.len() >= GREP_MAX_MATCHES {
    truncated = true;
    break 'outer;
}
```

A corpus with **exactly** 200 matching lines and nothing after pushes match
#200, making `matches.len() == 200 >= 200` true, so `truncated = true` is set
even though scanning was in fact exhaustive — there is no 201st match to justify
the flag. This is the over-reporting-on-exactly-200 false positive the audit's
A-correctness-04 said to avoid. Test gap also confirmed: the only `truncated`
assertion is `test_grep_files_hits` (`tests/files_mcp/mod.rs:457-460`, asserts
false for 2 matches); no boundary test at exactly 200 exists, so the regression
is uncaught. Severity **low** — a semantically incorrect `truncated=true` (and a
misleading summary at `538-543`) on an exhaustive result, not data loss.

**Fix.** Confirm a 201st match before flagging. Push unconditionally, then
`if matches.len() > GREP_MAX_MATCHES { truncated = true; break 'outer; }`. After
the `'outer` loop, trim the sentinel:
`if matches.len() > GREP_MAX_MATCHES { matches.truncate(GREP_MAX_MATCHES); }`.
Truncated is then set only when a `(GREP_MAX_MATCHES+1)`th match actually exists.
The byte-cap path at `514` is the same theoretical class but far less likely to
land on the boundary — leave as-is or address separately. Add a Tier-2 boundary
test: a file with exactly 200 matching lines → assert
`structuredContent.truncated == false` and `matches.len() == 200`; optionally a
201-match file asserting `truncated == true`, `matches.len() == 200`.

---

## 7. `file.rs` — memo comment falsely claims `process_content_for_llm` reads the cached boolean (LOW)

**id:** `r2-verify-track-a-02` · **category:** doc

**File:** `src-app/server/src/modules/file/chat_extension/file.rs:100-103`.

**What's wrong.** The A-correctness-06 comment states the tool-capability memo is
read by "the rest, plus the per-history `process_content_for_llm` calls." The
`process_content_for_llm` clause is false. `ensure_model_tools_capable`
(`available_files.rs:218-233`) seeds the memo into `stream_context.metadata`
(constructed at `streaming.rs:318-326`, fed to `call_before_llm_call` at
`streaming.rs:340`). But `process_content_for_llm` is invoked from
`convert_history_to_messages_with_extensions` (`streaming.rs:761`/`789`) whose
context is `&transform_context` — a SEPARATE `StreamContext` built at
`streaming.rs:277-285` with `metadata: HashMap::new()`, and used at `288-292`,
BEFORE `stream_context` exists and before `before_llm_call` seeds the memo. So
`file.rs:236-237`'s `model_supports_tools(&context.metadata)` reads an empty map
every time, short-circuits to `false` (no `model_id`/`provider_type`/`model_name`
keys → no DB lookup, `available_files.rs:198`), and the recency-drop there is
currently inert. Pre-existing at base `5567db30`; nothing regressed
functionally — the defect is purely the misleading comment.

**Fix.** Replace `file.rs:100-103` to describe what actually happens:

```rust
        // Compute the tool-capability once per LLM iteration and memoize it
        // into `context.metadata` (idempotent — whichever extension's
        // `before_llm_call` runs first seeds it; the others read the cached
        // boolean). NOTE: the per-history `process_content_for_llm` path runs
        // on a SEPARATE `transform_context` whose metadata is an empty map
        // (services/streaming.rs:277), built and consumed BEFORE this
        // `stream_context` exists, so it cannot see this memo — its
        // `model_supports_tools` call short-circuits to `false` (no model_id in
        // that map) and the recency-drop there is currently inert. Threading
        // the memo into `transform_context` would be a separate change.
```

Making the recency-drop tool-capability-aware (threading the memo/model metadata
into `transform_context`) is the real fix if the inert path is unintended, but
it is out of scope; correcting the comment is the minimal change.

---

## 8. `LlmModelLlamaCppSettingsSection.tsx` — new `contextLength` feature is dead code (LOW)

**id:** `r2-verify-track-c-fe-02` · **category:** incomplete-fix

**File:** `src-app/ui/src/modules/llm-provider/components/llm-models/shared/LlmModelLlamaCppSettingsSection.tsx`; call site
`EditLlmModelDrawer.tsx:110`.

**What's wrong.** Commit `52a5953b` added a `contextLength?: number` prop driving
(a) a "Model max context: N" description suffix, (b) a dynamic InputNumber max,
and (c) an over-context warning `Alert` guarded by `contextLength != null`. The
component is internally well-formed and compiles, but it is **unreachable**.
Confirmed in current code: the only reference outside its own file is
`EditLlmModelDrawer.tsx:110`, which sits INSIDE a commented-out JSX block gated
behind `{/* TODO: Add engine/device settings for local models once backend supports it */}`:

```tsx
{/* TODO: Add engine/device settings for local models once backend supports it */}
{/* {isLocalModel && (
    <>
      <LlmModelEngineSelectionSection />
      ...
      <LlmModelLlamaCppSettingsSection />
    </>
) } */}
```

There is no active `import` of the component, and `git diff main...HEAD` on
`EditLlmModelDrawer.tsx` is empty — the fix did not wire it up. Even uncommented,
the JSX passes no `contextLength`, so the description collapses to its fallback,
the ceiling stays at 131072, and the `Alert` never renders. The new behavior is
unreachable, untestable via E2E, and has zero user-facing effect. Correctly
self-classified **low** (shipped dead code, not a running-behavior bug).

**Fix.** Two paths:
- **(a) Wire it up** *only if* the local-engine-settings backend has landed:
  uncomment the block, re-add the imports, and pass
  `contextLength={currentModel?.capabilities?.context_length}` (`currentModel`
  is in scope at `EditLlmModelDrawer.tsx:22-24`; `capabilities.context_length`
  exists in `api-client/types.ts`); add an E2E spec asserting the "Model max
  context: N" suffix + the over-context Alert.
- **(b) Defer** (lower risk, recommended given the call site is byte-identical to
  main and explicitly TODO-gated): revert the `contextLength` prop additions back
  to the no-prop form so the codebase doesn't carry dead code implying a working
  feature.

---

## 9. `summarization_test.rs` — stale module doc references the dropped summarizer columns (LOW)

**id:** `r2-fresh-sweep-03` · **category:** doc

**File:** `src-app/server/tests/memory/summarization_test.rs:11`.

**What's wrong.** Line 11 reads
`//   * summarize_after_n_messages / summarizer_keep_recent round-trip + CHECK`,
but migration `00000000000085_memory_summary_token_aware.sql` DROPs those two
columns and replaces them with `summarize_after_tokens` /
`summarizer_keep_recent_tokens`. The test bodies were updated to the new token
columns (`test_summarizer_threshold_round_trip` uses `summarize_after_tokens` /
`summarizer_keep_recent_tokens` at `56-65`; the CHECK-constraint test asserts on
`summarizer_keep_recent_tokens`). The header contradicts the code immediately
below it. Harmless at runtime.

**Fix.** Replace line 11 with
`//   * summarize_after_tokens / summarizer_keep_recent_tokens round-trip + CHECK`.

---

## 10. `tokens.rs` — doc claims the chat trim transform shares `estimate_tokens`; it inlines a different (floor) heuristic (LOW)

**id:** `r2-fresh-sweep-04` · **category:** consistency

**File:** `src-app/server/src/common/tokens.rs:4-7`; mismatch at
`src-app/server/src/modules/chat/core/services/streaming.rs:1480`.

**What's wrong.** `tokens.rs:4-5` documents `estimate_tokens` as "Shared by the
chat context-trimming transform (clear old tool results past a threshold) and the
token-aware conversation summarizer." The summarizer genuinely calls it
(`memory/engine/summarizer.rs:136`/`144`). But `clear_old_tool_results` does NOT:
it inlines `total_chars / 4` at `streaming.rs:1480` (grep confirms `streaming.rs`
has zero references to `estimate_tokens`/`common::tokens`). The two also disagree
on rounding: `estimate_tokens` uses `chars.div_ceil(4)` (ceil) while
`streaming.rs` uses integer `/ 4` (floor). The "shared" contract is false at one
of its two documented call sites. Negligible numeric impact for a threshold
check → **low** (doc-accuracy/consistency, not behavioral). The finding's cited
`streaming.rs:1480` is the in-`modules/chat/core/services/` file, not a top-level
`streaming.rs`.

**Fix.** Pick one:
- **Preferred (single source of truth):** expose a
  `tokens_from_chars(chars: usize) -> usize { chars.div_ceil(4) }` in
  `common/tokens.rs`, call it in BOTH `streaming.rs` (replacing the inlined
  `total_chars / 4`) and internally in `estimate_tokens`, making the rounding
  genuinely shared.
- **Minimal:** amend the `tokens.rs:4-7` doc to stop claiming the chat transform
  shares the helper, e.g. "Used by the token-aware conversation summarizer; the
  chat context-trimming transform uses the same chars/4 heuristic inline (floor,
  not this fn's ceil)."

---

## 11. `FilePreviewList.tsx` — advisory `Alert` uses antd-v6 deprecated `message` prop (NIT)

**id:** `r2-verify-tests-02` · **category:** consistency

**File:** `src-app/ui/src/modules/file/chat-extension/components/FilePreviewList.tsx:87`.

**What's wrong.** The Round-1 advisory Alert uses
`message={<span><strong>{f.filename}</strong>: {meta.suggestion}</span>}`. Pinned
antd is v6.4.3, where `Alert.d.ts` marks `message?` as
`@deprecated please use title instead` while `title?: React.ReactNode` is the
current content prop. The sibling Alert at
`SummarizerSection.tsx:55` already uses `title=...`, so the two call sites
diverge. `message` still renders in v6 (same content slot), so the
file-upload-advisory E2E assertions (`.ant-alert-warning` + `<strong>filename</strong>` + suggestion) are unaffected — pure deprecation/consistency nit.

**Fix.** Rename `message={...}` → `title={...}` keeping the same
`<span><strong>{f.filename}</strong>: {meta.suggestion}</span>` child. The
rendered DOM is byte-identical; converges both Alert call sites on `title` and
clears a new antd-v6 deprecation warning under `npm run check`'s antd-lint.

---

## 12. `available_files.rs` — memo doc says "once per turn" but metadata is rebuilt every loop iteration (NIT)

**id:** `r2-fresh-sweep-05` · **category:** doc

**File:** `src-app/server/src/modules/file/available_files.rs:158-161` and
`212-217`; also `file.rs:100-103`.

**What's wrong.** The doc describes `model_tools_capable` as "seeded once per
turn" / "turn-stable". The `StreamContext` metadata is actually rebuilt fresh
every tool-loop iteration: the loop opens at `core/services/streaming.rs:217`
(`loop {`), and line 303 constructs a new `context_metadata = HashMap::new()`
inside it, assigned to `stream_context.metadata` at line 324. So the memo key
starts absent each iteration and `ensure_model_tools_capable` re-seeds it per
ITERATION. The recomputed value is identical every iteration because
`provider_for_task`/`model_id`/`model_name`/`provider_id` are captured BEFORE the
loop (`streaming.rs:180-182`) and never change mid-turn — no correctness/staleness
bug, only an inaccurate comment and a missed micro-optimization (DB+catalog
lookup re-runs each iteration). Pure **nit**.

**Fix.** Reword "once per turn" → "once per LLM iteration" in
`available_files.rs:158-161`, `212-217`, and `file.rs:100-103`, noting the memo
is re-seeded per iteration because the metadata map is rebuilt at the top of each
tool-loop iteration (the captured model fields don't change, so the recomputed
value is identical). Optional (not required): hoist the lookup into the
spawned-task scope above `loop {` at `streaming.rs:217` and seed it into
`context_metadata` at construction (line 303) for a true per-turn memo.

---

## 13. `files_mcp/handlers.rs` — truncation message always says "showing first N matches" even on the byte-budget cap (NIT)

**id:** `r2-fresh-sweep-06` · **category:** consistency

**File:** `src-app/server/src/modules/files_mcp/handlers.rs:538-543` (note);
`truncated` set at `509` (match cap) and `515` (byte cap).

**What's wrong.** `truncated` is set in two distinct places: line 509 when the
match cap fires (`matches.len() >= 200`) and line 515 when the per-call byte
budget fires (`scanned >= GREP_MAX_SCAN_BYTES == 16 MiB`). The summary note
unconditionally hardcodes `[showing first {GREP_MAX_MATCHES} matches; results truncated ...]` — always claiming 200 matches were shown. When the byte budget
triggered the stop, `matches.len()` can be far below 200 (even 0), so the
human-readable note overstates what was returned. The `structuredContent.truncated`
boolean (line 554) is honest and cause-neutral; only the summary string conflates
the two stop causes. Purely cosmetic → **nit**.

**Fix.** Track which cap fired and branch the message. Add `let mut hit_byte_cap = false;` near `let mut truncated = false;`, set `hit_byte_cap = true;` at the
byte-budget break (514-517), and branch the note (538-543): the byte-cap path
emits e.g. `[scan stopped at the size budget; results may be incomplete — narrow the pattern or pass id]`, the match-cap path keeps the "showing first N matches"
wording. A simpler cause-neutral alternative (no flag) is acceptable: replace the
note string with `[results truncated — narrow the pattern or pass id]`.

---

## 14. `memory_mcp/handlers.rs` — `app_error_to_jsonrpc` duplicated; error-code naming drift (NIT)

**id:** `r2-fresh-sweep-08` · **category:** consistency

**File:** `src-app/server/src/modules/memory_mcp/handlers.rs:148` and
`src-app/server/src/modules/files_mcp/handlers.rs:132`.

**What's wrong.** Two near-identical `app_error_to_jsonrpc` helpers exist (the
memory_mcp doc literally says "Mirrors files_mcp."), both mapping
400+UNKNOWN_TOOL → method_not_found, 400/404 → invalid_params, else internal;
memory_mcp only splits the `400 | 404` arm into two arms. Error-code naming
drift confirmed by grep: across the two MCP modules INVALID_ARGS appears 6×,
VALIDATION_ERROR 4×, INVALID_PARAMS 1× (`files_mcp:170`), INVALID_PATTERN 1×; the
deserialize-failure path uses INVALID_ARGS in 5 places but INVALID_PARAMS once
for the same tools/call-param-decode failure. Server-wide the dominant convention
is VALIDATION_ERROR (19×) vs INVALID_ARGS (6×, all in these new modules). All
`bad_request` codes map to HTTP 400 → invalid_params, so **zero behavioral
impact**. The asymmetric-path claim holds: memory_mcp short-circuits unknown
tools to method_not_found directly in `dispatch_tool_call` (`180-185`,
`218-222`), never constructing an UNKNOWN_TOOL AppError, making its mapper's
`400 if UNKNOWN_TOOL` arm dead code; files_mcp routes UNKNOWN_TOOL through the
mapper. Pure internal-naming/DRY **nit**.

**Fix (optional, no functional change).**
1. Hoist one shared `app_error_to_jsonrpc` into a common location (e.g.
   `crate::modules::mcp` or `crate::common`) both built-in MCP servers import; use
   the single-arm `400 | 404 => invalid_params` form (memory_mcp's two-arm split
   is gratuitous).
2. Standardize the internal `bad_request` code: pick one for JSON-RPC
   param-decode failures (currently INVALID_PARAMS at `files_mcp:170` vs
   INVALID_ARGS elsewhere) and prefer the codebase-dominant VALIDATION_ERROR for
   semantic validation so memory_mcp's VALIDATION_ERROR and files_mcp's
   INVALID_ARGS/INVALID_PATTERN converge.
3. Optionally make the unknown-tool path symmetric (either memory_mcp emits an
   UNKNOWN_TOOL AppError through the shared mapper, removing the dead arm, or
   files_mcp short-circuits like memory_mcp). All paths already yield the correct
   `-32602`/`-32601`.

---

## Convergence assessment

**Not converged.** Action required this round:

- **1 HIGH** (`r2-verify-track-c-fe-01`) — the kept-window per-result cap is
  behind an early return, so it never runs for conversations with ≤ 6 tool
  results; the shipped unit test `caps_oversized_kept_results` FAILS on the very
  code it ships with (reproduced empirically). This is a real outbound-context
  bug, not just a test issue — fix the function, then the test passes.
- **3 MEDIUM** — one false-failure test landmine (`r2-verify-tests-01`, exclusion
  assertion #1 fails-when-correct because built-in tool names are prefixed) and
  two security-relevant missing-test gaps: the admin-configurable-built-in
  editability guard (`r2-fresh-sweep-01`) and the memory_mcp per-tool write gate
  (`r2-fresh-sweep-02`) — both authorization boundaries with zero coverage.

The remaining 6 low + 4 nit items are doc/consistency/dead-code cleanups with no
running-behavior impact and can be batched.
