# Chat rendering showcase seed

A single, deterministic chat conversation that exercises **every renderable
block type** in the chat UI — for eyeball QA of markdown, code, tool calls,
files, thinking, and elicitation rendering.

Import is **idempotent** (fixed UUIDs + `ON CONFLICT DO NOTHING`), so re-running
just no-ops. The conversation id is always
`11111111-1111-1111-1111-111111111111`.

## Files in this directory

| File | Purpose |
|------|---------|
| `showcase.sql`      | The seed. Heavily commented, organized into sections (Markdown / Thinking / Tool calls / Files / Elicitation+streaming) each ending in an `-- add more here --` anchor. |
| `generate_files.py` | Generates the binary + text assets (PNG, JPG, multi-sheet XLSX, PDF, CSV, .py, .md, large .txt) into `files/`. Deps: `Pillow`, `openpyxl`. |
| `load.sh`           | Resolves the target DB + owner user, generates assets if missing, copies bytes into the file store, and runs `showcase.sql`. |
| `files/`            | The generated assets (safe to delete — `load.sh`/`generate_files.py` recreate them). |

## How to load

Prereqs: the server has **booted at least once** against the target DB (so the
built-in `mcp_servers` rows exist) and **first-run setup is complete** (so a
root admin user exists). `psql` + `python3` on PATH.

```bash
cd src-app/server/seeds/showcase
./load.sh
```

By default it targets the embedded dev Postgres (`…@127.0.0.1:54323/postgres`)
and the dev file store (`<server>/../../ziee-data/dev/app-data/files`), and
assigns ownership to the root admin (`users.is_admin = true`).

Override any of those:

```bash
DATABASE_URL='postgresql://postgres:password@127.0.0.1:54323/postgres' \
OWNER='<user-uuid>' \
FILES_DIR='/path/to/app-data/files' \
./load.sh
```

Then open the conversation in the chat UI (as the owner user) and scroll top to
bottom. The mcp tool-call **Calls** tab (per built-in server) shows the 13
recorded calls, including the failed + cancelled ones.

## What it covers

- **Markdown:** all 6 heading levels, bold/italic/strike/inline-code, ordered /
  unordered / nested / task lists, simple + wide tables, nested blockquotes,
  `hr`, links, footnotes, inline image, LaTeX inline + block math, two Mermaid
  diagrams, fenced code in rust/python/typescript/sql/bash/json/yaml/diff/html,
  a deliberately long code block + long prose block (scroll tests).
- **Thinking** block (with `metadata.token_count`).
- **Tool calls** (built-in MCP servers, realistic args + results):
  `code_sandbox.execute_command` (stdout/stderr/exit + resource_link chart),
  `web_search` + `fetch_url` (typed `structured_content`), `lit_search`,
  `memory` remember+recall, `citations.format_citations`, `control.call_api`,
  `get_tool_result`, plus a **failed**, a **cancelled**, a **large/truncated**,
  and a **mixed** (text + tool_use + tool_result + resource_link file) turn.
- **Files:** PNG + JPG (as `image` blocks), PDF / CSV / multi-sheet XLSX / .py /
  .md / large .txt (as `file_attachment` blocks), a tool-returned `resource_link`
  file, and 2 project files. On-disk bytes land at
  `<FILES_DIR>/originals/<owner>/<file_id>.<ext>`.
- **Elicitation** request block, and a long streaming-style assistant message.

## How to add a new case

1. **A new block/turn:** add `pg_temp.msg(...)` + `pg_temp.blk(...)` calls in the
   relevant `SECTION` of `showcase.sql`, using the next free message UUID
   (`30000000-…-NN`) and ordinal. Content is a `jsonb_build_object('type', …)`
   matching one of the `MessageContentData` variants
   (text / thinking / image / file_attachment / tool_use / tool_result /
   elicitation_request).
2. **A tool call** also needs a row in the `SECTION C-rows` `mcp_tool_calls`
   INSERT (for the Calls tab).
3. **A new file:** add a generator fn in `generate_files.py`, a `files` +
   `file_versions` row in section 0 of `showcase.sql`, and an entry in
   `load.sh`'s `FILE_MAP`. Recompute its `sha256` for the `checksum`.

## Notes / signal for the human

- **Content types actually registered by the UI** (`ContentRenderer` +
  extension registry): `text`, `thinking`, `image`, `file_attachment`,
  `tool_use`, `elicitation_request`, and `tool_result`. Anything else falls
  through to a literal **"Unknown content type: …"** block — a useful smoke
  signal if a future variant is added without a renderer.
- `tool_use` is rendered by `McpToolUseRenderer`, which **pairs** the matching
  `tool_result` (by `tool_use_id`) under a "Show details" expander. The
  `tool_result` block itself is claimed by the **file** extension's
  `MessageFilesView` (renders `resource_links` / attachments inline) or the
  **literature** card — so a `tool_result` carrying *only text* leans on the
  tool_use pairing for display. Verify a text-only tool_result still shows its
  text where you expect.
- The inline markdown image points at a non-existent URL on purpose (exercises
  the `<img>` layout / broken-image state without bundling an external asset).
- Math + Mermaid render via Streamdown's built-in remark-math / mermaid support;
  if a build strips those plugins, those blocks are the canary.
