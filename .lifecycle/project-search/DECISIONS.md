# DECISIONS — project-search

Every input the implementation needs, resolved up front. All resolved by
existing convention (no human input required); rationale recorded so
implementation runs nonstop.

### DEC-1: Match mode — prefix or substring?
**Resolution:** case-insensitive **substring** (`ILIKE '%' || $N || '%'`).
**Basis:** convention — `modules/mcp/repository.rs` uses exactly this predicate for its list search.

### DEC-2: Query-parameter name?
**Resolution:** `search`.
**Basis:** convention — the mcp accessible-servers list endpoint names its param `search` (`mcp/handlers/user.rs`).

### DEC-3: Which columns are searched?
**Resolution:** `name` OR `description`.
**Basis:** convention — mcp searches `name`/`display_name`/`description`; projects have `name` + `description` (no display_name), so the analogous set is name + description.

### DEC-4: Case sensitivity?
**Resolution:** case-insensitive.
**Basis:** convention — `ILIKE` (not `LIKE`) throughout the mcp precedent.

### DEC-5: Blank / whitespace-only search?
**Resolution:** trimmed; blank normalizes to `None` (no predicate — returns all).
**Basis:** convention — the mcp handler does `.as_deref().map(str::trim).filter(|s| !s.is_empty())`.

### DEC-6: Does `total` reflect the filter?
**Resolution:** yes — the predicate is applied to the COUNT query as well as the page SELECT, so pagination stays consistent.
**Basis:** convention — mcp `list_accessible` applies the same predicate to both queries; a filtered list with an unfiltered total would be a pagination bug.

### DEC-7: LIKE wildcard characters in user input (`%`, `_`) — escape them?
**Resolution:** no escaping; pass the trimmed term straight into the parameterized `ILIKE`.
**Basis:** convention + security — the mcp precedent does not escape; the query is fully parameterized (no injection), and a user typing `%` only widens matching **within their own** `user_id = $1` scope, which is harmless. Pre-resolved so the security audit angle in Phase 6 has a documented answer.

### DEC-8: Frontend scope for this pilot?
**Resolution:** backend API only; the projects-list search box is a follow-up.
**Basis:** scoping — the pilot's purpose is to exercise the 8-phase gate chain with reliable, fully-runnable tests; the `?search=` API is the self-contained, integration-testable shippable unit. "User-visible value" here is the API capability the UI consumes. Recorded to make the scope boundary explicit rather than an omission.

### DEC-9: OpenAPI regeneration scope?
**Resolution:** regenerate the server UI spec (`ui/openapi/openapi.json` + `ui/src/api-client/types.ts`) via the documented server `--generate-openapi` command; also run the desktop regen if `just openapi-regen` includes the projects endpoint.
**Basis:** codebase — the `types_ts_parity` golden lib test requires the committed spec + types to be regenerated in lockstep with any schema change.
