# INFRA_INTEGRATION — the three mandatory walks

## 1. User-experience walk

A user chats with a tool-capable model. A tool (RCPA DE-analysis, an MCP server,
or the code sandbox itself) produces an artifact file (e.g. `evaluation.json`),
surfaced in chat as a `resource_link` / saved artifact (`created_by IN
('mcp','llm')`). Later the model tries to inspect it with the code-sandbox
`read_file({filename})`.

- **Before:** the call fails with `-32603 "tool read_file failed"`. The model has
  no idea why and cannot self-correct; the user sees a stuck/erroring tool.
- **After:** `read_file` returns the artifact's content; `edit_file` can modify it;
  `list_files` shows it alongside workspace files, so the model can discover the
  name to read. A genuinely-missing file now returns an actionable
  `invalid_params` ("… not found; call list_files …") the model can act on
  in-turn. Artifacts remain NOT visible to `cat` in the `execute_command` shell
  (a deliberate, STATUS-noted scope boundary) — the model uses `read_file` for
  them.

## 2. Infrastructure-integration walk (subsystems this item touches)

- **Built-in MCP dispatch (code_sandbox)** — the change lives in the JSON-RPC
  `tools/call` dispatch + the `read_file`/`edit_file`/`list_files` tool bodies.
  The error-mapping guard (`map_tool_error`) affects ALL code_sandbox tools'
  error envelopes; verified it only surfaces client-class (4xx) messages and keeps
  5xx generic (no host-path leak).
- **File module / storage** — reuses `file::available_files::model_authored_file_ids`
  (the same provenance query the files-MCP manifest uses) + `Repos.file.get_by_ids_and_user`
  (batched ownership check) + `get_file_storage().load_original(user, blob_version_id, ext)`.
  No new storage path, no new query, no migration.
- **Permissions / ownership** — no new permission. Ownership is enforced twice: the
  handler's `assert_owns_conversation` gate (unchanged), and `get_by_ids_and_user`'s
  `f.user_id = $2` filter. `model_authored_file_ids` additionally scopes by
  conversation provenance. Cross-conversation / cross-user reads are impossible
  (TEST-4 proves it live).
- **bwrap sandbox / mount** — deliberately UNTOUCHED. `get_conversation_files` and
  the bind-mount are unchanged, so `execute_command` behavior and the sandbox
  attack surface are identical. Only `read_file`/`edit_file`/`list_files` (which do
  not require a rootfs) change.
- **Sync / streaming / approval / notifications** — not involved; this is a
  read-path resolution + error-shaping change with no new events, no state
  mutation beyond what `edit_file`/`write_file` already did (writing into the
  workspace).
- **MCP tool-call history** — records the tool call as before; a now-successful
  `read_file` records `completed` instead of `failed`. No code change needed
  (the recording chokepoint is upstream and untouched).

## 3. Entity-lifecycle walk

The only entity this item holds/reads is a **conversation file** (attachment,
project file, or now a model-authored artifact). It is resolved fresh on every
tool call (no caching in this path), so there is no stale-cache surface:

- **Add**: a newly-authored artifact becomes readable/listable on the very next
  tool call (provenance via `file_versions.source_message_id`). TEST-1/-7 exercise
  add→read / add→list.
- **Remove / delete / access-loss**: resolution is re-run per call via
  `model_authored_file_ids` + `get_by_ids_and_user`, which JOIN live `files` /
  `file_versions` rows and re-check `user_id`. A deleted file simply stops
  resolving (→ the clean 404); a file whose ownership no longer matches never
  appears. There is no local vs. sync divergence because nothing is cached and no
  `sync:` handler is involved — the read path reads the DB at call time.
- **Mutate**: `edit_file` copies the artifact into the workspace on first edit and
  edits the workspace copy (existing behavior, now reachable for artifacts);
  subsequent reads see the workspace copy (workspace-first — TEST-3/-5).
- **Cross-conversation**: an artifact authored in conversation A is provenance-
  scoped to A and never resolves in B (TEST-4) — the data-leak guard on the
  widened candidate set.
