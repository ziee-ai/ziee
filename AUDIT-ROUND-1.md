# Audit Round 1 — feat/project-improvements

Consolidated, adversarially-verified findings for the Track A/B/C + frontend
changes on branch `feat/project-improvements` (commits `6b3d2a81`, `2ced072a`,
`09b81114`, `5567db30` over `main`).

Every finding below was re-checked against the actual code on this branch.
Near-duplicates are merged (see the **Merged** notes). One completeness-critic
addition (`CC-add-01`) was found by reading the code directly and is marked as
such.

## Summary

| id | severity | category | file | title |
|---|---|---|---|---|
| C-and-loop-01 | **high** | bug | mcp/chat_extension/mcp.rs | `enable_mcp=false` + built-in auto-attach injects ALL accessible MCP servers, not just built-ins |
| C-and-loop-02 | **high** | bug | mcp/chat_extension/mcp.rs | Mixed built-in + third-party call in Disabled approval mode orphans the built-in `tool_use` (no `tool_result`) |
| A-correctness-01 | **high** | bug | files_mcp/handlers.rs | `read_file` line-slicing path is dead; offset/limit are pages not lines for normal text/code files |
| B-correctness-01 | **high** | bug | memory/chat_extension/memory.rs | Inline self-save bypasses user `extraction_enabled` gate + escapes the daily quota |
| frontend-01 | **high** | consistency | mcp/.../SystemMcpServersPage.tsx | Built-in hide breaks existing passing E2E tests that assert built-in cards visible |
| A-correctness-02 | medium | bug | files_mcp/handlers.rs | Extensionless images/binaries unreadable (`load_original` 404s) |
| A-correctness-03 / cross-cutting-01 | medium | bug | files_mcp/handlers.rs | All tool-call errors collapsed to JSON-RPC `internal` (-32603), mislabeling client errors |
| A-correctness-04 | medium | bug | files_mcp/handlers.rs | `grep_files` silently truncates at 200 matches with no truncation indicator |
| A-correctness-06 | medium | perf | file/available_files.rs | `model_supports_tools` + `resolve_available_files` re-run per turn with N sequential per-file queries, no memoization |
| B-correctness-02 | medium | bug | memory/engine/summarizer.rs | Fraction-of-window `trigger_override` can drop trigger below `keep_recent`, silently disabling summarization |
| C-and-loop-04 | medium | bug | code_sandbox/tools/files.rs | `read_file` AMBIGUOUS_FILENAME error advises a remedy `read_file` cannot honor |
| C-and-loop-07 | medium | missing-test | tests/agentic_chat/mod.rs | No test asserts third-party servers EXCLUDED when `enable_mcp=false` (would catch C-and-loop-01) |
| frontend-02 | medium | bug | mcp/.../SystemMcpServersPage.tsx | Client-side built-in filter desyncs pagination total/label, renders short/empty pages |
| cross-cutting-02 | medium | doc | memory_mcp/tools.rs | `recall` description still claims pure 'semantic similarity' after FTS fallback landed |
| A-correctness-05 | low | correctness | file/available_files.rs | Attachment `file_ids` have no ORDER BY → non-deterministic dedup canonical + manifest order |
| A-correctness-07 | low | consistency | file/available_files.rs | `available_files` spans ALL branches while code_sandbox sees only the active branch |
| A-correctness-08 | low | correctness | files_mcp/handlers.rs | `read_paginated` silently skips failed pages, presenting a non-contiguous range as contiguous |
| A-security-01 | low | perf | files_mcp/handlers.rs | Unbounded model-controlled `limit` can overflow `start + count` (debug panic / release wrap) |
| A-security-02 | low | perf | files_mcp/handlers.rs | `grep_files` scans full file bodies with no per-call byte cap |
| B-correctness-03 | low | bug | memory_mcp/handlers.rs | MCP `remember` does not validate `kind`; out-of-enum value hits DB CHECK → opaque internal error |
| B-correctness-04 | low | consistency | memory/chat_extension/retriever.rs | RRF tie-break ordering nondeterministic across equal fused scores |
| B-correctness-05 | low | bug | memory_mcp/handlers.rs | Content length cap uses byte length but error claims a char limit |
| B-security-01 | low | security | memory_mcp/handlers.rs | `recall` (a read op) gated on `memory::write`, not `memory::read` |
| C-and-loop-03 | low | consistency | code_sandbox/repository.rs | `upsert_builtin_server` 'preserve admin's value on conflict' rationale is now dead |
| C-and-loop-05 | low | security | mcp/repository.rs | `get_any_server` (auto-attach + approval bypass) does not filter on `enabled` |
| C-and-loop-06 | low | perf | chat/core/services/streaming.rs | `clear_old_tool_results` keep-last-K cannot bound the kept-window size |
| frontend-03 | low | consistency | llm-provider/.../LlamaCppLlmModelSettingsSection.tsx | Copy fix applied to a dead duplicate component (zero usages) |
| frontend-06 | low | missing-test | file/.../FilePreviewList.tsx | New upload-suitability advisory UI has no E2E coverage |
| frontend-07 / cross-cutting-05 | low | missing-test | memory/.../SummarizerSection.tsx | Token rename + new keep<trigger validation has no updated E2E |
| frontend-08 | low | consistency | api-client/types.ts | `ModelCapabilities.context_length` surfaced for UI but no UI consumes it |
| cross-cutting-03 | low | consistency | file/available_files.rs | Manifest says 'Address files by id (never by name)' but `read_file` accepts name |
| cross-cutting-04 | low | missing-test | tests/files_mcp/mod.rs | No integration coverage of files_mcp read/list/grep round-trips, AMBIGUOUS_NAME, pagination |
| CC-add-01 | low | correctness | file/available_files.rs | **(completeness-critic add)** `model_supports_tools`/`model_context_window` swallow DB errors → silent capability downgrade |
| A-correctness-10 | nit | doc | file/available_files.rs | Manifest renders single-page documents as 'text', under-describing paginated docs |
| frontend-04 | nit | style | file/.../FilePreviewList.tsx | New arrow callbacks use parenthesized single params; Biome `arrowParentheses` is 'asNeeded' |
| frontend-05 | nit | style | memory/.../SummarizerSection.tsx | Changed InputNumber lines exceed Biome lineWidth 80, would be reflowed |
| cross-cutting-06 | nit | style | memory/repository.rs | Broken column alignment after token rename in UPDATE SET |
| cross-cutting-08 | nit | consistency | files_mcp/routes.rs | files_mcp routes.rs omits the `.route()`-vs-`.api_route()` justification comment its sibling carries |

**Totals:** 36 confirmed findings — 5 high, 8 medium, 19 low, 4 nit (1 of the
lows is a completeness-critic add).

---

# HIGH

## 1. C-and-loop-01 — `enable_mcp=false` + built-in auto-attach injects ALL accessible MCP servers

**File:** `src-app/server/src/modules/mcp/chat_extension/mcp.rs:1257-1297` (also `helpers.rs:54-76`)

**What's wrong.** When `send_request.enable_mcp == false` but a built-in flag
(`attach_files_mcp` / `attach_memory_mcp`) makes `builtin_ids` non-empty, the
early-return at line 1258 is bypassed (`!enable_mcp && builtin_ids.is_empty()`).
The else branch then sets `mcp_servers = None` (line 1271), and
`validate_and_build_config(.., None)` (line 1282) takes the
"no specific servers requested → use ALL accessible servers with ALL tools"
path (`helpers.rs:73`), filling `server_configs` with every third-party +
group-system server the user can access. The built-ins are then appended
(1293-1297). This directly contradicts the inline comment at 1263-1264
("when ONLY built-in servers are auto-attaching, we attach just those"). The
downstream loop (1333+) injects all those servers' tools into the LLM call and,
for any server with `UsageMode::Always`, **pre-executes** its tools against the
user's message (verified the Always-mode pre-exec branch at `mcp.rs:~1340`) — a
real side effect despite the user turning MCP off.

```rust
let mcp_servers = if send_request.enable_mcp {
    send_request.mcp_config.as_ref().map(|c| c.mcp_servers.clone())
} else {
    None                       // ← routes to validate_and_build_config's "all accessible" branch
};
```

**Fix.** Make the disabled path request an explicit EMPTY list so only the
appended built-ins survive. Replace the `else { None }` with
`else { Some(Vec::new()) }`. Verified: `helpers.rs:54-72` iterates
`for req in requested` over the empty vec, producing empty `valid_configs`, so
only built-ins remain. (Equivalent: after `validate_and_build_config`, do
`if !send_request.enable_mcp { server_configs.retain(|(id,_)| is_builtin_server_id(*id)); }`
before the append loop.) Add the Tier-2 test from **C-and-loop-07**.

---

## 2. C-and-loop-02 — Mixed built-in + third-party call in Disabled approval mode orphans the built-in `tool_use`

**File:** `src-app/server/src/modules/mcp/chat_extension/mcp.rs:1840-1968`

**What's wrong.** The diff relaxed the early return to
`Disabled && !has_builtin_call` (line 1840-1841). A turn with a built-in **and**
a third-party tool now proceeds past it. In the approval loop, the built-in gets
`needs_approval = false` (line 1868-1869) → pushed to `tools_to_execute`
(line 1911); the third-party tool in Disabled mode hits the new
`Disabled => true` arm (line 1896) → pushed to `tools_needing_approval`
(line 1909). Because `tools_needing_approval` is non-empty, the block at
line 1916 runs, creates approval records **only** for `tools_needing_approval`,
and `return Ok(ExtensionAction::Complete)` at **line 1968 — before the execution
loop at line ~1976** that would run `tools_to_execute`.

The built-in's `tool_use` block was already finalized to the DB (read from
`message_with_content.contents`), so it sits on disk with **no `tool_result`**.
Resume paths only fetch `tool_use_approvals WHERE status='approved'`
(`approval/repository.rs:217`); the built-in was never inserted there, so nothing
ever backfills its result. The codebase documents this exact failure everywhere
else (max_iteration synthetic results 1583-1628; `stop_when_tools_called`
2575-2589; comments at 782/1048-1049/2576-2597 warn the provider rejects the next
request with *"tool_use ids found without tool_result blocks"*). The base code
never hit it because the old Disabled arm was `unreachable!()`.

**Fix.** Before the `return Ok(Complete)` at the approval pause (line 1968),
execute the built-in tools in `tools_to_execute` (they bypass approval by
design) and persist their `tool_result`s via
`Repos.chat.core.append_content(message_id, &tr.content_type(), tr.clone())`
(the pattern at 2580-2588) BEFORE creating the third-party approval records and
returning Complete. Do **not** use synthetic placeholders for built-ins — that
would silently drop the `read_file`/`remember` the relaxation exists to enable;
reserve placeholders only for `tool_use`s that will genuinely never run. Add a
Tier-2 test: Disabled conversation, model emits one built-in + one third-party
in the same turn; assert it pauses for the third-party AND the built-in already
has a persisted `tool_result`, and that the next message does not error with
"tool_use ids found without tool_result blocks".

---

## 3. A-correctness-01 — `read_file` line-slicing is dead; offset/limit are pages not lines for text/code

**File:** `src-app/server/src/modules/files_mcp/handlers.rs:279-285` (dispatch),
`289-367` (readers), `tools.rs:16/22/23` (description)

**What's wrong.** Dispatch routes `FileType::Text | FileType::Document` on
`if file.pages > 0 { read_paginated } else { read_text_lines }` (lines 280-284).
But every normally-processed text/code file has **`pages == 1`**:
`TextProcessor::extract_text` returns `vec![text]` (one element); `upload.rs`
sets `text_page_count = .len()` (==1) and `available_files.rs:102` sets
`pages = text_page_count`. So `pages > 0` is always true → text files route to
`read_paginated`, never `read_text_lines`. Hand-evaluating
`read_paginated(total=1, offset=Some(200), limit=Some(100))`: `start=200.min(1)=1`,
`end=(1+100).min(1)=1`, the `for page in 1..1` loop is empty → returns
`[<name> — no text on pages 2..1]`. Only the default `offset=0` read returns
content. Meanwhile `tools.rs:16/22/23` tells the model offset/limit mean **lines**
for text/code. `read_text_lines` is effectively dead (only reachable on an
extraction-failure edge case where `pages == 0`). No test covers the
offset/limit round-trip.

**Fix.** Dispatch on the unit semantics implied by `file_type`, not on
`pages > 0`:

```rust
FileType::Text => read_text_lines(&*storage, user_id, file, args.offset, args.limit).await,
FileType::Document => {
    if file.pages > 0 {
        read_paginated(&*storage, user_id, file, args.offset, args.limit).await
    } else {
        read_text_lines(&*storage, user_id, file, args.offset, args.limit).await
    }
}
```

Leave `grep_files` unchanged (its page-1 read already returns whole content for
text files). Add a Tier-2 round-trip test: upload a >2000-line text file, assert
`read_file(id, offset=200, limit=100)` returns lines 201..300 with
`line_start`/`line_end` metadata; plus a PDF case asserting page semantics.

---

## 4. B-correctness-01 — Inline self-save bypasses user `extraction_enabled` gate + escapes daily quota

**File:** `src-app/server/src/modules/memory/chat_extension/memory.rs:96-115`

**What's wrong.** In `before_llm_call`, the inline self-save path (insert
`attach_memory_mcp` + prepend `MEMORY_SAVE_NUDGE`) is gated **only** on
`tool_capable && admin.enabled` (lines 98-100). It never loads `user_settings`.
The path it replaces — the background extractor — explicitly gates on
`user_settings.extraction_enabled` and returns early when off
(`engine/extractor.rs:77-80`); the privacy-first default is OFF (migration 56:
`extraction_enabled ... DEFAULT FALSE`). `after_llm_call` (line 153) skips the
background extractor whenever `tool_capable`, so for the common tool-capable
model class a user who turned extraction OFF still gets the `remember` tool
attached + the assistant nudged to persist facts — defeating the opt-out.
Separately, the MCP `remember` handler inserts with `source='mcp_tool'`, but the
daily-quota query counts only `source='extraction'` (`extractor.rs:97-109`), so
inline saves also escape the per-user/day cap.

*(Note: honoring per-conversation `memory_mode='off'` for inline save would be a
NEW enhancement, not a regression — the background extractor never consulted
`memory_mode`. The genuine defect is the `extraction_enabled` bypass + quota
escape.)*

**Fix.** Load user settings and require extraction enabled before
attaching/nudging, mirroring the extractor's gate:

```rust
if admin.enabled {
    if let Ok(us) = Repos.memory.get_or_init_user_settings(context.user_id).await {
        if us.extraction_enabled {
            context.metadata.insert("attach_memory_mcp".into(), serde_json::json!("true"));
            request.messages.insert(0, ChatMessage { role: Role::System,
                content: vec![ContentBlock::Text { text: MEMORY_SAVE_NUDGE.to_string() }] });
        }
    }
}
```

Then decide whether inline `mcp_tool` saves should count toward the quota: if
yes, broaden the count query to `source IN ('extraction','mcp_tool')` or move the
quota check into the `remember` handler.

---

## 5. frontend-01 — Built-in hide breaks existing passing E2E tests

**File:** `src-app/ui/src/modules/mcp/components/system/SystemMcpServersPage.tsx:50`

**What's wrong.** The only change vs main is
`const filteredServers = systemServers` →
`systemServers.filter(server => !server.is_built_in)` (line 50). Migration 7
seeds `filesystem` ('Filesystem Access') and `fetch` ('Web Fetch'); migration 25
sets `is_built_in = true` for them. The page never filtered built-ins
server-side, so these cards previously rendered. The filter removes them — and
the impact is **broader than a single test**: in the unmodified
`07-mcp/mcp-admin-servers.spec.ts` describe block, `lines 46-47` assert both
'Filesystem Access' and 'Web Fetch' visible; `120/126` toggle 'Filesystem
Access'; `103/239` edit 'Web Fetch'; `489` asserts the built-in card visible
then its Edit btn; search/filter (134-165) and sort (196-220) tests depend on
built-in cards. The test-file diff (+65) only **appends** a new describe block;
it does not update any broken assertion. `McpServerCard.tsx` also conflicts:
Edit is rendered for built-ins (273-283) while Delete is gated
`canDelete && !server.is_built_in` (285) — built-ins were intentionally
edit-yes/delete-no, contradicting the new 'zero configurable surface' rationale.

**Fix (recommended — Option A, convention-consistent):** revert line 50 to
`const filteredServers = systemServers` and rely on the existing card-level
`canDelete && !server.is_built_in` gate; no test changes needed.
**If hiding is truly intended (Option B):** keep the filter but (a) move the
exclusion into SQL — see **frontend-02** — and (b) update every affected
unmodified test to assert built-ins are absent (`toHaveCount(0)`), removing the
Web Fetch edit / Filesystem toggle / search / sort cases that reference them, and
drop the now-dead Delete gate in `McpServerCard`. Given Edit is still offered for
built-ins, Option A is lower-risk.

---

# MEDIUM

## 6. A-correctness-02 — Extensionless images/binaries unreadable (`load_original` 404s)

**File:** `src-app/server/src/modules/files_mcp/handlers.rs:469-474` (`extension_of`); save side `upload.rs:100-104`

**What's wrong.** SAVE derives the on-disk ext as
`filename.rsplit('.').next().unwrap_or("bin").to_lowercase()`. For an
extensionless name `photo`, `rsplit` returns the whole name `"photo"`, which
survives the sanitizer (`filesystem.rs:143-152`) → stored as `{id}.photo`. READ
uses `Path::extension()` which is `None` for a dot-less name → `"bin"` → looks
for `{id}.bin` and 404s. Images always read via `load_original` with no page
fallback (handlers.rs:248-253), so an extensionless image is unreadable through
`read_file` → JSON-RPC internal error. *Empirically reproduced: `photo`/`noext`
MISMATCH; but odd/long exts (`data.backup-2024`, long ext) MATCH because
`load_original` re-runs the SAME sanitizer on the read side — so the blast radius
is extensionless files only.*

**Fix.** Mirror the upload-time rule in `extension_of`:

```rust
fn extension_of(filename: &str) -> &str {
    filename.rsplit('.').next().filter(|s| !s.is_empty()).unwrap_or("bin")
}
```

(Or the more robust fix: have `load_original` glob `{id}.*`, or persist the
sanitized extension on the `File` row at upload time.) Add a test uploading an
extensionless image (filename `photo`, PNG bytes) asserting `read_file` returns
the image block, not `not_found`. Do **not** special-case odd/long extensions —
they already round-trip via the shared sanitizer.

---

## 7. A-correctness-03 / cross-cutting-01 — All tool-call errors collapsed to JSON-RPC `internal` (-32603) **(merged)**

**File:** `src-app/server/src/modules/files_mcp/handlers.rs:89-92`

**What's wrong.** `handlers.rs:89-92` maps EVERY `dispatch_tool_call` error to
`JsonRpcError::internal(e.to_string())` (-32603). But `dispatch_tool_call` emits
client-class errors: `INVALID_PARAMS` (151), `UNKNOWN_TOOL` (159),
`INVALID_ARGS` (243/393/395), `AMBIGUOUS_NAME` (220), `MISSING_TARGET` (231),
`INVALID_PATTERN` (406), plus `not_found`. All surface as -32603 instead of
-32602 (invalid_params) / -32601 (method_not_found). `AppError::status_code()`
exists; `JsonRpcError::invalid_params`/`method_not_found` constructors exist —
so categories are available. **Important correction to both findings:**
`memory_mcp` is only PARTIALLY the "correct" sibling — it tags `method_not_found`
/`invalid_params` at the **dispatch** level (134/162), but its **per-tool**
`bad_request` errors ALSO collapse to `internal` at `memory_mcp/handlers.rs:172`.
There is **no shared AppError→JsonRpc mapping helper anywhere**, and
`AppError.error_code` is private with only a `status_code()` accessor. Existing
tests assert only `error.is_object()`, so nothing locks in the wrong behavior.

**Fix.** Add `pub fn error_code(&self) -> &str` on `AppError` (common/type.rs),
then map in BOTH handlers:

```rust
fn app_error_to_jsonrpc(e: &AppError) -> JsonRpcError {
    match e.status_code() {
        400 if e.error_code() == "UNKNOWN_TOOL" => JsonRpcError::method_not_found(&e.to_string()),
        400 => JsonRpcError::invalid_params(e.to_string()),
        404 => JsonRpcError::invalid_params(e.to_string()),
        _   => JsonRpcError::internal(e.to_string()),
    }
}
```

Apply the same to `memory_mcp/handlers.rs:172` for parity. Add a Tier-3 test
asserting `error.code == -32601` for an unknown tool and `-32602` for a bad-args
`tools/call`.

---

## 8. A-correctness-04 — `grep_files` silently truncates at 200 matches with no indicator

**File:** `src-app/server/src/modules/files_mcp/handlers.rs:415-467`

**What's wrong.** The loop breaks via `break 'outer` once
`matches.len() >= GREP_MAX_MATCHES` (=200, line 31/441). The summary (449-465)
is derived solely from the matches list with no cap marker, and
`structuredContent` is `json!({ "matches": matches })` (466) with no `truncated`
field. So a capped result is indistinguishable from "exactly 200 matches exist",
and the model reasonably stops iterating. This is inconsistent with the sibling
readers, which DO signal truncation (`read_paginated` "…pages shown; … for more";
`read_text_lines` "…of N; … for more").

**Fix.** Track the cap-hit and surface it. Set a `bool truncated` at the
`break 'outer` site, append a marker to the summary when set, and add
`"truncated": truncated` to `structuredContent`:

```rust
if truncated { summary.push_str(&format!(
    "\n[showing first {} matches; results truncated — narrow the pattern or pass id]", GREP_MAX_MATCHES)); }
Ok(text_result(summary, Some(json!({ "matches": matches, "truncated": truncated }))))
```

Prefer the explicit flag at the break site over `matches.len() >= 200` to avoid
over-reporting on a corpus with exactly 200 matches.

---

## 9. A-correctness-06 — `model_supports_tools` + `resolve_available_files` re-run per turn with N sequential queries, no memoization

**File:** `src-app/server/src/modules/file/available_files.rs:148-176`, `291-362`

**What's wrong.** (1) `model_supports_tools` does an uncached
`Repos.llm_model.get_by_id(model_id)` (line 158) every call. It is invoked from
`file::before_llm_call`, `project::before_llm_call`, once per replayed
`FileAttachment` block in `file::process_content_for_llm` (called once per content
block per history message by `streaming.rs:761/789`), and twice in
`memory::before_llm_call`. So K old attachments → K + ~2-4 identical model
lookups per turn, none memoized. (2) `resolve_available_files` (291-362) loads +
ownership-checks each file in a loop with a separate
`Repos.file.get_by_id_and_user` (352); the repo has no batch method. It runs on
every tool-capable `before_llm_call` AND on every files-MCP tool call
(handlers.rs:153). A 100-file project → 100+ sequential round-trips per manifest
build, repeated per tool call.

**Fix.** (1) Memoize the boolean in `context.metadata` (`model_id`/`provider_type`
/`model_name` are turn-stable): the earliest `before_llm_call` (which has
`&mut context`) seeds `metadata["model_tools_capable"]`; `model_supports_tools`
returns it early if present. (2) Add `get_by_ids_and_user(&[Uuid], user_id)`
(`WHERE id = ANY($1) AND user_id = $2`), fetch the union of project + attachment
ids in one round-trip into a `HashMap<Uuid,File>`, then iterate the ordered id
lists building `AvailableFile`s from the map (preserving project-first ordering).
Add a Tier-2 test asserting a many-file project resolves with a single batched
query and ownership filtering still drops foreign/deleted ids.

> See also **CC-add-01** (completeness add): the `if let Ok(Some(..))` pattern in
> both functions silently swallows transient DB errors.

---

## 10. B-correctness-02 — Fraction-of-window `trigger_override` can drop trigger below `keep_recent`, disabling summarization

**File:** `src-app/server/src/modules/memory/engine/summarizer.rs:385-393`

**What's wrong.** Migration 85 enforces `keep_recent < summarize_after` only on
the admin row (defaults 3000 / 12000). `refresh_summary` applies
`trigger = trigger.min(override)` but passes `keep_recent` through to
`decide_summarize_action` unchanged (line 393) — no re-clamp.
`chat_extension/memory.rs:174-177` computes `override = context_window * 0.75`;
for a 2048-token model that is 1536 < the default `keep_recent` 3000. Tracing
`decide_summarize_action`: when total (e.g. 2000) exceeds the overridden trigger
(1536), it does NOT Noop at line 138; the keep-recent loop keeps newest, then
keeps older while `acc + t <= 3000`; since total (2000) < 3000 the loop never
breaks, cutoff walks to 0, and line 152 returns Noop. So precisely the
small-context models the override protects never summarize until they exceed
`keep_recent` (3000) — by which point a 2-4K window has already overflowed.

**Fix.** Clamp `keep_recent` below the effective trigger right after computing
the override:

```rust
let trigger = match trigger_override { Some(o) => trigger.min(o), None => trigger };
// Preserve keep_recent < trigger after the override (DB CHECK only guards the admin row).
let keep_recent = keep_recent.min(trigger.saturating_sub(1));
```

Add a unit test asserting `decide_summarize_action` does NOT Noop when
`total > trigger` and `keep_recent` was clamped.

---

## 11. C-and-loop-04 — `read_file` AMBIGUOUS_FILENAME advises a remedy `read_file` cannot honor

**File:** `src-app/server/src/modules/code_sandbox/tools/files.rs:161-218` (and `sandbox.rs:610-643`)

**What's wrong.** Duplicate-suffixed names (`foo (2).csv`) exist ONLY as bwrap
bind **destinations** (`sandbox.rs:612-643`); stored filenames are never changed
(`sandbox.rs:610-611`). On the host, attachments are staged under
`attachments/<file_id>`, not by suffixed filename. `load_file_content` reads the
HOST workspace by filename: `foo (2).csv` → NotFound, then the fallback filter
compares `f.filename == "foo (2).csv"` against the unchanged stored name
(`foo.csv`) → zero matches → generic `Err`. The AMBIGUOUS_FILENAME branch fires
only for the bare `foo.csv`, and its advice ("read by the exact mounted path")
sends the model to retry `read_file("foo (2).csv")`, which provably cannot
resolve. The error also frames `file_id` as "the unambiguous handle", but
`read_file`'s MCP schema exposes only `filename`/`start_line`/`end_line` — no
`file_id`. Net: the second of two same-named attachments is unreadable through
`read_file`; the only working route is `execute_command` + `cat` of the suffixed
in-sandbox path.

**Fix (minimal, A).** Rewrite the AMBIGUOUS_FILENAME message to point at the
route that works — instruct the model to `execute_command` and `cat` the suffixed
mount path (the suffix algorithm is deterministic; ideally reuse the dedup logic
from `build_bwrap_argv` via a shared helper so the message and mount can't
drift). **(Fuller, B):** add an optional `file_id` param to `read_file`'s schema
and branch `load_file_content` on it (`load_original` by id, bypassing the
filename filter). Either way fix the misleading comment at `files.rs:169-170`.

---

## 12. C-and-loop-07 — No test asserts third-party servers EXCLUDED when `enable_mcp=false`

**File:** `src-app/server/tests/agentic_chat/mod.rs:238-284`

**What's wrong.** `files_mcp_auto_attaches_even_when_enable_mcp_false` registers
only a stub model + a project file and asserts ONLY that the built-in `read_file`
IS attached. The test user owns NO third-party MCP server (none is ever
registered in `agentic_chat/mod.rs`), so the test passes regardless of whether
the **C-and-loop-01** leak exists. No test in `tests/` covers
`enable_mcp=false` + built-in-flag + third-party-present exclusion.

**Fix.** Add `third_party_mcp_server_excluded_when_enable_mcp_false`: register a
second in-process stub MCP server exposing a uniquely-named tool
(`thirdparty_ping`) + a user-scoped server row pointing at it; create a project
with a knowledge file (flags the files built-in); PUT mcp-settings
`{ enable_mcp: false }`; send a read-first plan; assert `requests_with_tool
("read_file") >= 1` AND that across `stub.requests()` the union of tool names
contains ONLY built-in files/memory tools and NO `thirdparty_ping`
(prefixed `<server_id>__thirdparty_ping`); assert the third-party stub recorded
zero `list_tools`/`tools/call` hits (AtomicUsize counter). This fails until
**C-and-loop-01** is fixed.

---

## 13. frontend-02 — Client-side built-in filter desyncs pagination total/label, renders short/empty pages

**File:** `src-app/ui/src/modules/mcp/components/system/SystemMcpServersPage.tsx:50`; `mcp/repository.rs:1487,1544`

**What's wrong.** All three built-ins are `is_system=true, is_built_in=true`
rows. `list_system_mcp_servers` rows query filters only `WHERE is_system = true`
under `LIMIT/OFFSET`, and the COUNT query likewise — so `total` over-counts by up
to 3. The only built-in exclusion is the client-side
`systemServers.filter(... !is_built_in)` at line 50, running **after**
pagination. Result: pagination `total` and the "X-Y of Z" label over-count; a
page whose rows are all built-in renders fewer cards than `pageSize` (or none)
while pagination still shows more; the empty state can show on a page that
actually returned built-in rows.

**Fix.** Move the exclusion into SQL. Add `AND is_built_in = false` to BOTH the
rows query (after `WHERE is_system = true`, ~1487) and the COUNT query (~1544).
Then in the component remove the line-50 filter and render/empty-check
`systemServers` directly. Run `cargo clean` so build.rs re-verifies the SQLx
queries. *(This pairs with frontend-01 Option B; note repository.rs:873 already
uses the `is_built_in = false` pattern.)*

---

## 14. cross-cutting-02 — `recall` description still claims pure 'semantic similarity' after FTS fallback landed

**File:** `src-app/server/src/modules/memory_mcp/tools.rs:41`

**What's wrong.** The description reads "Search the user's memories by semantic
similarity to a query." But commit `2ced072a` made embedding OPTIONAL in
`recall()` (`handlers.rs:343-360`): with an embedding model it runs hybrid
(vector + FTS via RRF); on embed failure OR no configured `embedding_model_id` it
falls back to `retriever::fts_search` — pure lexical `websearch_to_tsquery
('simple', ...)` ranked by `ts_rank_cd` (`retriever.rs:284-306`, doc:
"Works with NO embedding model"). So the model is told it gets semantic search
while it can actually get keyword search. Inconsistent with the sibling
`grep_files` tool, whose description is explicit ("This is keyword search, not
semantic").

**Fix.** Update `tools.rs:41` to: *"Search the user's memories by relevance to a
query — hybrid semantic + full-text when an embedding model is configured,
otherwise keyword/full-text only. Returns up to top_k matches; pick terms and
iterate."*

---

# LOW

## 15. A-correctness-05 — Attachment `file_ids` have no ORDER BY → non-deterministic dedup canonical + manifest order

**File:** `src-app/server/src/modules/file/available_files.rs:310-334,364-369`

**What's wrong.** The attachment query (310-334) is
`SELECT DISTINCT fid::uuid ... WHERE fid ~ $2` with NO ORDER BY → Postgres
returns hash-aggregated rows in arbitrary order. The project query orders
(`ORDER BY added_at ASC`); the sibling sandbox `get_conversation_files` orders
`ORDER BY f.created_at, f.id` with an explicit "deterministic + stable" comment.
`dedup_by_checksum` picks first-in-input as canonical. Consequences: with two
byte-identical attachments, the canonical-vs-aka pick can flip between turns; the
manifest row order among attachments is non-deterministic. Project-vs-attachment
dedup IS stable (project always first). Doc comments are also inaccurate
(module line 19-20 "canonical = earliest upload"; dedup line 366-368 "each by
upload order").

**Fix.** Order the attachment query, mirroring `get_conversation_files`:
deduplicate ids into a CTE, then join `files` and
`ORDER BY f.created_at, f.id`. Correct the doc to "first in resolution order
(project files first, then attachments, each ordered by upload time)".

---

## 16. A-correctness-07 — `available_files` spans ALL branches; code_sandbox sees only the active branch

**File:** `src-app/server/src/modules/file/available_files.rs:318`; `code_sandbox/repository.rs:83,104-107`

**What's wrong.** `available_files` joins ALL branches
(`JOIN branches b ... JOIN branch_messages bm ON bm.branch_id = b.id`);
`code_sandbox/repository.rs:83` joins only `bm.branch_id = c.active_branch_id`.
The surfaces are separate implementations (the sandbox mount calls
`get_conversation_files`, NOT `resolve_available_files`), yet both doc-claim
parity (available_files.rs:10-13 says it feeds BOTH — false; repository.rs:104-107
says the sandbox sees "the same effective file set as the chat" — false for
attachments). Net: a file attached only in a non-active branch appears in the
manifest/read-tools but is absent from the sandbox bind-mount (active ⊆ all).
Project files are identical between paths; only attachment branch-scope diverges.

**Fix.** Make `get_conversation_files` use the same all-branches join (or, better,
have the sandbox mount call `resolve_available_files` and map → `ConversationFile`,
deleting the bespoke SQL — which also adds the ownership re-check + content-dedup
the sandbox query lacks). If active-branch scoping is intended, instead fix BOTH
doc comments to state the intentional divergence.

---

## 17. A-correctness-08 — `read_paginated` silently skips failed pages, presenting a non-contiguous range as contiguous

**File:** `src-app/server/src/modules/files_mcp/handlers.rs:302-315,328-331` (and grep 419-431)

**What's wrong.** A page whose `load_text_page` errors is handled with
`Err(_) => continue` (313) — no marker emitted, so successfully-loaded pages are
concatenated with headers that jump (page 2 → page 4). `structuredContent` still
reports the full requested span. The `out.is_empty()` fallback only fires when ALL
pages fail, so partial failures are uncaught. `grep_files` has the same class:
`.unwrap_or_default()` substitutes an empty string for a failed page, silently
losing matches. Only manifests on actual page-file corruption.

**Fix.** Replace `Err(_) => continue` with an explicit
`--- {name} · page {N} · [unreadable] ---` marker, track `missing: Vec<u32>`,
and add `"missing_pages": missing` to `structuredContent`. Apply the same to
`grep_files` (record/surface failed pages instead of silent `unwrap_or_default`).

---

## 18. A-security-01 — Unbounded model-controlled `limit` can overflow `start + count`

**File:** `src-app/server/src/modules/files_mcp/handlers.rs:299,349`

**What's wrong.** `ReadArgs.offset`/`limit` are `Option<usize>`, fully
model-controlled. `end = (start + count).min(total)` with `count =
limit.unwrap_or(...).max(1)` (unbounded). `start = offset.unwrap_or(0).min(total)`
can be ≥1, so `start + count` (1 + usize::MAX) overflows BEFORE `.min(total)`. No
`overflow-checks` override exists → debug panics (unwinds to 500), release wraps
to a garbage value (then clamped, so memory-safe). No confidentiality impact.

**Fix.** Use saturating arithmetic in both readers:
`let end = start.saturating_add(count).min(total);`. Optionally clamp the window:
`limit.unwrap_or(DEFAULT).clamp(1, MAX_*_LIMIT)`.

---

## 19. A-security-02 — `grep_files` scans full file bodies with no per-call byte cap

**File:** `src-app/server/src/modules/files_mcp/handlers.rs:387-447`

**What's wrong.** The only stopping condition is `matches.len() >= 200`, which
fires only when a match is FOUND — a zero-match file is iterated end-to-end, and
each page/file is loaded fully into memory. With `id` unset, all `f.text` files
are scanned. Unlike `read_file`, grep has no byte/line budget. Caller-scoped
(per-user own conversation), not cross-tenant; regex is linear-time (regex 1.12.3,
default 10MB size_limit). A resource-fairness nit, not a vuln.

**Fix.** Add `const GREP_MAX_SCAN_BYTES`, accumulate `scanned += text.len()`,
`break 'outer` when exceeded with a `truncated` flag surfaced in summary +
`structuredContent` (mirrors read_file). Optionally call `.size_limit(...)`
explicitly for clarity.

---

## 20. B-correctness-03 — MCP `remember` does not validate `kind` → DB CHECK violation surfaces as opaque internal error

**File:** `src-app/server/src/modules/memory_mcp/handlers.rs:213-250`; extractor `extractor.rs:34,229,289`

**What's wrong.** `RememberArgs.kind` is a free `String` passed straight to
`Repos.memory.insert(.., &args.kind, ..)` with no validation; `is_valid_kind` is
never imported/called here. `user_memories.kind` has
`CHECK (kind IN ('preference','fact','goal','relationship','other'))`. An
out-of-enum kind violates the CHECK → `database_error` → `JsonRpcError::internal`
(opaque). REST `create_memory` DOES validate (memory/handlers.rs:176-178). The
extractor (`op.kind` free String) passes unvalidated kinds to `apply_add`/
`apply_update`; there a CHECK violation is caught + logged as a warning, silently
dropping the op.

**Fix.** In `remember()`: `if !is_valid_kind(&args.kind) { return
Err(AppError::bad_request("VALIDATION_ERROR", "kind must be one of: ...")); }`. In
the extractor, clamp unknown kinds: `let kind = if is_valid_kind(&kind) { kind }
else { "other".to_string() };` so ops degrade to 'other' rather than being
dropped.

---

## 21. B-correctness-04 — RRF tie-break ordering nondeterministic across equal fused scores

**File:** `src-app/server/src/modules/memory/chat_extension/retriever.rs:333-350`

**What's wrong.** `scores` is a default `HashMap` (RandomState), so its
iteration order is randomized per-instance. `into_iter().collect()` feeds `fused`
in that order; `sort_by` keys only on the f64 score; Rust's stable sort keeps
tied elements in incoming (random) order; `.take(limit)` then truncates, making
inclusion at the cutoff vary run-to-run. Equal scores are common with RRF
(k=60). Output flows into the injected memory block, so it's user-visible.

**Fix.** Add a deterministic secondary key on the Uuid:

```rust
fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
```

---

## 22. B-correctness-05 — Content length cap uses byte length but error claims a char limit

**File:** `src-app/server/src/modules/memory_mcp/handlers.rs:234-239`; `memory/handlers.rs:169-172,223-226`

**What's wrong.** `if content.len() > MAX_CONTENT_LEN { ... "content exceeds 4000
char limit" }`. Rust `.len()` is the UTF-8 **byte** count, so the check is
byte-based while the message says "char". Duplicated at all three sites (MCP
remember + REST create + REST update). For multibyte content (CJK ~3 B/char,
emoji ~4) it rejects at ~1000-1333 chars. The rest of the memory stack counts
scalars (`estimate_tokens` uses `chars().count()`). Over-rejects, never
over-accepts. *(Note: the `remember` tool description does NOT state "4000
characters" — no tool-desc change needed.)*

**Fix.** Pick one measure for all three. Simplest (preserves "char" wording):
`if content.chars().count() > MAX_CONTENT_LEN`. Alternatively keep `.len()` and
reword to "byte limit" (and rename the constant). Keep MCP + REST create + REST
update in lockstep.

---

## 23. B-security-01 — `recall` (a read op) gated on `memory::write`, not `memory::read`

**File:** `src-app/server/src/modules/memory_mcp/handlers.rs:34,67-90,304-367`

**What's wrong.** `jsonrpc_handler` is gated with
`RequirePermissions<(MemoryWrite,)>`, and `dispatch_tool_call` routes
`remember`/`recall`/`forget` with no per-method check. `recall` is a pure read
yet requires `memory::write`. Deviates from REST (list/get use `MemoryRead`).
Net: `memory::read`-only users can't recall via MCP; a contrived write-without-read
user could. NOT a cross-tenant leak (every query anchors on `user_id`).
Administrators hold both via wildcard, so practical impact is contrived
split-permission roles.

**Fix.** Lower the handler extractor to `RequirePermissions<(MemoryRead,)>` (so
authentication still happens), thread `auth.user` into `dispatch_tool_call`, and
enforce per-tool: `memory::read` for `recall`, `memory::write` for
`remember`/`forget`, returning a permission-denied `JsonRpcError` (StatusCode::OK)
when absent.

---

## 24. C-and-loop-03 — `upsert_builtin_server` 'preserve admin's value on conflict' rationale is now dead

**File:** `src-app/server/src/modules/code_sandbox/repository.rs:162-187,211-221` (and `memory_mcp/repository.rs:40-45`)

**What's wrong.** The doc comment + ON CONFLICT clause deliberately omit
`enabled`/`display_name`/`description`/`timeout_seconds`/`usage_mode`/
`max_concurrent_sessions`, justified by "admin UI lets operators tweak these via
PATCH". But this branch's `09b81114` added a guard at
`mcp/repository.rs:1576-1581` (`if existing.is_built_in { return Err(...) }`) so
NO API path can set those columns anymore — the omission's multi-line
justification is dead/stale. The comment also cites a test
(`mcp_built_in_protection_test.rs`) that does not exist. *(The finding's
escalated "operator can no longer disable the built-in" sub-claim is NOT
code-supported: `enabled` does not gate built-in reachability — the real gate is
config `code_sandbox.enabled`. So this is doc-hygiene only.)*

**Fix (doc-only).** Rewrite the `upsert_builtin_server` doc + inline comment to
drop the false PATCH rationale and state that built-ins are immutable via the API
as of the `is_built_in` guard; remove the stale test reference. Apply the same to
`memory_mcp/repository.rs:40-45`.

---

## 25. C-and-loop-05 — `get_any_server` (auto-attach + approval bypass) does not filter on `enabled`

**File:** `src-app/server/src/modules/mcp/repository.rs:1335-1396`; call sites `chat_extension/mcp.rs:1289,1853,153-156`

**What's wrong.** `get_any_mcp_server` selects with only `WHERE id = $1` — no
`enabled` predicate. Auto-attach (1289 before_llm, 1853 after_llm) and the
approval-bypass key purely on server identity via a fetch that ignores `enabled`.
Built-ins get `needs_approval=false` and execute even in Disabled mode. **Both
mitigations are currently complete:** `update_system_mcp_server` rejects built-in
modification; `upsert_builtin_server` ON CONFLICT omits `enabled` so
re-registration never resets it; the only `enabled=false` paths
(`disable_for_health_failure`, `enforce_on_update_transition`) both exclude
built-ins. So no live path disables a built-in today — a defense-in-depth gap,
not exploitable.

**Fix.** Guard `enabled` at the auto-attach fetch in both loops:
`if let Some(s) = Repos.mcp.get_any_server(*id).await? { if s.enabled {
builtin_servers.push(s); } }`. Hardening-only; no behavior change in the shipping
config.

---

## 26. C-and-loop-06 — `clear_old_tool_results` keep-last-K cannot bound the kept-window size

**File:** `src-app/server/src/modules/chat/core/services/streaming.rs:1462-1498`

**What's wrong.** The threshold (30_000) only gates WHETHER to trim. The
surviving window is a fixed COUNT (`KEEP_LAST_TOOL_RESULTS=6`) with no per-result
size cap and no aggregate token budget (`block_text_chars` measures size but is
never consulted to bound the kept set). If the 6 newest tool_results are
individually large (e.g. several 50k-char outputs), the kept window alone can
still exceed the context limit. Mirrors Anthropic's `clear_tool_uses` default
semantics → a hardening gap in pathological agentic loops, not a correctness bug.
*(Only mutates the OUTBOUND copy; stored history is untouched and the model can
re-read, so it's safe to be more aggressive.)*

**Fix.** Optionally add a hard bound on the surviving window: (a) per-result cap —
truncate any kept tool_result whose `block_text_chars` exceeds a ceiling; and/or
(b) budget-driven trim — walk newest→oldest accumulating chars and stop keeping
once the running estimate reaches a fraction of `threshold_tokens`. Keep matching
tool_use blocks intact so the model can re-call. Add a unit test with 6 oversized
recent results asserting the post-trim estimate stays bounded.

---

## 27. frontend-03 — Copy fix applied to a dead duplicate component

**File:** `src-app/ui/src/modules/llm-provider/components/llm-models/shared/LlamaCppLlmModelSettingsSection.tsx` (+ `MistralRsLlmModelSettingsSection.tsx`)

**What's wrong.** `LlamaCppLlmModelSettingsSection` is byte-identical (modulo the
exported name, both 14818 bytes) to the rendered `LlmModelLlamaCppSettingsSection`
and has ZERO importers in `src/`. The 4096→8192 ctx-size copy fix (commit
`5567db30`) was applied to BOTH the live and the dead copy, so it shipped in dead
code. The same dead-duplicate exists for MistralRs. A divergence hazard.

**Fix.** Delete the two unused duplicates (`shared/LlamaCppLlmModelSettingsSection.tsx`,
`shared/MistralRsLlmModelSettingsSection.tsx`); `EditLlmModelDrawer.tsx` renders
only the `LlmModel*` variants. Verify with
`grep -rn "LlamaCppLlmModelSettingsSection\|MistralRsLlmModelSettingsSection" src/`
(expect zero) and `npm run check`.

---

## 28. frontend-06 — New upload-suitability advisory UI has no E2E coverage

**File:** `src-app/ui/src/modules/file/chat-extension/components/FilePreviewList.tsx:24-37,80-95`

**What's wrong.** The advisory (Alert import, `advisories` computation, warning
render) is genuinely new. The data path is correct end-to-end (backend annotates
`processing_metadata.suitability/suggestion` at upload; the store sets the full
File entity into `selectedFiles`). But a whole-tree grep of `tests/` for
`suitability`/`advisory`/`ant-alert-warning`/the suggestion strings returns
ZERO. Backend has unit tests, but no UI/E2E exercises the rendered Alert.

**Fix.** Add an E2E (`09-chat/file-upload-advisory.spec.ts`, `--workers=1`):
upload a low-suitability fixture (pptx / .zip / text-layer-less PDF) so the
backend annotates `suitability='low'`; assert `.ant-alert-warning` renders
containing the filename + suggestion copy. Use a real upload (not the mock
fixture, whose `processing_metadata` is null).

---

## 29. frontend-07 / cross-cutting-05 — Token rename + new keep<trigger validation has no updated E2E **(merged)**

**File:** `src-app/ui/src/modules/memory/components/sections/SummarizerSection.tsx:109-123`

**What's wrong.** Fields renamed `summarize_after_n_messages`→
`summarize_after_tokens` and `summarizer_keep_recent`→`summarizer_keep_recent_tokens`
with new bounds (min500/max1M/step1000; min100/max999999/step500) and a client
validator rejecting `keep >= trigger`. The logic matches the backend (migration
85 CHECKs). But grep of `tests/` for the token field names / 'Summarize after' /
'Conversation summarizer' returns nothing; none of the 11 specs in `12-memory/`
exercise the summarizer form. *(The cross-cutting-05 also bundles the suitability
advisory — see frontend-06 for that half.)*

**Fix.** Add `12-memory/summarizer-thresholds.spec.ts` (`--workers=1`): as admin,
navigate to the memory admin page, locate the "Conversation summarizer" card via
`getByLabel('Summarize after N tokens')` / `getByLabel('Keep recent tokens
verbatim')`. Assert the validation path (trigger 5000, keep 6000 → inline error
"Keep-recent (6000) must be less than the trigger (5000)." + no success toast),
then the happy path (trigger 20000, keep 4000 → "Summarizer settings saved.").

---

## 30. frontend-08 — `ModelCapabilities.context_length` surfaced for UI but no UI consumes it

**File:** `src-app/ui/src/api-client/types.ts:1456`; backend `file/available_files.rs:197,225`

**What's wrong.** `types.ts:1456` adds `context_length?: number` to
`ModelCapabilities` (but NOT `ModelCapabilities2` at 1463), and the OpenAPI
description states it's "surfaced here so the UI can show the ceiling…". But grep
of the UI for `context_length` returns ZERO non-generated hits — the only
consumer is the backend summarizer (`available_files.rs:197/225`). Both
llama.cpp settings sections render only a plain "Context Size" InputNumber bound
to `ctx_size`; neither reads the model's native ceiling or warns when `ctx_size`
exceeds it. The OpenAPI promise is unfulfilled.

**Fix (either):** (A) implement the promised UI — pass `model.capabilities?.
context_length` into the llama.cpp section, append "native max: N" to the
description, warn when `ctx_size > context_length`, and set the InputNumber `max`
to the ceiling. (B) If deferred, soften the OpenAPI doc comment to describe the
field as backend-only and track the UI affordance in a follow-up. Also confirm
the `ModelCapabilities` vs `ModelCapabilities2` asymmetry is intended.

---

## 31. cross-cutting-03 — Manifest says 'Address files by id (never by name)' but `read_file` accepts name

**File:** `src-app/server/src/modules/file/available_files.rs:233-239`; `files_mcp/tools.rs:16,21`; `files_mcp/handlers.rs:195-235`

**What's wrong.** The manifest system block says "Address files by `id` (never by
name)." But `read_file`'s description advertises "by `id` (preferred) or `name`",
declares a first-class `name` arg, and `resolve_target` genuinely implements name
resolution (matches `f.name`/`f.aka`, returns AMBIGUOUS_NAME on >1). So `name` is
a real working path the manifest flatly forbids — two model-facing strings
contradict each other. *(The finding's path citation was partly wrong; the real
conflicting pair is exactly these two files.)* Pure terminology inconsistency.

**Fix.** Since `name` is a genuinely supported, unambiguity-guarded path, soften
the manifest at `available_files.rs:238`: "Address files by `id` (preferred);
`name` works only when it uniquely identifies one file." (Alternative stricter
stance: drop the `name` arg from the schema + `resolve_target` — but that removes
a working convenience.)

---

## 32. cross-cutting-04 — No integration coverage of files_mcp read/list/grep round-trips, AMBIGUOUS_NAME, pagination

**File:** `src-app/server/tests/files_mcp/mod.rs` (109 lines)

**What's wrong.** The file has exactly 3 tests (initialize, tools/list count,
requires-conversation-id). None exercises `tools/call` against a real
conversation. Untested handler logic: `resolve_target` AMBIGUOUS_NAME /
MISSING_TARGET / not-found; `read_paginated`/`read_text_lines` pagination;
Image/Binary branches; `grep_files` ignore_case default, 200-cap, malformed-regex
fallback, INVALID_PATTERN, empty-pattern INVALID_ARGS; cross-tenant ownership
NOT_FOUND. The `grep_first_file` stub plan exists (`stub_chat.rs:254`) but no test
references it.

**Fix.** Add direct `tools/call` integration tests in `tests/files_mcp/mod.rs`
building a real conversation (reuse agentic_chat helpers): (1) `list_files`
returns the file id/name; (2) `read_file` by id with offset/limit asserts
line_start/line_end/total_lines + the continuation marker; (3) AMBIGUOUS_NAME
(two same-named files) + MISSING_TARGET; (4) `grep_files` hit + malformed-pattern
literal-escape fallback (still 200) + empty-pattern INVALID_ARGS; (5)
cross-conversation ownership → JSON-RPC error. Plus an agentic round-trip
exercising `grep_first_file`.

---

## 33. CC-add-01 — `model_supports_tools` / `model_context_window` silently swallow DB errors (completeness-critic add)

**Severity: low — completeness-critic add (verified by reading the code).**

**File:** `src-app/server/src/modules/file/available_files.rs:158,196`

**What's wrong.** Both functions use `if let Ok(Some(model)) =
Repos.llm_model.get_by_id(model_id).await` and fall through on `Err`. So a
**transient DB error** during `before_llm_call` is indistinguishable from "model
not found": `model_supports_tools` returns `false` (silently degrading a
tool-capable model to no-tools — so the files/memory built-ins won't attach for
that turn), and `model_context_window` returns `None` (silently dropping the
fraction-of-window summarizer override). The error is neither logged nor
propagated.

```rust
if let Ok(Some(model)) = Repos.llm_model.get_by_id(model_id).await {  // Err → silently false / None
    ...
}
```

**Fix.** Distinguish the not-found case from the error case: at minimum
`tracing::warn!` on the `Err` arm so a DB blip during capability resolution is
observable, e.g.
`match Repos.llm_model.get_by_id(model_id).await { Ok(Some(m)) => ..., Ok(None) => {}, Err(e) => tracing::warn!("model lookup failed during capability resolution: {e}") }`.
This dovetails with **A-correctness-06**'s memoization fix — once the boolean is
computed once and cached in `context.metadata`, the error surface shrinks to a
single site per turn, making a warn-on-error cheap and meaningful.

---

# NIT

## 34. A-correctness-10 — Manifest renders single-page documents as 'text'

**File:** `src-app/server/src/modules/file/available_files.rs:248-258`

**What's wrong.** `readable = if f.pages > 1 { "N pages" } else { "text" }`. A
plain text file has `pages == 1` (TextProcessor → `vec![text]`), and a 1-page PDF
also yields `text_page_count == 1`. So a 1-page `FileType::Document` and a plain
text file both render `readable="text"` — indistinguishable, though their read
units genuinely differ (lines for text, pages for docs). The separate `kind`
field does say "document", so the model isn't entirely blind. The doc comment on
`pages` (line 70 "0 for plain-text-in-one-page") is itself stale (plain text is
pages==1).

**Fix.** Derive the label from `file_type`: a `Document` reads "N page(s)" even at
one page; non-document text reads "text". Optionally fix the stale `pages` doc
comment.

---

## 35. frontend-04 — New arrow callbacks use parenthesized single params; Biome is 'asNeeded'

**File:** `src-app/ui/src/modules/file/chat-extension/components/FilePreviewList.tsx:30,37`

**What's wrong.** `biome.json:128` sets `arrowParentheses: "asNeeded"`. The new
block adds `.map((f) => ({` (30) and `.filter((x) => ...)` (37) — bare un-typed
single params that Biome strips to `f =>` / `x =>`. Local convention is ~96% bare
single params; sibling file-module files use bare. The destructured
`.map(({ f, meta }) => ...)` (82) correctly keeps parens. Cosmetic.

**Fix.** Drop the parens (run `npm run format` from `src-app/ui`):
`.map(f => ({` and `.filter(x => ...)`. Leave line 82 unchanged.

---

## 36. frontend-05 — Changed InputNumber lines exceed Biome lineWidth 80

**File:** `src-app/ui/src/modules/memory/components/sections/SummarizerSection.tsx:102,125`

**What's wrong.** `biome.json:11` sets `lineWidth=80`. Line 102 is 84 cols, line
125 is 82 cols; both gained a `step={...}` attr in this PR. Running
`@biomejs/biome format` exits 1 and breaks each JSX attribute onto its own line
for both. Cosmetic. *(The formatter also wants to reflow two pre-existing `extra`
prose blocks — out of scope for this finding.)*

**Fix.** `npx @biomejs/biome format --write src/modules/memory/components/
sections/SummarizerSection.tsx` (or hand-wrap only the two InputNumber elements to
keep the change surgical).

---

## 37. cross-cutting-06 — Broken column alignment after token rename in UPDATE SET

**File:** `src-app/server/src/modules/memory/repository.rs:577-588`

**What's wrong.** The SET block hand-aligns every `=` to a fixed column. The two
edited lines break it: `summarize_after_tokens` (22 chars) has only 2 spaces →
`=` too early; `summarizer_keep_recent_tokens` (29 chars) has 6 spaces → `=` past
the block. The query compiles/runs correctly. Cosmetic.

**Fix.** Realign the block to the new widest field (29 chars → align `=` to
column 30), or minimally pad `summarize_after_tokens` and reduce
`summarizer_keep_recent_tokens` to a single space before `=`.

---

## 38. cross-cutting-08 — files_mcp routes.rs omits the `.route()`-vs-`.api_route()` justification comment

**File:** `src-app/server/src/modules/files_mcp/routes.rs:9`

**What's wrong.** `memory_mcp/routes.rs:9-14` carries a comment explaining the
JSON-RPC handler uses Axum's `route()` (not aide's `api_route`) because it
dispatches multiple methods over one path and isn't a typed REST endpoint.
`files_mcp/routes.rs` uses the identical pattern correctly but drops the comment.
A future reader could "fix" it to `api_route` and try to OpenAPI-document a
JSON-RPC endpoint. No functional impact.

**Fix.** Mirror the sibling's comment above the `.route("/files/mcp", ...)` call.

---

## Completeness-critic pass — notes

- **OpenAPI drift: checked, none.** `openapi.json` is tracked at
  `src-app/ui/openapi/openapi.json` (NOT the stale `src-app/ui/src/api-client/
  openapi.json` path CLAUDE.md cites) and was regenerated on this branch
  (`+1288/-551`), carrying the new `summarize_after_tokens`,
  `summarizer_keep_recent_tokens`, `context_length` fields in sync with
  `types.ts`. No drift.
- **Concurrency:** the inline self-save + background extractor are mutually
  exclusive per turn (tool_capable branch), so no double-write race beyond the
  quota-escape already captured in B-correctness-01. No additional concurrency
  bug found.
- **Error-path leaks:** `assert_owns_conversation` correctly returns NOT_FOUND for
  both missing and foreign conversations (no cross-tenant existence leak). One
  silent-error-swallow surfaced → **CC-add-01**.
- **Terminology / model-facing contradictions:** captured
  (cross-cutting-02/03, A-correctness-10).
- **Missing tests:** captured (C-and-loop-07, frontend-06/07, cross-cutting-04/05).
