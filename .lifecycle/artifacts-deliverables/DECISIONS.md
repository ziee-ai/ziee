# DECISIONS — artifacts-deliverables

Every product/human input the implementation needs, resolved up front so
implementation runs nonstop. Resolutions prefer existing convention + the
substrate study; the external design research (Claude Artifacts / ChatGPT
Canvas / Gemini / Notion / Cursor) informs the model-facing and co-edit choices.

### DEC-1: Is an artifact a new entity or a file-store file with a type?
**Resolution:** A file-store `File` (which already carries content + versions +
viewers + sync + export) **designated** as a conversation deliverable by a thin
`artifacts` link row (`id, conversation_id, file_id, title, artifact_type`). Not
a parallel content/version subsystem.
**Basis:** codebase — files are already versioned (`file_versions`,
`commit_new_version`), viewered, synced (`SyncEntity::File`), and pandoc-exportable;
the link-table-over-a-real-entity shape mirrors `project_files`/`project_bibliography`.
Research also recommends building ON an existing file-store rather than a bespoke silo.

### DEC-2: How is version history stored — full snapshots or diffs?
**Resolution:** Full snapshots per version, reusing `file_versions` (append-only,
content-addressed sha256 no-op, restore-appends-new-version). No diff chains.
**Basis:** codebase (that is exactly how `file_versions` works) + research
(snapshot-per-version is the simple, fragility-free choice for small text docs).

### DEC-3: How is an artifact identified/referenced across turns?
**Resolution:** A **server-issued** `artifact_id` (uuid) returned in the
`create_artifact` tool result; the model MUST pass it to
`update_artifact`/`rewrite_artifact`/`get_artifact`.
**Basis:** research — Claude's model-chosen string identifiers silently fork a
duplicate artifact on a typo; a server-issued id eliminates that failure class.

### DEC-4: How does the model create/update — full replace, targeted edit, or diff?
**Resolution:** Four tools: `create_artifact(title, artifact_type, content)`,
`update_artifact(artifact_id, old_str, new_str)` (exact-literal replace,
must match **exactly once**), `rewrite_artifact(artifact_id, content)`
(full replace for structural changes), `get_artifact(artifact_id)`.
**Basis:** research (Claude's create/update/rewrite is the proven, token-cheap
shape) + codebase (`files_mcp::edits` already implements exact str-replace-once —
reuse it). Explicitly NOT OpenAI's LLM-authored regex `update` (a ReDoS +
wrong-span-match correctness risk).

### DEC-5: Do artifact tool calls require per-call approval or bypass it?
**Resolution:** Bypass approval — add `artifacts_server_id()` to
`is_builtin_server_id`. No `is_trusted_resource_emitter` edit (the tools return
`is_saved:true` structuredContent, never `ziee://<host-path>` links).
**Basis:** codebase — artifact writes touch only the caller's own,
append-only-versioned, always-restorable data; `files_mcp`/`citations` (own-data,
low-risk) bypass, while `control_mcp` (mutates external API state) does not. Artifacts
match the former.

### DEC-6: How are concurrent model + user edits reconciled?
**Resolution:** Turn-based single-writer. Each save (model tool OR user `PUT`)
appends a new version that becomes head. The model always re-reads the latest via
`get_artifact`; a stale `update_artifact` (`old_str` not uniquely present in the
current head) returns a fail-loud `is_error`, prompting a re-read. No real-time
OT/CRDT.
**Basis:** research (turn-based single-writer is the correct, cheap model for a
single-panel chat; real-time collab is expensive and low-value here) + codebase
(`commit_new_version` is already the atomic append point).

### DEC-7: What is the user's editing surface in the canvas?
**Resolution:** A markdown-**source** `Textarea` edit-mode with an explicit Save,
for text types only (`markdown|code|csv`); the view mode reuses the existing file
viewer registry for rich rendering. No new rich-text/code-editor dependency.
**Basis:** codebase — there is NO editor primitive in the app today
(no TipTap/ProseMirror/CodeMirror/Monaco/contentEditable); the `Textarea`+Save
idiom is established by `CoreMemoryBlocksEditor`. Research supports restricting
direct editing to text types (Claude edits Markdown only).

### DEC-8: What does the panel tab persist — inline content or a pointer?
**Resolution:** A pointer `{ artifactId, fileId, version? }`; content is fetched
live from `Stores.Artifact`/`Stores.File`.
**Basis:** codebase — the `file` panel already uses the pointer pattern; the
literature panel's inline-serialized `data` lives in a 30-day-TTL localStorage
snapshot, which is wrong for a durable, syncable, shareable deliverable.

### DEC-9: How does the canvas open?
**Resolution:** Auto-open the canvas on a `create_artifact` tool result; provide a
persistent inline `ArtifactToolResultCard` ("Open canvas") to re-open it later.
**Basis:** codebase (literature's `tool_result` card → `displayInRightPanel`) +
UX (the deliverable should surface the moment it is created).

### DEC-10: Which export formats, and how are they delivered?
**Resolution:** `md` (raw serializer output), `docx` (pandoc native writer),
`pdf` (pandoc + embedded typst engine). Delivered as a streamed HTTP attachment
via the RFC-5987 `content_disposition` helper.
**Basis:** codebase — pandoc 3.7 + typst are embedded and both md→docx and
md→pdf were smoke-tested against the shipped binaries; `convert_to_pdf` +
`content_disposition` + `workspace_export` are the reuse templates.

### DEC-11: How is a conversation rendered to markdown for export?
**Resolution:** A new serializer emitting `## User`/`## Assistant` role headers,
text as prose, `tool_use`/`tool_result`/`thinking`/code as fenced blocks, and
`file_attachment`/`image` as links — extending the block-filtering approach of
`summarization::summarizer::message_to_summarizable`.
**Basis:** codebase — only a lossy `role: text` transcript builder exists today; a
faithful markdown renderer is genuinely new but bounded to the known
`MessageContentData` variant set.

### DEC-12: What is the scoping / sharing model?
**Resolution:** Single-owner (the conversation's owner). No multi-user ACL /
sharing. Cross-user access returns 404. "Co-edit" means the agent and the owning
user both append versions to one file.
**Basis:** codebase — neither `files` nor `projects` has any ACL primitive (both
are strictly `user_id` + `ON DELETE CASCADE`, "no sharing" explicit in the
projects migration). Adding an ACL is out of scope; handoff is via export.

### DEC-13: How does an artifact relate to projects?
**Resolution:** v1 artifacts are conversation-scoped. The underlying file remains
attachable to a project through the existing `project_files` mechanism (no new
project↔artifact UI in v1).
**Basis:** codebase (`project_files` already links arbitrary files to projects) +
scope control (keeps v1 focused on the conversation deliverable + export).

### DEC-14: What permission gates the feature?
**Resolution:** A new `artifacts::use`, granted to the default `Users` group by
migration 133; admins inherit via `*`. It gates the whole MCP surface + the user
REST + artifact export; conversation-owned reads additionally enforce ownership.
Conversation export reuses `conversations::read`.
**Basis:** codebase — mirrors `citations::use`/`web_search::use` (one permission
gating a built-in's whole surface, granted to Users via an idempotent migration).

### DEC-15: How does this behave on desktop?
**Resolution:** Fully available. Artifacts is a server-side module and the desktop
app embeds the server, so no module blocklist entry is added; the `artifact`
frontend module is mirrored into `src-app/desktop/ui/` and both api-clients are
regenerated.
**Basis:** memory — desktop embeds the server; `just openapi-regen` regenerates
BOTH `ui/` and `desktop/ui/`, each with its own types/Permissions/AppEvents.

### DEC-16: Which migration numbers?
**Resolution:** `132_create_artifacts_table.sql` and
`133_grant_artifacts_permissions_to_users.sql`.
**Basis:** codebase — `ls migrations/` tail is `131_…`; 132/133 are the next free
numbers, verified no collision.

### DEC-17: What is the artifact-type vocabulary?
**Resolution:** `markdown | code | csv | html` for v1. Each maps to a file
extension/mime so the existing viewer registry routes rendering; `markdown|code|csv`
are user-editable, `html` is view+export only.
**Basis:** research (prose/code first-class everywhere; a first-class tabular type
fits the life-science audience) + codebase (the viewer registry already renders
markdown/tabular/code/web). Live React/HTML *execution* is deliberately excluded
(no concrete need; would require sandbox wiring).

### DEC-18: What chat-extension order does the attach-flag extension use?
**Resolution:** `23` (between `file`=20/`control_mcp`=22 and the built-ins at
24–29), which is < the MCP collector's `30`.
**Basis:** codebase — the order table requires any flag-setting extension to run
before the collector at 30; 23 is a free slot.

### DEC-19: Is a new user REST endpoint needed to append content, or can an existing one be reused?
**Resolution:** A new `PUT /api/artifacts/{id}` (title and/or content) appends a
version via `commit_new_version` (`created_by='user'`).
**Basis:** codebase — the existing `/api/files/*` routes expose upload + restore
but NO "append a version from user-supplied bytes"; the user side of co-edit needs
this new endpoint.

### DEC-20: Is selection-scoped "have the model edit this highlighted range" in v1?
**Resolution:** Deferred beyond v1. v1 ships direct user edit (DEC-7) plus the full
model `create`/`update`/`rewrite`/`get` surface. A future selection→instruction
affordance can seed `update_artifact`'s `old_str` server-side.
**Basis:** research (a nice-to-have, not core to any incumbent's baseline) + scope
control. Non-blocking for the v1 data model or tool surface (it is purely additive
later).
