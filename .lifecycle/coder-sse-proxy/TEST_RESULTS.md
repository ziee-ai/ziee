# TEST_RESULTS

Diff touches only `docker/**` (no `src-app/**`), so no cargo/npm/e2e workspace
gate applies. Full guard log:
`/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/coder-sse-proxy-test.log`.

- **TEST-1**: PASS — `node docker/web/check-sse-headers.mjs`. Positive: OK
  (exit 0). Negative (value `no-cache` for X-Accel-Buffering): FAIL (exit 1).
  Negative (remove `proxy_buffering off`): FAIL (exit 1). Restored: OK.
  Also verified running inside the Dockerfile `config-check` stage
  (`docker build --target config-check` → guard prints OK; a missing directive
  would exit non-zero and fail the image build).
- **TEST-2**: PASS — manual live `curl -N` of `GET /api/sync/subscribe` through
  the Coder published URL, with the FINAL shipped `nginx.conf` applied to the
  live `ziee-web-ziee-web-1` container via a reversible `nginx -s reload`:
  - BEFORE the fix: 0 bytes received in 40s (edge fully buffered).
  - AFTER the fix: `event: connected` arrived immediately; `:` keepalive at
    +15s (incremental streaming). Direct response carried a single
    `cache-control: no-cache` (no duplicate). Live container restored to the
    repo original after each test (verified `diff`-identical, add_header count 0).
  Not CI-automatable (requires the live Coder edge); executed + recorded this
  session (also in `coder-sse-proxy.STATUS.md`).

## Deterministic phase-8 gates (config-only diff)
- **A2** clean working tree — PASS (all load-bearing files committed).
- **A3/A4** no diff-added skip/ignore/only, no cosmetic assertions — PASS.
- **A8** no built-in MCP server added — N/A.
- **A9/A10** no permission introduced — N/A.
- **R2-5** no `/api` e2e route-mocks added — N/A.
- No frontend workspace touched → no `npm run check` / `gate:ui` / e2e gate.
