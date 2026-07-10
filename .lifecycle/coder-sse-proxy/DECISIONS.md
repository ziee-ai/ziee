# DECISIONS

### DEC-1: Where to emit the disable-buffering header — axum server, inner nginx, or both?
**Resolution:** Inner nginx (`docker/web/nginx.conf` `location /api`), via
`add_header X-Accel-Buffering no always;`.
**Basis:** codebase + live evidence — nginx *consumes* the upstream `X-Accel-*`
header (the axum-set copy at `code_sandbox/version_handlers.rs:357` is absent from
the response downstream of the inner nginx), so a server-set header never reaches
the Coder edge. Re-emitting it at the inner nginx is the only lever that
propagates through coderd to the edge nginx. Live-proven to work.

### DEC-2: Set the header at the axum layer too (belt-and-suspenders for single-proxy deployments)?
**Resolution:** No — nginx-only for this fix.
**Basis:** user — approved scope is A1 (nginx `add_header`) only. The axum-layer
copy does not help the Coder (double-proxy) case (inner nginx eats it), and adding
it would pull `src-app/server/**` into the diff (triggering the full backend test
gate) for no benefit to the reported symptom. Deferred as a possible future
hardening for single-proxy topologies.

### DEC-3: Include `Cache-Control: no-cache` alongside `X-Accel-Buffering: no`?
**Resolution:** Yes — add both, matching what was proven live.
**Basis:** convention + user hunch — no-cache is correct for an authenticated API
(prevents any intermediary caching), and the SSE spec expects it. Where a handler
already sets it (sync/chat) the response carries it twice, which is valid and
harmless per RFC 7234. Buffering is disabled solely by `X-Accel-Buffering`;
Cache-Control is defensive.

### DEC-4: Scope the header to only SSE responses, or all `/api`?
**Resolution:** All `/api` (apply at the `location /api` level).
**Basis:** convention — nginx cannot cheaply condition `add_header` on
content-type without `map`/`if`; applying at the location level is the standard
idiom. `X-Accel-Buffering: no` on small non-SSE JSON responses is a no-op in
practice (they fit in one buffer), so there is no downside to the broader scope.

### DEC-5: What is the gated test for a pure nginx-config change?
**Resolution:** A dependency-free Node guard (`docker/web/check-sse-headers.mjs`)
asserting the directives are present, tier: unit; plus the manual live-`curl`
verification through the Coder edge recorded as tier: integration (not
CI-automatable).
**Basis:** codebase — mirrors the repo's `.mjs` check-script convention; the
end-to-end proof genuinely requires the live Coder ingress and cannot run in CI,
so it is a recorded manual verification, not a skipped/cosmetic test.

### DEC-6: Any operational tunable introduced (limits/timeouts/toggles)?
**Resolution:** None.
**Basis:** codebase — the change adds only static response headers; no
memory/cpu/timeout/retention/rate/quota/concurrency/toggle value is introduced,
so the configurable-settings rule does not apply.
