# DECISIONS — artifacts-deliverables

Every product/human input resolved up front so implementation runs nonstop. Resolutions
prefer existing convention + the substrate study; external design research (Claude
Artifacts / ChatGPT Canvas / Gemini Canvas / Notion / Cursor) informs the model-facing,
co-edit, and editor choices.

### DEC-1: New artifact entity/MCP/permission for the agent, or reuse the file substrate?
**Resolution:** Reuse. A deliverable is a file-store `File`; the agent authors + edits it
with the existing `files_mcp` tools; versioning is `file_versions`/`commit_new_version`;
viewing is the `file` panel. No new agent-facing MCP, no new artifact entity.
**Basis:** codebase — `files_mcp::create_file` already authors revisable documents,
`edit_file` already does unique `old_str`→`new_str` versioned edits, `create_file`
stamps `source_message_id`.

### DEC-2: What is the genuinely-missing work?
**Resolution:** (a) user editing (the panel is read-only; no user REST appends content),
(b) a deliverable-framed, auto-opening canvas, (c) export (user-facing, multi-format,
plus whole-conversation).
**Basis:** codebase — `versions.rs` has no append; `FilePanel`/viewers are head-bound
read-only; `convert_document` is model-only md→PDF.

### DEC-3: How does the user save an edit?
**Resolution:** A new `POST /api/files/{id}/versions` → `commit_new_version(created_by='user')`.
**Basis:** codebase — no existing endpoint writes file content; `restore_version` is the
closest mirror (ownership + `commit_new_version` + sync).

### DEC-4: How are user + model edits reconciled?
**Resolution:** Turn-based single-writer via append-only versions; `append_version`'s
`SELECT … FOR UPDATE` serializes writers; nothing lost; the model reads the latest head
on its next turn. No real-time OT/CRDT.
**Basis:** codebase (row-lock + content-addressed no-op) + research (turn-based is
correct for a single-panel chat).

### DEC-5: Which types are user-editable, and with what editor?
**Resolution:** `markdown` → Plate WYSIWYG; `code` → CodeMirror (plain text, syntax
highlight); `csv` → an editable data grid. Binary/pdf/image stay view + export. Every
editor Saves through the ITEM-1 append path.
**Basis:** user selection (code/CSV editing in scope) + fit — each editor matches its
content type; markdown is the flagship report surface, code/CSV serve technical
deliverables.

### DEC-6: Which WYSIWYG editor for markdown?
**Resolution:** Plate (`platejs`) + `@platejs/markdown`, lazy-loaded, Plate's shadcn
components adopted into the kit. Rejected for v1: Milkdown (markdown-native but not
shadcn-native), TipTap+shadcn-tiptap (viable but weaker shadcn/kit alignment), a plain
textarea (insufficient per requirement).
**Basis:** research — Plate is the best React + shadcn/ui fit, ships shadcn/Radix
components under an own-your-components model matching this repo's kit, and rounds-trips
the GFM subset via remark. `shadcn-component-discovery`/`review` runs before authoring.

### DEC-7: The file's native content stays canonical — how does the editor round-trip it?
**Resolution:** The file (markdown / source / CSV) is canonical. Markdown deserializes
on open and serializes back on save via `@platejs/markdown`, constrained to the
Streamdown-rendered GFM subset, with normalize-on-save for minimal diffs. **CORRECTION
(runtime-verified — DRIFT-1.6):** Plate does NOT preserve unmodeled constructs, it DROPS
them; so every supported GFM construct MUST have its Plate plugin (marks/headings via
basic-nodes, lists via `@platejs/list`, tables/links/images/code via the markdown plugin)
or it is lost on save. The full subset is proven lossless by `markdownRoundtrip.test.ts`.
**Basis:** codebase — files store their native format and Streamdown renders markdown;
keeping the format canonical keeps `files_mcp`/export/RAG working unchanged. Research —
GFM round-trip fidelity is imperfect, so the subset is constrained + tested.

### DEC-8: Edit engine (Plate) differs from the render engine (Streamdown) — keep both or unify?
**Resolution:** Keep both — Streamdown for the read-only view (existing pipeline reused),
Plate for edit — constrained to the same GFM subset, with a round-trip + render-parity
test so what-you-edit matches what-you-render.
**Basis:** codebase — reusing Streamdown avoids rewriting the shared render path
(chat + file viewer); unifying on Plate read-only is a larger change deferred unless
parity drift proves it necessary.

### DEC-9: How do the editor dependencies avoid bloating the app?
**Resolution:** Lazy-load each editor bundle behind Edit-mode entry (mirror
`LazyStreamdown`), so view-only users never load them; add deps at identical versions to
`ui` + `desktop/ui` (syncpack-aligned), pinned to the repo's React/TS `overrides`.
**Basis:** codebase — `LazyStreamdown` is the established lazy-load pattern;
`.syncpackrc.json` + `overrides` enforce cross-workspace parity.

### DEC-10: How is code edited?
**Resolution:** `code` files edit in a lazy-loaded, kit-adopted CodeMirror editor as
plain text (syntax highlighting only) — no AST round-trip, so Save writes exact bytes.
**Basis:** research/codebase — CodeMirror is the standard React code editor; plain-text
editing sidesteps the markdown fidelity risk (DEC-7).

### DEC-11: How is CSV edited, and how is fidelity kept?
**Resolution:** `csv` files edit in an editable data grid extending the tabular viewer
(PR #119): parse CSV → grid on open, serialize grid → CSV on save, reusing the viewer's
existing CSV parser (not a new one). Quoting/embedded-delimiter/header fidelity is
round-trip-tested.
**Basis:** codebase — the tabular viewer already parses + renders CSV; making it editable
reuses that and keeps the file canonical as CSV.

### DEC-12: How are images added to a markdown deliverable?
**Resolution:** Drag-drop/paste uploads via the existing `POST /api/files/upload`
(existing size/type limits) and inserts a markdown image reference at the cursor — so the
image survives the round-trip as a link, not embedded bytes.
**Basis:** codebase — reuses the upload endpoint + the canonical-markdown rule; Plate has
a first-class image plugin.

### DEC-13: How does the version-diff view work — frontend or backend?
**Resolution:** Frontend-only. A Compare control in `FileVersionBar` fetches two versions'
text via the existing `GET /api/files/{id}/versions/{v}/text` and renders an added/removed
diff with a small diff library, for text/markdown/code.
**Basis:** codebase — the per-version text endpoint already exists; a client diff avoids
any server change.

### DEC-14: How does the canvas surface automatically?
**Resolution:** Auto-open the `file` panel on the FIRST `create_file`/`rewrite_file` tool
result; keep the inline preview + manual "Open in side panel".
**Basis:** codebase (literature `tool_result`→`displayInRightPanel`; the file
chat-extension already renders tool-returned files inline) + UX.

### DEC-15: Deliverables list — derived, curated, or both?
**Resolution:** Both. Base list is derived (`source_message_id` ∈ conversation +
`created_by IN ('mcp','llm')`), UNION user-pinned files, MINUS user-hidden ones — curation
stored in a new `conversation_deliverables` link table (migration 132, mirrors
`project_files`; `pinned=false` = hidden). Pin/unpin emits owner-scoped `SyncEntity::Deliverable`.
**Basis:** user selected pin-as-deliverable. Derived alone can't promote a plain upload or
hide noise; a thin curation table over the derived base gives both without changing how
files are authored.

### DEC-16: Which export formats, delivered how?
**Resolution:** `md` (raw), `pdf` (pandoc + typst), and `docx | odt | rtf | html` (native
pandoc writers) via a generalized `convert_to(format)`; streamed attachment via
`content_disposition`. User file export is a download (distinct from the model's save-back
`convert_document`).
**Basis:** codebase — pandoc 3.7 ships odt/rtf/html writers needing no engine, typst is the
pdf engine, both smoke-tested; `content_disposition` + `workspace_export` are the reuse
templates.

### DEC-17: How is a conversation rendered to markdown?
**Resolution:** A new serializer (`## User`/`## Assistant` headers, text prose,
tool/thinking/code fenced, attachments/images as links) extending
`summarizer::message_to_summarizable`.
**Basis:** codebase — only a lossy `role: text` transcript exists; a faithful renderer is
the one non-trivial new backend piece.

### DEC-18: Scoping / sharing + permissions for the new endpoints.
**Resolution:** Single-owner, no ACL (handoff via export). File append/export reuse the
file endpoints' ownership + permission gating; conversation export + deliverables reuse
`conversations::read` + ownership. No new permission.
**Basis:** codebase — no ACL primitive exists; existing gating covers the reused paths.

### DEC-19: Selection → LLM — how, without a new endpoint or trust-boundary change?
**Resolution:** In v1, two flavors. Query-about (non-mutating): the selection is quoted
into the chat composer as context; the model answers in chat. Edit-selection (mutating):
the selection + instruction are sent so the model runs `edit_file(old_str=<selection>)`
landing as a new version; if the selection is not a unique substring, degrade to
instruction-only (never an ambiguous `old_str`).
**Basis:** user directive + research (Claude/ChatGPT/Gemini all ship selection editing) +
codebase — `edit_file` already does unique-`old_str` versioned edits, so this is
request-shaping through the existing tool, no new route, no trust-boundary change.

### DEC-20: Multiple open deliverables + unsaved edits.
**Resolution:** Deliverables open as tabs in the existing tabbed right panel; edit state
is per tab/`fileId`; switching, closing, or navigating away while a canvas is dirty raises
an unsaved-changes guard (Save / Discard / Cancel). One canvas is edited at a time.
**Basis:** codebase — `rightPanel.tabs[]` + `displayInRightPanel` already give tabbed
multi-file; the guard mirrors standard `beforeunload`/route-leave patterns.

### DEC-21: The model (or another device) changes a file the user is editing — how reconciled?
**Resolution:** A non-destructive choice. While editing, the canvas watches `sync:file`
for its `fileId`; if the head advances past the editor's base version, it shows a banner —
Reload latest (discard local) or Keep my changes (append as a new head via ITEM-1). Never
auto-overwrite.
**Basis:** codebase — `append_version` is row-locked + append-only (nothing lost) and
`SyncEntity::File` already fires on head change; this adds only the UI reconciliation.

### DEC-22: Desktop parity + OpenAPI.
**Resolution:** All endpoints are server-side and desktop embeds the server, so they work
on desktop unchanged; the editors + panel edits are mirrored into `src-app/desktop/ui/`
and `just openapi-regen` regenerates BOTH api-clients (new endpoints + `SyncEntity::Deliverable`
+ the widened `format` enum). Verify `npm run check` (incl. syncpack for the new deps) in
both workspaces.
**Basis:** memory — desktop embeds the server; both-workspace regen + check convention.

### DEC-23: What is explicitly OUT of v1?
**Resolution:** Deferred (each additive later without reworking the v1 data model):
comment/suggestion (track-changes) mode, project-level deliverables, workflow-run artifact
bundling, multi-user sharing/ACL, real-time co-editing, live HTML/React execution.
**Basis:** user selection — presented and not chosen for v1.
