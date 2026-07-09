# DECISIONS — artifacts-deliverables (v5: + code/CSV editing + diff + pin + image + multi-format)

Resolved up front. v3 keeps v2's reuse-the-file-substrate core and upgrades the editing
surface from a plain textarea to a rich WYSIWYG editor per requirement.

### DEC-1: New artifact entity/MCP/permission, or reuse the file substrate?
**Resolution:** Reuse. Deliverable = a file-store `File`; agent authoring stays on
`files_mcp`; versioning on `file_versions`/`commit_new_version`; viewing on the `file`
panel. No new table, MCP, permission, or migration.
**Basis:** codebase — `files_mcp::create_file` already authors revisable documents,
`edit_file` already does unique `old_str`→`new_str` versioned edits, `create_file`
stamps `source_message_id`.

### DEC-2: What is the genuinely-missing work?
**Resolution:** (a) user editing (panel is read-only; no user REST appends content),
(b) auto-open + deliverable framing, (c) export (docx + user button + conversation).
**Basis:** codebase — `versions.rs` has no append; `FilePanel`/viewers are head-bound
read-only; `convert_document` is model-only md→PDF.

### DEC-3: How does the user save an edit?
**Resolution:** New `POST /api/files/{id}/versions` → `commit_new_version(created_by='user')`.
**Basis:** codebase — no existing endpoint writes file content; `restore_version` is the
closest mirror.

### DEC-4: How are user + model edits reconciled?
**Resolution:** Turn-based single-writer via append-only versions; `append_version`'s
`SELECT … FOR UPDATE` serializes writers; nothing lost; the model sees the latest head
on its next turn.
**Basis:** codebase (row-lock + content-addressed no-op) + external research
(turn-based is correct for a single-panel chat).

### DEC-5: Which types are user-editable in v1, and with what?
**Resolution:** three editable types, each with the right tool — **`markdown` →
Plate WYSIWYG**, **`code` → CodeMirror** (plain-text, syntax-highlighted), **`csv` →
an editable data grid** (extending the tabular viewer). Binary/pdf/image stay
**view + export**. Every editor Saves via the same append-version path (ITEM-1).
**Basis:** user selection pulled code/CSV editing into v1. Fit — WYSIWYG suits prose,
CodeMirror suits code (no round-trip risk, plain text), a grid suits tabular data; each
maps to the file's type. (Supersedes v3/v4's "markdown only".)

### DEC-6: Which WYSIWYG editor?
**Resolution:** **Plate (`platejs`) + `@platejs/markdown`**, lazy-loaded, with Plate's
shadcn components adopted into the kit. Considered + rejected for v1: Milkdown
(markdown-native but not shadcn-native — more custom styling), TipTap + shadcn-tiptap
(popular, but Plate's component-ownership + shadcn alignment fits this repo better),
plain textarea (v2 — insufficient per requirement).
**Basis:** external research — Plate is repeatedly cited as the best React + shadcn/ui
fit, ships shadcn/Radix components under an own-your-components model that matches this
repo's kit, and `@platejs/markdown` round-trips the GFM subset (tables/task-lists/
strikethrough/code/blockquotes/footnotes) via remark. Implementation runs
`shadcn-component-discovery`/`shadcn-component-review` before authoring.

### DEC-7: Markdown stays the source of truth — how does the editor round-trip it?
**Resolution:** The file's markdown is canonical. On open, `markdownToEditor(md)`
deserializes; on save, `editorToMarkdown(value)` serializes back to GFM markdown,
constrained to the Streamdown-rendered subset, with a normalize-on-save pass for stable
minimal diffs. Constructs the editor does not model are **preserved verbatim, never
dropped**.
**Basis:** codebase — files store markdown and Streamdown renders it; keeping markdown
canonical avoids a second storage format and keeps `files_mcp`/export/RAG working
unchanged. External research — Plate/remark support GFM round-trip but fidelity is
imperfect, so the subset is constrained + tested.

### DEC-8: Edit engine (Plate) differs from render engine (Streamdown) — keep both or unify?
**Resolution:** Keep both: Streamdown for the read-only view (existing pipeline reused),
Plate for edit. Constrain the editable feature set to the Streamdown-rendered GFM subset
and add a round-trip + render-parity test so what-you-edit matches what-you-render.
**Basis:** codebase — reusing Streamdown for view avoids rewriting the entire render
path (chat + file viewer share it); unifying on Plate read-only is a larger, riskier
change deferred unless parity drift proves it necessary.

### DEC-9: How does the editor dependency avoid bloating the app?
**Resolution:** Lazy-load the editor bundle behind Edit-mode entry (mirror
`LazyStreamdown`), so it never loads for view-only users; add the dep at identical
versions to `ui` + `desktop/ui` (syncpack-aligned), pinned to the repo's React/TS
`overrides`.
**Basis:** codebase — `LazyStreamdown` is the established lazy-load pattern;
`.syncpackrc.json` + the `overrides` block enforce cross-workspace version parity.

### DEC-10: How does the canvas surface automatically?
**Resolution:** Auto-open the `file` panel on the FIRST `create_file`/`rewrite_file`
tool result; keep inline preview + manual "Open in side panel". No new pin flag in v1.
**Basis:** codebase (literature `tool_result`→`displayInRightPanel`; the file
chat-extension already renders tool-returned files inline) + UX.

### DEC-11: Deliverables list — derived, curated, or both?
**Resolution:** **Both.** The base list is derived (`source_message_id` ∈
conversation + `created_by IN ('mcp','llm')`), UNION user-**pinned** files, MINUS
user-**hidden** ones — curation stored in a new `conversation_deliverables` link table
(migration 132, mirrors `project_files`; `pinned=false` = hidden). Pin/unpin emits a new
owner-scoped `SyncEntity::Deliverable`.
**Basis:** user selected pin-as-deliverable. Derived alone can't promote a plain upload
or hide noise; a thin curation table over the derived base gives both without changing
how files are authored. (Supersedes v2–v4's "derived, no table".)

### DEC-12: Which export formats, delivered how?
**Resolution:** `md` (raw), `docx` (pandoc native), `pdf` (pandoc + typst); streamed
attachment via `content_disposition`. New `convert_to_docx` is the only new converter.
User file export is a download (distinct from the model's save-back `convert_document`).
**Basis:** codebase — pandoc 3.7 + typst embedded, md→docx/pdf smoke-tested;
`content_disposition` + `workspace_export` are the reuse templates.

### DEC-13: How is a conversation rendered to markdown?
**Resolution:** A new serializer (`## User`/`## Assistant` headers, text prose,
tool/thinking/code fenced, attachments/images as links) extending
`summarizer::message_to_summarizable`.
**Basis:** codebase — only a lossy `role: text` transcript exists; a faithful renderer
is the one non-trivial new backend piece.

### DEC-14: Scoping / sharing + permissions for the new endpoints.
**Resolution:** Single-owner, no ACL (handoff via export). File append/export reuse the
file endpoints' ownership + permission gating; conversation export + deliverables reuse
`conversations::read` + ownership. No new permission.
**Basis:** codebase — no ACL primitive exists; existing gating covers the reused paths.

### DEC-15: Desktop parity + OpenAPI.
**Resolution:** Endpoints are server-side (desktop embeds the server → work unchanged);
frontend + editor mirrored into `src-app/desktop/ui/`; `just openapi-regen` both
api-clients; `deliverables` reuses the `File` schema (endpoint-surface regen only).
Verify `npm run check` (incl. syncpack for the Plate dep) in both workspaces.
**Basis:** memory — desktop embeds the server; both-workspace regen + check convention.

### DEC-16: Selection-scoped "have the model edit this highlighted range" in v1?
**Resolution:** **In v1** (un-deferred per requirement). Two flavors: **query-about**
(non-mutating — quote the selection into chat, model answers) and **edit-selection**
(mutating — seed a targeted `edit_file(old_str=<selection>)` that lands as a new
version). A selection popover exposes both in the canvas.
**Basis:** user directive + external research (Claude "Edit with Claude" / ChatGPT /
Gemini all ship selection editing) + codebase (`files_mcp::edit_file` already does
unique-`old_str` targeted edits, so edit-selection is prompt-shaping, not new backend).

### DEC-17: Multiple open deliverables + unsaved edits.
**Resolution:** Multiple deliverables open as **tabs** in the existing tabbed right
panel (no new surface). Edit state is tracked **per tab/`fileId`**; switching tabs,
closing a tab, or navigating away while a canvas is dirty raises an unsaved-changes
guard (Save / Discard / Cancel). One canvas is edited at a time (the active tab).
**Basis:** codebase — `rightPanel.tabs[]` + `displayInRightPanel` already give tabbed
multi-file; the dirty guard mirrors standard `beforeunload`/route-leave patterns.

### DEC-18: The model (or another device) changes a file the user is editing — how is it reconciled?
**Resolution:** Turn it into an explicit, non-destructive choice. While editing, the
canvas watches `sync:file` for its `fileId`; if the head advances past the editor's
base version, it shows a banner — **Reload latest** (discard local, load new head) or
**Keep my changes** (append the user's edit as a new head via the ITEM-1 path). Never
auto-overwrite. The model always reads the current head on its next turn.
**Basis:** codebase — `append_version` already row-locks + is append-only
(nothing lost), and `SyncEntity::File` already fires on head change; this adds only the
UI reconciliation on top of existing signals.

### DEC-19: How is selection→edit wired without a new endpoint or a trust-boundary change?
**Resolution:** The selection popover composes a normal chat send carrying the exact
selected text + the user's instruction as a small structured-context field; the model
performs the edit through the standard `files_mcp::edit_file` tool
(`old_str=<selection>`), which is append-only + restorable. If the selection is not a
unique substring, degrade to instruction-only (never emit an ambiguous `old_str`).
Query-about carries the selection as quoted context only (no tool call).
**Basis:** codebase — `edit_file` requires a unique `old_str` and already versions +
gates edits; shaping the request client-side reuses that path with no new server route
and no new trust boundary. (Supersedes v3 DEC-16's "deferred".)

### DEC-20: How is code edited, and does it share the markdown round-trip risk?
**Resolution:** `code` files edit in a lazy-loaded, kit-adopted **CodeMirror** editor as
**plain text** (syntax highlighting only) — no markdown/AST round-trip, so Save writes
the exact bytes. Same lazy-load + syncpack + peer-pin discipline as Plate (DEC-9).
**Basis:** research/codebase — CodeMirror is the standard React code editor; plain-text
editing sidesteps the fidelity risk that markdown WYSIWYG carries (DEC-7).

### DEC-21: How is CSV edited, and how is round-trip fidelity kept?
**Resolution:** `csv` files edit in an **editable data grid** that extends the existing
tabular viewer (PR #119) — parse CSV → grid on open, serialize grid → CSV on Save,
reusing the viewer's existing CSV parser (not a new one). Quoting/embedded-delimiter/
header fidelity is round-trip-tested.
**Basis:** codebase — the tabular viewer already parses + renders CSV/TSV; making it
editable reuses that rather than adding a parser, and keeps the file canonical as CSV.

### DEC-22: How are images added to a markdown deliverable?
**Resolution:** Drag-drop or paste into the WYSIWYG uploads via the existing
`POST /api/files/upload` (existing size/type limits enforced) and inserts a **markdown
image reference** at the cursor — so the image survives the ITEM-7 serialize as a link,
not embedded bytes.
**Basis:** codebase — reuses the upload endpoint + the markdown-canonical rule (DEC-7);
Plate has a first-class image plugin.

### DEC-23: How does the version-diff view work — frontend or backend?
**Resolution:** Frontend-only. A **Compare** control in `FileVersionBar` fetches two
versions' text via the existing `GET /api/files/{id}/versions/{v}/text` and renders an
added/removed line (or word) diff with a small diff library. No backend, for
text/markdown/code types.
**Basis:** codebase — the per-version text endpoint already exists; a client diff avoids
any server change.

### DEC-24: Which export formats does v5 expose, and via what?
**Resolution:** `md` (raw), `pdf` (typst), `docx | odt | rtf | html` (native pandoc
writers) — a generalized `convert_to(format)` replaces the single `convert_to_docx`, and
the `?format=` enum on the file + conversation export endpoints widens accordingly. Each
format is smoke-tested against the embedded pandoc.
**Basis:** codebase — pandoc 3.7 ships odt/rtf/html writers needing no engine; the
generalization is a small extension of the ITEM-2 converter. (Extends DEC-12.)

### DEC-25: What stays OUT of v1 after the v5 additions?
**Resolution:** Deferred (available on request): comment/suggestion (track-changes) mode,
project-level deliverables, workflow-run artifact bundling, multi-user sharing/ACL,
real-time co-editing, and live HTML/React execution.
**Basis:** user selection — these were presented and not chosen for v1; each is additive
later without reworking the v5 data model.
