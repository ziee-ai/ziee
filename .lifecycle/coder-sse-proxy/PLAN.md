# PLAN — Fix SSE buffering through the Coder published URL

## Context
SSE/streaming works on the direct path (`http://localhost:8080`) but is fully
buffered by the **Coder edge nginx** (`nginx/1.24.0`, wildcard TLS ingress in
front of `coderd` v2.34.2), so chat streaming and every other `/api` SSE endpoint
never deliver frames through the published URL. Root cause and a live,
end-to-end-proven fix are recorded in `coder-sse-proxy.STATUS.md` at the repo
root. The disable-buffering signal the edge honors — `X-Accel-Buffering: no` —
must be emitted by ziee's **inner** nginx, because nginx consumes the upstream
`X-Accel-*` header set by the axum server and never forwards it.

## Items
- **ITEM-1**: In `docker/web/nginx.conf` `location /api`, emit
  `add_header X-Accel-Buffering no always;` (+ `add_header Cache-Control no-cache always;`)
  so the inner nginx sends a fresh disable-buffering signal downstream to
  coderd → the Coder edge nginx, which honors it and stops buffering `/api`
  (SSE) responses. Proven live: with this change, `GET /api/sync/subscribe`
  through the Coder URL streams `event: connected` immediately (was 0 bytes/40s).
- **ITEM-2**: Add a dependency-free regression guard that asserts the SSE-critical
  directives are present in `docker/web/nginx.conf`'s `location /api` block
  (`add_header X-Accel-Buffering no` and the existing `proxy_buffering off`), so
  a future edit can't silently re-break streaming through a buffering proxy.

## Files to touch
- `docker/web/nginx.conf` — add the two `add_header` lines in `location /api`.
- `docker/web/check-sse-headers.mjs` — new Node guard script (ITEM-2).
- `docker/web/README.md` — one line documenting the guard + why the header exists.

## Patterns to follow
- **nginx SSE hardening** — mirror the existing SSE-correctness block already in
  `docker/web/nginx.conf` `location /api` (`proxy_buffering off; proxy_cache off;
  proxy_request_buffering off; chunked_transfer_encoding on;`), extending it with
  the `add_header` lines and a comment matching that block's style.
- **Header intent precedent** — the rationale mirrors the in-repo comment at
  `src-app/server/src/modules/code_sandbox/version_handlers.rs:352` (which sets
  `X-Accel-Buffering: no` on an axum SSE response); the NEW insight is that the
  header must be re-emitted at the inner nginx to survive to an *outer* proxy.
- **Standalone `.mjs` check** — mirror the repo's dependency-free Node check
  scripts (e.g. `src-app/ui/scripts/*.mjs`): pure `node:fs`, exit non-zero with a
  clear message on failure.
