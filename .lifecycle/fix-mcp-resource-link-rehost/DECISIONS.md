# DECISIONS — resolved up front

Every decision below is resolved by codebase convention or the human's plan approval — none left open.

### DEC-1: Which field marks a server whose host must NOT be trusted — `is_system` or `is_built_in`?
**Resolution:** `is_built_in`. Exclude only in-process built-in servers (loopback `127.0.0.1`);
admin-registered non-built-in system servers (`is_system=true, is_built_in=false`, real external
URL) DO vouch for their result-file host.
**Basis:** codebase — built-ins are the servers listening on loopback (`is_builtin_server_id`
roster; deploy `seed.sql` seeds rcpa/dscc/biognosia as `is_built_in=false`). The original
`is_system` exclusion existed solely to keep built-in loopback hosts out of the trust set; that
concern is exactly `is_built_in`.

### DEC-2: How to get system servers' hosts past the `list_accessible` URL redaction?
**Resolution:** Add a dedicated server-side-only repository accessor
`list_accessible_result_link_hosts(user_id) -> Vec<String>` that returns HOSTS only (never URLs),
computed from the un-redacted `list_accessible_mcp_servers` data. Do NOT un-redact the shared
`list_accessible` / `get_all_accessible_config` path.
**Basis:** codebase/security — the `url=None` redaction (`repository.rs:639-643`) is a deliberate
client-facing feature (a non-admin must not learn the admin URL); un-redacting the shared list
would re-expose URLs to clients. A hosts-only accessor confines the un-redacted data to an
internal SSRF-policy decision.

### DEC-3: Also strip loopback hosts from the trust set as belt-and-suspenders?
**Resolution:** No. The precise guard is identity (`is_built_in`).
**Basis:** codebase/convention — loopback stripping would break the existing
`http_link_matched_trusted_host_is_ingested` test (which trusts `127.0.0.1`) and the documented
same-host multi-container deployment, and would change existing behavior for user-registered
loopback servers. Scope creep + regression; rejected.

### DEC-4: Rewrite the LLM-facing HTTP `resource_link` URI to `/api/files/{id}` after ingest?
**Resolution:** No. Keep the existing behavior — only `ziee://` host-path links get their URI
rewritten; an HTTP link keeps its URI and gets `file_id` stamped (UI renders via `/api/files`).
The LLM is separately handed a clean ziee download URL via the existing saved-artifact
`hidden_content` mechanism; ITEM-4 guidance steers it to that URL.
**Basis:** codebase — a deliberate comment in `persist_links` states external→external artifact
chaining relies on NOT rewriting HTTP URIs. Changing it is out of scope and risks that path.

### DEC-5: Does this feature introduce an operational tunable that should be an admin settings row?
**Resolution:** No new tunable. The trust set is DERIVED from actually-registered servers, not a
knob. The existing `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE` env opt-in is left untouched (interim
operator mitigation, the user's to set). The security boundary (which hosts earn the private
policy) is intentionally NOT admin-configurable — it must not be weakenable into a blanket private
allow; it is a security boundary fixed to "hosts of registered, accessible, enabled servers."
**Basis:** security — the Phase-4 configurable-settings rule's explicit exception for a security
boundary that must not be operator-weakened.

## Descope dispositions
- DESCOPED: ITEM-6 — sandbox `execute_command` egress guard: bwrap `--share-net` makes a hard egress filter non-trivial (needs `--unshare-net`/Landlock-NET/egress proxy — none enabled) and Part A removes the need. [approved: khoi — plan approval 2026-07-13]
