# DECISIONS — artifacts-deliverables (v2)

Resolved up front so implementation runs nonstop. v2 reflects the code-grounded
re-scope: reuse the existing file substrate; build only the missing top layer.

### DEC-1: Do we build a new artifact entity / MCP / permission, or reuse the file substrate?
**Resolution:** Reuse. An "artifact/deliverable" is a file-store `File`. Agent
authoring stays on the existing `files_mcp` tools (`create_file`/`edit_file`/
`edit_file_lines`/`rewrite_file`); versioning stays on `file_versions` +
`commit_new_version`; viewing stays on the `file` right-panel. NO new `artifacts`
table, NO new `artifacts` MCP, NO new permission, NO migration.
**Basis:** codebase — `files_mcp::create_file` is documented as "author a document the
user can view and revise across turns," `edit_file` already does unique
`old_str`→`new_str` with restorable versions, and `create_file` stamps
`source_message_id`. The v1 plan's artifacts table/MCP/permission duplicated all of
this. (Supersedes v1 DEC-1..DEC-5, DEC-14, DEC-16, DEC-17, DEC-18.)

### DEC-2: What is the genuinely-missing work?
**Resolution:** Three things: (a) **user editing** (the panel is read-only; no user
REST appends content), (b) **auto-open + deliverable framing** (created files open on
click, not automatically), (c) **export** (only model-tool md→PDF exists; no docx, no
user button, no conversation export).
**Basis:** codebase — verified `file/handlers/versions.rs` has list/get/restore but no
append; `FilePanel`/viewers are "head-bound" read-only; `convert_document` is
markdown→PDF, model-tool-only.

### DEC-3: How does the user save an edit — new endpoint or reuse an existing one?
**Resolution:** New `POST /api/files/{id}/versions` appending a version via
`commit_new_version(created_by='user')`. No existing endpoint writes file content.
**Basis:** codebase — the `/api/files/*` routes expose upload + restore only;
`restore_version` is the closest write path to mirror (ownership + `commit_new_version`
+ sync), differing only in that the bytes come from the request.

### DEC-4: How are user + model edits reconciled?
**Resolution:** Turn-based single-writer via the existing append-only model. Every save
(user REST or model tool) appends a new head through `commit_new_version`; the
`append_version` `SELECT … FOR UPDATE` row lock serializes concurrent writers; nothing
is lost (all versions kept, any restorable). The model always operates on the current
head, so it naturally sees user edits on its next turn.
**Basis:** codebase (`append_version` already row-locks + is content-addressed no-op) +
external research (turn-based single-writer is the correct model for a single-panel
chat; real-time OT/CRDT is unwarranted).

### DEC-5: Which file types are user-editable in the canvas?
**Resolution:** Text types only — `markdown | code | csv | text`. PDF / image / office
stay view-only (no sane source editor; they are already rendered by their viewers).
**Basis:** codebase (the viewer registry already routes these) + external research
(Claude restricts direct editing to Markdown/text).

### DEC-6: What editing widget does the canvas use?
**Resolution:** A markdown-**source** `Textarea` with an explicit Save. No new
rich-text/code-editor dependency.
**Basis:** codebase — there is NO editor primitive in the app
(no TipTap/ProseMirror/CodeMirror/Monaco/contentEditable); `CoreMemoryBlocksEditor`'s
`Textarea`+Save is the established idiom.

### DEC-7: Do we add an "artifact panel" type, or edit the existing `file` panel?
**Resolution:** Edit the existing `file` panel (add a view/edit toggle). No new panel
type; the panel data stays `{ fileId, version? }` (pointer + server fetch).
**Basis:** codebase — the `file` panel already renders any file with a version bar; a
parallel panel type would duplicate it. Every file becomes editable by its owner, which
is acceptable (single-owner model).

### DEC-8: How does the canvas surface automatically (the "deliverable" feel)?
**Resolution:** Auto-open the `file` panel on the FIRST appearance of a
`create_file`/`rewrite_file` tool result; keep the existing inline preview +
"Open in side panel" for manual re-open. No new "pin" flag in v1.
**Basis:** codebase (literature's `tool_result`→`displayInRightPanel`; the file
chat-extension already renders tool-returned files inline) + UX (the deliverable should
appear the moment it is authored) + scope control.

### DEC-9: How is "the list of deliverables in this conversation" obtained — new table or derived?
**Resolution:** Derived. `GET /api/conversations/{id}/deliverables` queries files whose
`file_versions.source_message_id` is in the conversation and `created_by IN ('mcp','llm')`,
reusing the `available_files` ownership join. No new table, no new column.
**Basis:** codebase — `create_file` already stamps `source_message_id`, and
`resolve_available_files` already scopes files to a conversation by ownership; the
association exists and only needs a read query.

### DEC-10: Which export formats, and how delivered?
**Resolution:** `md` (raw), `docx` (pandoc native writer), `pdf` (pandoc + embedded
typst). Delivered as a streamed HTTP attachment via the RFC-5987 `content_disposition`
helper. A new `convert_to_docx` sibling of `convert_to_pdf` is the only new converter.
**Basis:** codebase — pandoc 3.7 + typst are embedded; md→docx and md→pdf were
smoke-tested against the shipped binaries; `convert_to_pdf` + `content_disposition` +
`workspace_export` are the reuse templates.

### DEC-11: Is file export the same thing as the model's `convert_document`?
**Resolution:** No — keep them separate. `convert_document` is a model tool that
saves a PDF back into the file store. The new `GET /api/files/{id}/export?format=` is a
user-facing **download** in a chosen format (md/docx/pdf), not a save.
**Basis:** codebase — different callers (model vs user), different outcomes (persist vs
download), different format sets.

### DEC-12: How is a conversation rendered to markdown for export?
**Resolution:** A new serializer emitting `## User`/`## Assistant` headers, text as
prose, `tool_use`/`tool_result`/`thinking`/code as fenced blocks, `file_attachment`/
`image` as links — extending `summarizer::message_to_summarizable`'s block handling.
**Basis:** codebase — only a lossy `role: text` transcript builder exists today; a
faithful renderer is the one non-trivial new backend piece, bounded to the known
`MessageContentData` variant set.

### DEC-13: What is the scoping / sharing model, and which permissions gate the new endpoints?
**Resolution:** Single-owner (no ACL / sharing; handoff is via export). File
append-version + file export reuse the file endpoints' existing ownership + permission
gating (mirror `restore_version`); conversation export + deliverables reuse
`conversations::read` + ownership. No new permission.
**Basis:** codebase — neither `files` nor `projects` has any ACL primitive; the existing
file/conversation permissions already gate the reused paths.

### DEC-14: Desktop parity + OpenAPI.
**Resolution:** All endpoints are server-side and desktop embeds the server, so they
work on desktop unchanged; the frontend edits are mirrored into `src-app/desktop/ui/`
and `just openapi-regen` regenerates BOTH api-clients. The `deliverables` response reuses
the existing `File` schema, so the regen is endpoint-surface only (no new domain type).
**Basis:** memory — desktop embeds the server; `just openapi-regen` covers both
workspaces, each with its own types/Permissions; verify `npm run check` in both.

### DEC-15: Is selection-scoped "have the model edit this highlighted range" in v1?
**Resolution:** Deferred beyond v1. v1 ships direct user edit + the existing model
`create_file`/`edit_file`/`rewrite_file` surface. A future selection→instruction
affordance can seed `edit_file`'s `old_str` server-side — purely additive.
**Basis:** external research (a nice-to-have, not baseline) + scope control.
