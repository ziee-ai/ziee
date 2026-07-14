# PLAN — fix re-hosting of SYSTEM MCP servers' result files (+ tighten LLM guidance)

Backend-only change. No UI workspace (`src-app/ui/**`, `src-app/desktop/ui/**`) is
touched; no OpenAPI regen is implied (no request/response type change).

## Problem (one line)
An admin-registered **system** MCP server (`is_system=true, is_built_in=false`, e.g.
rcpa/dscc/biognosia at `host.docker.internal`) returns a result-file `resource_link`;
`persist_links` fails to re-host it because the SSRF trust set excludes on the wrong field
(`is_system` instead of `is_built_in`) and the source list has the URL redacted — so no
`file_id` is stamped and the LLM improvises host rewrites.

## Items

- **ITEM-1**: Change `trusted_hosts_from_servers` (`src-app/server/src/modules/mcp/resource_link.rs`)
  to filter on `is_built_in` (exclude only in-process built-in loopback servers) instead of
  `is_system`, so an admin-registered non-built-in system server's host vouches for its result
  files. Rewrite its doc comment, AND update the now-stale in-`persist_links` NOTE (~538-540) that
  claims system URLs are redacted → use the env opt-in (no longer true for the production callers).
- **ITEM-2**: Add a server-side-only accessor `McpRepository::list_accessible_result_link_hosts(user_id)
  -> Result<Vec<String>>` (`src-app/server/src/modules/mcp/repository.rs`) — a lean query selecting
  only `is_built_in, url` with the same accessibility predicate as `list_accessible_mcp_servers`
  plus `enabled = true`, returning `trusted_hosts_from_servers(...)`. Returns HOSTS only, never
  URLs, never serialized to a client (bypasses the `list_accessible` URL redaction that blanks
  system-server URLs).
- **ITEM-3**: Swap the 3 `trusted_hosts` derivation sites to the new accessor, gated by
  `if server.is_built_in { Vec::new() } else { … }` so the extra query fires only for external
  emitters: chat approval path (`chat_extension/mcp.rs` ~944), chat auto-exec path (~2898),
  workflow dispatcher (`workflow/dispatch.rs` ~1297). Delete the now-stale comment blocks that
  claim system servers are covered only by the env opt-in.
- **ITEM-4**: Broaden the chat LLM guidance (`chat_extension/mcp.rs` `tool_system_guidance` ~83-105
  and `saved_artifact_hidden_content_guidance` ~116-128) to cover "a file another tool handed you
  as a URL" — use the ziee-provided (`/api/files`) URL; never fetch/forward the raw MCP
  `host.docker.internal` URL; never rewrite/guess/substitute the host.
- **ITEM-5**: Add the bridge case to the `code_sandbox` `get_resource_link` tool description
  (`code_sandbox/handlers.rs` ~1248-1291): if a tool gave you a file as a URL and you had to pull
  it into the sandbox yourself, call `get_resource_link` on the local filename.
- **ITEM-6**: [DESCOPED] sandbox `execute_command` egress guard (block arbitrary internal IPs from
  the sandbox fetch). Non-trivial (`--share-net`); Part A removes the need. See DECISIONS.md
  DESCOPED disposition.

## Files to touch
- `src-app/server/src/modules/mcp/resource_link.rs` (ITEM-1; + unit tests)
- `src-app/server/src/modules/mcp/repository.rs` (ITEM-2)
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` (ITEM-3, ITEM-4; + guidance unit tests)
- `src-app/server/src/modules/workflow/dispatch.rs` (ITEM-3)
- `src-app/server/src/modules/code_sandbox/handlers.rs` (ITEM-5; + description unit test)
- `src-app/server/tests/mcp/resource_link_test.rs` (integration test for ITEM-1/2/3)

## Patterns to follow
- **Trust-set derivation**: mirror the existing `trusted_hosts_from_urls` / `trusted_hosts_from_servers`
  helpers and their `#[cfg(test)]` tests in `resource_link.rs` — same iterator-over-`(bool, url)`
  shape, same `host_of` extraction.
- **New repository accessor**: mirror `McpRepository::list_accessible` and the free `sqlx::query!`
  functions in `repository.rs` (same accessibility WHERE predicate: `s.user_id = $1 OR
  (s.is_system AND EXISTS(user_group_mcp_servers … ug.user_id = $1))`, same `AppError` mapping).
  Return a plain `Vec<String>` — no new model type.
- **Call-site gating**: mirror the existing workflow short-circuit
  `if is_built_in { Vec::new() } else { … }` at `workflow/dispatch.rs:1297` and apply the same
  shape to the two chat sites.
- **Guidance edits + tests**: mirror the existing guidance functions and their asserted-substring
  `#[cfg(test)]` tests already present in `mcp.rs` and `handlers.rs` — extend the strings and the
  substring assertions in lockstep (no test deletion; A5 shrinkage guard).
- **Integration test**: mirror the existing `persist_links` tests + `start_fixed_response_mock`
  loopback fixture and the admin-API `create_system_server` / group-assign helpers in
  `tests/mcp/` (see `resource_link_test.rs` and `tests/mcp/mod.rs`).
