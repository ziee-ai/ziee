# TESTS — enumerated up front

The change is infrastructure config (`docker/web/nginx.conf`). It touches no
`src-app/**` Rust/TS path, so no cargo/npm/e2e gate applies. The automatable,
CI-runnable guard is a **config-content regression test** (a future edit must not
silently drop the SSE-critical directives); the full end-to-end proof through the
Coder edge is a manual `curl` verification (not CI-automatable — it needs the live
Coder ingress) recorded in `coder-sse-proxy.STATUS.md` and the phase-8 results.

- **TEST-1** (tier: unit) [covers: ITEM-1, ITEM-2] file: `docker/web/check-sse-headers.mjs` — asserts: parsing `docker/web/nginx.conf`, the `location /api` block contains BOTH `add_header X-Accel-Buffering no` AND `proxy_buffering off`; the script exits non-zero if either is missing (guards against a future edit re-introducing edge buffering).
- **TEST-2** (tier: integration) [covers: ITEM-1] file: `coder-sse-proxy.STATUS.md` — asserts: manual before/after `curl -N` of `GET /api/sync/subscribe` through the Coder published URL — before the fix 0 bytes/40s (buffered), after the fix `event: connected` arrives immediately + `:` keepalive at +15s (incremental streaming). Not CI-automatable (requires the live Coder edge); executed live this session and recorded.
