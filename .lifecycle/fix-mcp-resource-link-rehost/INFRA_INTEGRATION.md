# INFRA_INTEGRATION — the three mandatory walks

## (1) User-experience walk
A user chats with an assistant that uses an org **system** MCP server (rcpa/dscc/biognosia). The
server runs a tool and returns a produced file (chart/CSV/PDF) as a `resource_link` at
`http://host.docker.internal:18123/…`.
- **Before:** `persist_links` fetch is SSRF-blocked → no `file_id` → the UI shows "Failed to load
  file content", and the LLM, handed the raw URL, rewrites the host (`127.0.0.1`, the public
  domain) to reach the bytes.
- **After:** the server's registered host is in the trust set → the fetch runs under `MCP_USER`
  → the file is ingested, `file_id` stamped → the UI renders a normal file card via
  `/api/files/{id}`, and the LLM is handed a clean ziee download URL (via the existing
  saved-artifact `hidden_content` list) which the broadened guidance tells it to use verbatim,
  never rewriting the host. The user can view/download the file; tool-to-tool hand-off works.

## (2) Infrastructure-integration walk
Subsystems this change touches and how each is handled:
- **MCP chat result pipeline** (`chat_extension/mcp.rs`, BOTH approval + auto-exec sites): trust-set
  derivation swapped to the new accessor, gated on `!server.is_built_in`. The `persist_links`
  signature/behavior is otherwise unchanged; the downstream artifact-event + download-URL minting
  loop is untouched, so system-server artifacts flow through the SAME "saved artifact" path that
  already hands the LLM a clean URL.
- **Workflow tool dispatcher** (`workflow/dispatch.rs`): same derivation swap; the existing
  `is_built_in` short-circuit is preserved, so built-in (`code_sandbox` `ziee://`) emitters skip
  the DB query as before.
- **MCP repository** (`repository.rs`): NEW read-only accessor mirroring the `list_accessible`
  accessibility predicate; the client-facing URL redaction in `list_accessible` is left intact (the
  accessor is a separate, hosts-only path). No schema change.
- **SSRF policy** (`url_validator.rs`): unchanged. The fix only changes WHICH hosts populate the
  trust set; `MCP_USER` still blocks IMDS `169.254/16` + IPv6 link-local; redirects still disabled
  on the scoped path.
- **File ingest + sync** (`file::ingest::ingest_bytes`, `publish_file_changed`): unchanged — the
  system-server artifact is a normal `File` owned by the user, identical to any other re-hosted
  `resource_link` artifact.
- **LLM guidance** (`chat_extension/mcp.rs` guidance fns, `code_sandbox/handlers.rs`
  `get_resource_link` description): text broadened to cover a tool-returned URL; no behavioral code.
- **Permissions/authz:** none introduced. The accessor reuses the existing accessibility predicate;
  it exposes no new endpoint and returns no data to clients.

## (3) Entity-lifecycle walk
The only "entity" produced is a re-hosted **file artifact** — a normal `File` row, NOT a new
entity type, store, or sync channel.
- **add:** ingested via the shared `ingest_bytes`; `file_id`/`version` stamped onto the link; the
  existing `publish_file_changed` sync + `send_artifact_created_event` fire (unchanged path).
- **delete / access-loss:** handled by the pre-existing File lifecycle (owner-scoped
  `/api/files/{id}`, normal delete cascade). No new local/sync split — nothing new subscribes.
- **trust set itself is stateless:** derived per `persist_links` call from live DB rows. If the
  system MCP server is later deleted or the user loses group access, the accessor simply stops
  returning its host on the next call — no dangling/persisted trust state to invalidate.
