# FIX_ROUND-1 — merge ledger, fix, re-audit

## Confirmed findings fixed (from the phase-6 ledger + the re-audit angles)

- **perms-authz HIGH (loopback-SSRF)** — `trusted_hosts` was built from `accessible_servers`, which is
  augmented with auto-attached BUILT-IN servers (via `get_any_server`, un-redacted `url`); built-ins
  carry a loopback `url` (`http://127.0.0.1:<port>/…`), leaking `127.0.0.1` into the trust set → an
  external `resource_link` at `http://127.0.0.1:<port>` bypassed the default loopback block.
  **Fixed**: new `trusted_hosts_from_servers` helper EXCLUDES `is_system` servers (covers all
  built-ins), used by all three call sites; regression TEST-11 added. (DEC-10.)
- **tests-quality HIGH (false-green)** — the redirect test's target was unreachable, so `saved==0`
  held whether or not redirects were disabled. **Fixed**: the mock now 302-redirects to a SECOND
  reachable loopback mock serving 200, so the assertion fails if redirect-disabling is reverted.
- **concurrency medium/low (env-mutation UB)** — the integration TEST-8 and unit TEST-5 mutated the
  process-global `ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE` in the parallel test binary (data-race UB +
  subprocess inheritance). **Fixed**: removed all `set_var` from tests — TEST-5 is now a read-only
  default check; TEST-8 relocated to a PURE unit test of the env-opt-in decision→policy→validate chain.
- **concurrency low (mock task leak)** — the mock accept loop was never cancelled. **Fixed**: mock
  returns a `MockGuard` that aborts the accept task on drop.
- **perf low (wasted query)** — the workflow site ran `list_accessible` even for built-in emitters
  (which never consult `trusted_hosts`). **Fixed**: guarded behind `if !is_built_in`.
- **security low (imprecise docs)** — the "IMDS/link-local blocked" claim was imprecise: `MCP_USER`
  allows IPv6 ULA (`fc00::/7`), so an IPv6-only ULA metadata endpoint is not blocked. **Fixed**: doc
  comments + CLAUDE.md now state IPv4 link-local/IMDS `169.254.0.0/16` + IPv6 `fe80::/10` blocked,
  RFC1918 + IPv6 ULA allowed.

## Rejected findings (documented, not defects)

- **security medium (union-of-hosts confused deputy)** — by-design per the user's explicit
  AskUserQuestion choice ("any enabled registered server's host"); scoped to the acting user's own
  accessible servers; documented (DEC-8, CLAUDE.md).
- **patterns-conformance low (no `.no_proxy()`)** — consistent with the sibling public/global fetch
  paths (which also omit it and carry the same headers); not a regression (DEC-9).

## Re-audit

A fresh blind round was run on the loopback-fixed diff (see FIX_ROUND-2.md). The loopback-SSRF and
false-green fixes above were themselves discovered by the phase-6 re-audit angles (perms-authz,
tests-quality), so this round DID surface new confirmed findings and is not the convergence round.

**New confirmed findings:** 2
