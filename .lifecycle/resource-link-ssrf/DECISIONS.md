# DECISIONS — resource-link SSRF fix

### DEC-1: Which SSRF policy allows the private artifact fetch?
**Resolution:** Reuse the existing `OutboundUrlPolicy::MCP_USER` (allow_localhost + allow_private,
allow_link_local=false) — never a blanket private-allow, and never a new bespoke literal. It permits
RFC1918/loopback but keeps IMDS/link-local (169.254.0.0/16, fe80::/10) blocked.
**Basis:** codebase — `MCP_USER` (url_validator.rs:108) exactly models "admin-trusted MCP surface,
private allowed, cloud-metadata blocked"; mirrors the intent of `SEARXNG_POLICY`.

### DEC-2: How is the same-host match scoped — exact host:port, or host only?
**Resolution:** Host only, case-insensitive, port ignored. The registered MCP endpoint (e.g.
`:9004`) and its artifact server (e.g. `:9005`) share the host; matching host-only is what makes the
same-host multi-container case work.
**Basis:** user — chose "any enabled accessible server's host"; task symptom (RCPA `:9004` →
artifact `:9005`, same host `172.21.0.1`).

### DEC-3: How are off-host redirects prevented from inheriting the private allowance?
**Resolution:** On the scoped (`PrivateScoped`) path, build the client with redirects DISABLED
(`.redirect(reqwest::redirect::Policy::none())`). Artifact download URLs are direct 200s; a redirect
simply fails (logged, not saved). The env-opt-in (`PrivateGlobal`) path keeps the default validated
redirect policy, which re-validates each hop against `MCP_USER` (operator opted into all-private).
The public path is unchanged.
**Basis:** user + convention — task requires "off-host redirect must NOT inherit the allowance";
`discover.rs:250-252` is the redirect-disabled-validated-client precedent.

### DEC-4: Is the release env opt-in a fixed toggle or an admin settings row?
**Resolution:** A release-honored process **env var** `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE=1`
(off by default), NOT an admin settings row. It is a deployment-topology escape hatch (like a
bind-address or proxy setting), read at fetch time via `std::env::var` with no cfg-gate. It is a
security-relaxation knob, so it stays an operator-only env var — not surfaced in the admin UI where
a non-expert could footgun the SSRF boundary. IMDS stays blocked even when set (uses `MCP_USER`).
**Basis:** user — chose "scoped trust + release env opt-in"; convention — mirrors the existing
`*_ALLOW_LOOPBACK` seam shape but promoted to release-honored (the first such), matching the
"align with the pending provider SSRF opt-in" note. The configurable-settings rule's fixed-constant
exception applies: this is a security boundary that must not be admin-weakened through the UI.

### DEC-5: How are system-registered (admin) same-host MCP servers handled, given url redaction?
**Resolution:** `trusted_hosts` is built from the accessible-server list in scope
(`accessible_servers` in chat; `list_accessible(...).servers` in workflow). `McpRepository::list_accessible`
redacts `url` on **is_system** servers, so a same-host external server registered as a *system*
server won't appear in `trusted_hosts`. The common deployment registers these as **user-owned**
(`rcpa-user`/`dscc-user`), whose `url` is NOT redacted → scoped trust works. For the system-server
edge, `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE=1` (DEC-4) is the catch-all. Documented in a code
comment at the call sites + CLAUDE.md. No new un-redacted repo method is added (keeps the change
minimal and avoids a second hot-path query in chat).
**Basis:** codebase — `list_accessible` redaction (repository.rs:632-635); user — env opt-in already
chosen as the general escape hatch.

### DEC-6: Should the LLM-facing artifact URI be rewritten to `/api/files/{id}` after ingest?
**Resolution:** No. HTTP-fetched links keep their original URI; only the `file_id`/version are
stamped back (which is what the frontend uses to render the card via `/api/files/{id}`). Rewriting
the raw MCP URL to a ziee-authenticated path would break external-container→external-container
chaining (the next container cannot fetch a ziee-authenticated `/api/files` path). Display is fixed
purely by successful ingest + the existing `file_id` stamp.
**Basis:** codebase — `MessageFilesView.tsx` resolves the card from `link.file_id`;
`useResourceLinkContent` refuses non-`/api/` URLs (so display was the only breakage);
`resource_link.rs:501` already rewrites ONLY `ziee://` host-path links, HTTP links intentionally
keep their URI.

### DEC-7: Precedence order among the policy inputs?
**Resolution:** `debug MCP_RESOURCE_LINK_ALLOW_LOOPBACK` (existing debug seam, highest) →
`ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE` (global) → same-host match (scoped) → `PUBLIC_HTTP_OR_HTTPS`
(default). Debug seam stays highest so existing debug tests are unaffected; global-before-scoped
means an operator who opts into all-private gets the more-permissive redirect-following behavior
uniformly.
**Basis:** codebase — preserves the existing debug seam; user — env opt-in is a deliberate global
relaxation.
