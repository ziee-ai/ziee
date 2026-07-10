# PLAN_AUDIT — audited against the codebase

## Breakage risk
- **ITEM-1** adds two `add_header` directives inside `location /api`. nginx
  `add_header` is additive and applies to all response codes with `always`; it
  does not alter routing, proxying, or upstream behavior. `X-Accel-Buffering: no`
  on a *non-SSE* `/api` JSON response merely disables edge buffering for that
  small response (negligible). No existing caller breaks. Verified live: plain
  `GET /api/auth/me` still returns normally with the patched config; SSE now
  streams through Coder.
  - **Duplicate `Cache-Control: no-cache`**: some SSE handlers (sync, chat) already
    emit `cache-control: no-cache` from the server, so the response carries the
    header twice. Per RFC 7234 duplicate identical directives combine harmlessly.
    Confirmed in the live experiment (two `Cache-Control: no-cache` lines, no
    client issue).
- **ITEM-2** is a standalone Node script + a README line — zero runtime surface,
  cannot break the app.

## Pattern conformance
- **ITEM-1** extends the EXISTING SSE-correctness block in the same `location /api`
  (`proxy_buffering off` …). Comment style matches. Conforms.
- **ITEM-2** mirrors the repo's dependency-free `.mjs` check-script convention
  (`node:fs`, non-zero exit + message). Conforms.

## Migration collisions
- None. This change adds no SQL migration. Highest existing migration
  (`…153`) is untouched. No collision.

## OpenAPI regen
- Not required. No request/response type, route, or permission changes. No
  `openapi.json` / `api-client/types.ts` regen in either workspace.

## Per-item verdicts
- **ITEM-1** — verdict: PASS — extends the existing `location /api` SSE block;
  additive `add_header`; no caller breakage; live-proven end-to-end through Coder.
- **ITEM-2** — verdict: PASS — standalone `node:fs` guard mirroring the repo's
  `.mjs` check convention; no runtime surface.
