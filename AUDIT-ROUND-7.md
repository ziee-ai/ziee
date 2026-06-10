# Audit Round 7 — Confirmed Findings

Branch: `feat/project-improvements`
Scope: Track A file/project chat-extension restructure (manifest + `files` MCP),
agentic-loop seeding, project knowledge fan-out.

All four findings below were independently reproduced from current code
(`real: true`). No high or medium-severity correctness defect produces data
loss; the medium finding is a token/confusion waste on a content-dedup edge.

## Summary

| ID | Severity | Category | File | Title |
|---|---|---|---|---|
| r7-verify-trackA-restructure-01 | medium | incomplete-fix | `file/chat_extension/file.rs` | Current-turn IMAGE upload inlined TWICE when byte-identical to another conversation file |
| r7-verify-trackA-restructure-02 | low | bug | `file/chat_extension/file.rs` | Current-turn NON-image upload with EMPTY text body is not inlined |
| r7-verify-trackA-interactions-01 | low | perf | `chat/core/services/streaming.rs` | `seed_available_files` runs unconditionally per loop iteration even when never read |
| r7-final-sweep-01 | low | consistency | `project/chat_extension/project.rs` | Project knowledge files vanish on resolve-failure (gate mismatch vs file ext) |

Counts by severity: **0 high, 1 medium, 3 low** (4 total).

---

## r7-verify-trackA-restructure-01 — Current-turn image double-inlined under content-dedup (medium)

**File:** `src-app/server/src/modules/file/chat_extension/file.rs`
**Category:** incomplete-fix · **Severity:** medium

The current user message's image content blocks are persisted
(`streaming.rs:101-134`) BEFORE the spawned task runs `seed_available_files`
(`streaming.rs:315`), so the current upload is included in the
`attachment_file_ids` resolved by `resolve_available_files` and is subject to
dedup. `dedup_by_checksum` (`available_files.rs:522-544`) keeps the FIRST entry
in resolution order as canonical and folds later same-checksum files into `aka`
(a `Vec<String>` of filenames only — the dropped file's uuid does not survive).
The current upload is the NEWEST attachment, so when its bytes match a prior
attachment or a project file it is the one dropped from `avail`.

In `before_llm_call` (`file.rs:160-167`), `is_image` is decided via
`avail.iter().find(|f| f.id == *file_id)` → for the dropped current image this
returns `None` → `unwrap_or(false)` → NOT skipped → `process_file_blocks`
inlines it as a `ContentBlock::Image`. Meanwhile the replay path
(`process_content_for_llm`, `file.rs:251-264`) classifies the same upload by its
stored `mime_type` (`image/...` → `is_image=true`), so the drop at line 262
(gated on `!is_image`) KEEPS it and re-inlines it — unconditional on `avail`
membership. Net: the current image is produced TWICE in one request. No
block-level dedup exists between assembly and the provider.

Effect: wasted tokens and possible model confusion, not data loss — medium is
correct. No `#[cfg(test)]` or integration test covers content-identical-image
double-inline.

**Fix:** In `before_llm_call`, classify images by the file's OWN `mime_type`
(mirroring `process_content_for_llm`) instead of by `avail` membership, so
content-dedup can't hide a current upload. Fetch the file row, enforce the
`user_id` ownership check, `continue` on `image/` mimes, otherwise
`process_file_blocks`. Add a Tier-2 test: turn 1 upload+attach image A, turn 2
upload byte-identical image B (`B.checksum == A.checksum`, `B.id != A.id`),
assert the turn-2 main-generation `ChatRequest` contains the image exactly once.
Add a dedup regression case asserting a current-turn image folded into a
sibling's `aka` is still inlined exactly once. Hand-format ~80 cols; no rustfmt.

---

## r7-verify-trackA-restructure-02 — Empty-text non-image upload not inlined (low)

**File:** `src-app/server/src/modules/file/chat_extension/file.rs`
**Category:** bug · **Severity:** low

An empty-text send (`text.rs:61-63` returns `Ok(Vec::new())` on empty text; the
send path has NO empty-content guard — the `content.trim().is_empty()` check at
`handlers/messages.rs:98` is `edit_message` only) plus a `file_id` produces a
persisted user message whose ONLY block is the `file_attachment`
(`file.rs:72-80`). That message is committed (`streaming.rs:101-134`) BEFORE
history is fetched (line 197), so it enters replay. In replay,
`process_content_for_llm` (`file.rs:262-264`) drops the non-image attachment
when `tool_capable && manifest_available` with no current-turn exemption; the
fallback `convert_extension_to_content_block` (`streaming.rs:824-826`) returns
`None` for `file_attachment` (`registry.rs:308`), so `all_blocks` is empty and
`streaming.rs:838 if !all_blocks.is_empty()` skips pushing the User message
entirely.

`before_llm_call` then inlines `file_blocks` only via `request.messages
.last_mut()` guarded on `last_message.role == Role::User` (`file.rs:182-184`).
With no current User message pushed, the last message is the System manifest
(first turn) or a prior Assistant message (multi-turn) → guard fails →
`file_blocks` silently discarded. Net: the current non-image upload is inlined
ZERO times.

Severity LOW (not data loss): `resolve_available_files` selects
`content_type IN ('file_attachment','image')` including the current-turn block,
so `render_manifest` lists the file and the model can recover it via `read_file`.
The bug is a behavioral inconsistency — empty-text upload demoted to
read-on-demand instead of inlined like the non-empty-text path. No test covers
empty-text + `file_id`.

**Fix:** In `before_llm_call` (`file.rs:182-186`), when the last message is not
a User turn, push a fresh `ChatMessage { role: Role::User, content: file_blocks }`
instead of dropping the blocks. Safe because the System manifest is at index 0,
so the appended User turn keeps correct ordering. Add a Tier-2 test sending
`content == ""` + a non-image `file_id` on a tool-capable model and assert the
provider request contains the file content inlined in a User message (not only
in the manifest). If the team prefers to keep the demote-to-`read_file`
behavior, the minimum is a documenting comment at `file.rs:182` plus a test
pinning that contract — but inlining matches user intent.

---

## r7-verify-trackA-interactions-01 — `seed_available_files` runs unconditionally (low)

**File:** `src-app/server/src/modules/chat/core/services/streaming.rs`
**Category:** perf · **Severity:** low

`seed_available_files()` is called unconditionally at `streaming.rs:315-320`
inside the agentic tool-calling `loop {` (line 217), so it runs once per
iteration and is NOT gated on tool-capability. It invokes
`resolve_available_files()` (`available_files.rs:419`) = 3-4 DB queries
(`project_id_for_conversation`, conditional `list_file_ids`, the multi-table
attachment JOIN, `get_by_ids_and_user`). The seeded set is read in EXACTLY two
places, both in `file/chat_extension/file.rs` and both gated on `tool_capable`:
manifest injection (`file.rs:114`, `if tool_capable && manifest_available`) and
the replay recency-drop (`file.rs:262`). The other `resolve_available_files`
caller (`files_mcp/handlers.rs:164`) is a separate HTTP path that does NOT read
the seeded metadata, so gating the seed does not affect it. When the model is
not tool-capable both readers short-circuit before reading the seed → pure
wasted work.

`ensure_model_tools_capable(&mut context_metadata).await` is already called
immediately before (lines 304-307) with its return value discarded, so the
gating boolean is available at zero extra cost. Skipping the seed when
non-tool-capable leaves `files_manifest_available()` returning `false` (absent
key → `unwrap_or(false)`), exactly the value both readers already require to be
true — so behavior is identical. Regression confirmed against history: at
`7bebfd44` the resolve lived inside `if tool_capable {` in `file.rs`; `ebf1e00c`
moved it to the unconditional `streaming.rs` seed.

**Fix:** Capture the bool returned by `ensure_model_tools_capable()` (currently
discarded at line 304) and wrap the `seed_available_files()` call in
`if tool_capable { ... }`. Behavior-equivalent; eliminates 3-4 wasted DB queries
per loop iteration on the common non-tool-capable / zero-file path. Keep
hand-narrow ~80-col formatting; no rustfmt. No new tests required (pure
non-behavioral optimization).

---

## r7-final-sweep-01 — Project knowledge vanishes on resolve-failure (low)

**File:** `src-app/server/src/modules/project/chat_extension/project.rs`
**Category:** consistency · **Severity:** low

`project.rs:191` gates the inline-skip on `tool_capable` ALONE
(`let project_blocks = if tool_capable { Vec::new() } else if ...`). The file
extension that is supposed to compensate by exposing those files via the
manifest + `files` MCP tools gates on BOTH conditions at `file.rs:114`
(`if tool_capable && manifest_available`), and the `attach_files_mcp` flag that
auto-attaches the `files` MCP server is set ONLY inside that same block
(`file.rs:125`). `manifest_available` is seeded by `seed_available_files`
(`streaming.rs:315`) via `resolve_available_files(...).await.unwrap_or_default()`
(`available_files.rs:256-262`): a resolver `Err` swallows to an empty set,
seeding `files_manifest_available = false`. The resolver's step 1
(`available_files.rs:419-431`) is exactly what loads project knowledge files
(`project_id_for_conversation` → `project_files.list_file_ids`) and propagates
DB errors via `?`, so a transient DB blip there fails the whole resolver.

On such a failure for a tool-capable model: project ext (order 8) skips inlining
because `tool_capable == true`; file ext (order 20) skips the manifest AND never
sets `attach_files_mcp` because `manifest_available == false` → the `files` MCP
server is not attached either. Net: the model sees neither the project files'
content nor any tool to read them for that turn — defeating the no-data-loss
fallback the file ext explicitly documents (`file.rs:247-250`). Both extensions
share the same seeded `context.metadata` via the same `&mut StreamContext`, so
the project ext can read the identical gate.

Severity LOW: transient-only, no cross-tenant/security impact, just a per-turn
loss of project-file visibility on a DB blip. The resolve-failure fallback is
untested.

**Fix:** In `project.rs` `before_llm_call`, match the file ext's gate so the
inline path is retained whenever the manifest is unavailable OR the model is
non-tool-capable. Read `files_manifest_available(&context.metadata)` and change
the skip head to `if tool_capable && manifest_available { Vec::new() }`. Add a
regression test (alongside `agentic_chat::manifest_injected_and_read_file_round_trips`)
that forces `resolve_available_files` to fail and asserts a tool-capable model
still receives inlined project knowledge when `manifest_available == false`.
Hand-format ~80 cols; no rustfmt.
