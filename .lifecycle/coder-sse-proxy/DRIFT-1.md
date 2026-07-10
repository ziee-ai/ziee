# DRIFT-1 — implementation vs plan

Compared `git diff origin/khoi...HEAD` against PLAN.md items.

- **DRIFT-1.1** — verdict: none — ITEM-1 implemented exactly as planned: both
  `add_header X-Accel-Buffering no always;` and `add_header Cache-Control no-cache
  always;` added inside `location /api`, extending the existing SSE-correctness
  block with an explanatory comment. Matches DEC-1/3/4.
- **DRIFT-1.2** — verdict: none — ITEM-2 implemented as planned:
  `docker/web/check-sse-headers.mjs` (dependency-free `node:fs`) brace-matches the
  `location /api` block and asserts both `proxy_buffering off` and `add_header
  X-Accel-Buffering no`; exits non-zero when either is missing (verified both
  directions). Matches DEC-5.
- **DRIFT-1.3** — verdict: none — `docker/web/README.md` updated with one line
  documenting the header rationale + the guard script, as planned (Files to touch).
- **DRIFT-1.4** — verdict: none — no `src-app/**`, migration, or OpenAPI surface
  touched, consistent with BASE.md/PLAN_AUDIT.md. No axum-layer header added
  (DEC-2).

**Unresolved drifts:** 0
