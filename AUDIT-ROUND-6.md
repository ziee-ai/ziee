# Audit — Round 6

**Branch:** `feat/project-improvements`
**HEAD:** `7bebfd44`
**Scope:** Re-verification of the Round-5 Track A "recency-drop" fix for file-attachment replay.

This round did **not** converge. Round 5 claimed to make the recency-drop fire on
the user-message replay path, but verification shows the targeted method is still
unreachable for file content, and a second (pre-existing, low-severity) data-loss
window remains when file resolution fails. Two confirmed findings below.

---

## Summary

| ID | Severity | Category | File | Title |
|----|----------|----------|------|-------|
| r6-verify-transform-metadata-01 | Medium | incomplete-fix | `src-app/server/src/modules/file/chat_extension/file.rs` | Round-5 fix is a no-op: `process_content_for_llm` is never dispatched for file content (handler declares the extension name `"file"`, not the serde tags `"file_attachment"`/`"image"`) |
| r6-final-sweep-01 | Low | bug | `src-app/server/src/modules/file/chat_extension/file.rs` | Recency-drop fires even when manifest injection / files-MCP attach failed, silently losing old attachment content for that turn |

**Totals:** 2 confirmed — 0 high, 1 medium, 1 low.

---

## r6-verify-transform-metadata-01 — Round-5 fix is a no-op (dispatch mismatch)

- **Severity:** Medium
- **Category:** incomplete-fix
- **File:** `src-app/server/src/modules/file/chat_extension/file.rs` (`handled_content_types`, lines 189–191)

### What's wrong

Registry dispatch is an exact string match. `registry.process_content_for_llm`
(`registry.rs:512-517`) calls `get_handler_for_content_type(content.content_type())`,
which does `ext.handled_content_types().contains(&content_type)`
(`registry.rs:495-502`). `MessageContentData::content_type()` (`content.rs:61-67`)
serializes `self` and reads the serde `"type"` field. The enum is
`#[serde(tag="type", rename_all="snake_case")]` (`content.rs:53-54`), so the
composed variants (`chat_extensions.rs:45-46`) serialize to the tags **`"image"`**
and **`"file_attachment"`** — corroborated by integration assertions
`content_blocks[1]["content_type"] == "file_attachment"`
(`file_attachments_test.rs:431,433,521,522,629,636,643`).

But `FileExtension::handled_content_types()` returns `vec!["file"]`
(verified at `file.rs:189-191`) — the extension **name** (from
`define_extension_content!`), not the content-type tags. `"file"` matches neither
`"file_attachment"` nor `"image"`. (Contrast the text extension, which correctly
returns `vec!["text","thinking"]` at `text.rs:176-178`.)

### Consequence

On the user-message replay path (`streaming.rs:798-806`):
`process_content_for_llm` returns `Ok(None)` (no handler) → fallback
`convert_extension_to_content_block`. But `FileExtension` does not implement
`convert_extension_content` (only MCP does, matching `tool_use`/`tool_result`,
`mcp.rs:2747`; the file ext uses the trait default `Ok(None)`, `registry.rs:308`),
so the block is `None` and the persisted `FileAttachment` is dropped from replay
**unconditionally**, regardless of tool-capability.

The Round-5 commit `7bebfd44` only seeded `transform_context` metadata in
`streaming.rs` and rewrote the `file.rs` comment to claim "the recency-drop below
actually fires." It never touched `handled_content_types`, so the targeted method
stays unreachable and the seeded metadata has zero observable effect. The same
mismatch makes `should_skip_in_assistant_forwarding` (`file.rs:199-208`) and
`process_content_from_db` (`file.rs:292-298`) dead for file content (both gated by
`get_handler_for_content_type` at `registry.rs:530,569`).

Pre-existing (not introduced by the diff), but the branch has **not** converged on
the Track A recency-drop as claimed.

### Why medium (not high)

Functionally the LLM still has the auto-injected manifest + `read_file` to recover
old file contents, so this is a behavior/correctness gap and a misleading commit
message — not data loss or a security issue.

### Corrected fix

Change `FileExtension::handled_content_types()` (`file.rs:189-191`) to return the
actual serde content-type tags instead of the extension name:

```rust
fn handled_content_types(&self) -> Vec<&'static str> {
    vec!["file_attachment", "image"]
}
```

This routes both persisted variants through the registry to
`FileExtension::process_content_for_llm`, where the recency-drop (drop old
non-image `FileAttachment`s on tool-capable models, keep images inlined) and the
non-tool-capable inline fallback finally execute; it also activates
`should_skip_in_assistant_forwarding` for MCP-produced `FileAttachment` blocks on
assistant messages and `process_content_from_db` (a no-op here, harmless). Both
code paths internally re-extract via `FileContent::from_message_content` and
early-return `None` on non-file content (`file.rs:204,216`), so widening the
declared types is safe.

Add an integration/unit test that drives
`convert_history_to_messages_with_extensions` over a persisted user-message
`FileAttachment` and asserts: (a) on a tool-capable model the block is dropped from
the replayed `ChatMessage`; (b) on a non-tool-capable model the block is inlined as
a document/text `ContentBlock`; and a persisted-image case asserting the image
stays inlined on both. This would have caught both the original empty-metadata bug
and this dispatch mismatch. Mirror the `file_attachments_test.rs` setup. Keep Rust
hand-formatted (no rustfmt).

---

## r6-final-sweep-01 — Recency-drop fires without its recovery mechanism

- **Severity:** Low
- **Category:** bug
- **File:** `src-app/server/src/modules/file/chat_extension/file.rs`
  (`process_content_for_llm`, lines 235–244)

### What's wrong

The manifest insert + `attach_files_mcp` flag are set **only** inside the
`Ok(files) if !files.is_empty()` arm of `before_llm_call` (`file.rs:119-132`); the
`Err(e)` arm (`file.rs:134-137`) merely `tracing::warn!`s and continues — no
manifest, no flag. Meanwhile the recency-drop (`file.rs:235-244`) is gated **only**
on `model_supports_tools(&context.metadata)` + non-image, with no dependency on
`resolve_available_files` succeeding.

Ordering makes the two independently reachable:
`convert_history_to_messages_with_extensions` (`streaming.rs:312`, drives
`process_content_for_llm`) runs **before** `call_before_llm_call`
(`streaming.rs:350`, which calls `resolve_available_files`). They are temporally
separate, so a transient DB failure can hit one and not the other.

The drop's capability check can be entirely DB-free while
`resolve_available_files` is DB-heavy — the crux of independent failure. The
`transform_context` metadata (`streaming.rs:285-309`) is **not** seeded with
`model_tools_capable` (that key is set only by `ensure_model_tools_capable` inside
`before_llm_call`, which runs later). During replay, `model_supports_tools`
(`available_files.rs:155-201`) skips the memo and, for a cloud model whose
persisted `capabilities.tools` is `None`, falls through to the in-memory
`model_registry::lookup` catalog (lines 190-199) and returns `true` with **no DB
access**. Yet `resolve_available_files` (`available_files.rs:366-461`) issues 3–4
DB queries. A transient pool-checkout/connection error isolated to those queries
therefore drops every old non-image text attachment (`Ok(None)`) while no manifest
is injected and the files MCP server is never auto-attached
(`auto_attach_builtin_ids` gates it on `attach_files_mcp == "true"`,
`mcp.rs:127-128`).

The drop is real data loss, not a marker: `file.rs:242-244` is a bare
`return Ok(None)`, and the registry fallback also yields `None`
(`streaming.rs:773-777,801-805` → `registry.rs:580-590` → default trait impl at
`registry.rs:308`). Net for that turn: the model can neither see the old attachment
inline **nor** read it via a tool.

### Why low

Requires the confluence of a tool-capable catalog-backed cloud model + old
non-image attachments + a DB error isolated to the `resolve_available_files` calls,
and it self-heals on the next turn once resolution succeeds. The `Ok(empty)` arm is
**not** a data-loss path (an old attachment present in history would normally be in
the resolved set unless deleted/foreign, in which case there is nothing to
re-inline). Only the `Err` arm loses data.

> Note: this finding builds on the Round-5 fix and only becomes reachable once
> r6-verify-transform-metadata-01 is fixed (otherwise the drop never fires at all).

### Corrected fix

The naive gate (`context.metadata.get("attach_files_mcp") == Some("true")`) does
**not** work: that flag is set by `before_llm_call` *after* the transform pass and
on `stream_context`, never on `transform_context`, so it is always absent during
`process_content_for_llm` and would unconditionally revert the Round-5 fix.

Instead, gate the drop on the recovery being **present**, computed once before
replay. In `streaming.rs`, before building `transform_context` (~line 285), resolve
availability once and seed a boolean:

```rust
let manifest_available = matches!(
    crate::modules::file::available_files::resolve_available_files(
        conversation_id, user_id).await,
    Ok(files) if !files.is_empty()
);
context_metadata.insert(
    "files_manifest_available".to_string(),
    serde_json::Value::Bool(manifest_available),
);
```

Have `before_llm_call` reuse that already-resolved result (pass it through, or read
the flag) instead of resolving a second time, injecting the manifest + setting
`attach_files_mcp` only when true — single source of truth, one resolve per
iteration. Then in `process_content_for_llm` (`file.rs:242`) require **both**
tool-capability **and** manifest availability before dropping:

```rust
let manifest_available = context.metadata
    .get("files_manifest_available")
    .and_then(|v| v.as_bool())
    .unwrap_or(false);
if tool_capable && manifest_available && !is_image {
    return Ok(None);
}
```

This makes the drop fall back to inlining whenever resolution failed (or returned
empty), so a transient `resolve_available_files` error degrades to the pre-Track-A
safe behavior (re-inline) instead of data loss, while preserving the Round-5
success-path drop. Add a Tier-2 integration test that forces
`resolve_available_files` to `Err` on a tool-capable model and asserts the old
attachment content is still present in the replayed request rather than silently
dropped — the existing agentic_chat tests only cover the success path.

---

## Verdict

**The audit has NOT converged.** After five rounds of fixes, Round 6 found 1 medium
and 1 low finding, both in
`src-app/server/src/modules/file/chat_extension/file.rs`. The medium finding shows
the Round-5 fix is effectively a no-op (the recency-drop handler is never
dispatched for file content), and the commit message claiming it "actually fires"
is misleading. Fix r6-verify-transform-metadata-01 first; r6-final-sweep-01 then
becomes reachable and should be addressed in the same change.
