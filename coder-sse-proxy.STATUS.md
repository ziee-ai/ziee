# STATUS — SSE/streaming broken through the Coder published URL

**Worker:** coder-sse-proxy · **Branch:** `fix/coder-sse-proxy-buffering` (off `khoi`)
**State:** DONE — PR #132 opened against `khoi` (https://github.com/ziee-ai/ziee/pull/132). 9/9 feature-lifecycle green; `.lifecycle` stripped from the tip; live container restored + healthy.

## Fix shipped
`docker/web/nginx.conf` `location /api`: `add_header X-Accel-Buffering no always;`
(Cache-Control was dropped after the blind audit — it caused a duplicate header on
non-SSE `/api` responses and is irrelevant to buffering; the SSE handlers already
set `cache-control: no-cache` at the server). Guard: `docker/web/check-sse-headers.mjs`,
run on every image build via a Dockerfile `config-check` stage. Re-verified live
with the FINAL config: `/api/sync/subscribe` streams `event: connected` immediately
+ `:` keepalive at +15s through the Coder URL; single `cache-control: no-cache` (no
duplicate). Live container restored to repo original after each test.

---

## TL;DR root cause

Streaming (SSE) works direct (`http://localhost:8080`) but is **fully buffered**
by the **Coder edge nginx** (`nginx/1.24.0 (Ubuntu)`, the wildcard
`*.workspace.tinnguyen-lab.com` TLS ingress) in front of Coder `coderd` v2.34.2.
Plain `/api` requests are fine — only chunked SSE bodies get held.

The disable-buffering signal that edge nginx would honor — `X-Accel-Buffering:
no` — never reaches the edge, because **ziee's own inner nginx (1.22.1) consumes
the upstream `X-Accel-*` header** (standard nginx behavior) and does not forward
it. The one endpoint that sets it (`code_sandbox` install/subscribe) is therefore
also buffered at Coder.

**Fix (A1):** have the *inner* nginx re-emit the header to the edge via
`add_header X-Accel-Buffering no always;` (+ `Cache-Control no-cache always;`) in
`docker/web/nginx.conf` `location /api`. One lever unbuffers every SSE endpoint.

---

## Evidence (reproduced this session)

Edge topology:
```
browser → edge nginx/1.24.0 (Ubuntu wildcard TLS ingress)
        → coderd/wsproxy v2.34.2 (Go reverse proxy, auto-flushes SSE)
        → agent tunnel → ziee-web inner nginx/1.22.1 (:8080) → ziee server (:9000)
```

`GET /api/sync/subscribe` (admin bearer), two paths:

| Path | Observation |
|---|---|
| Direct `localhost:8080` | `event: connected` **immediately**; `:` keepalive at **+15s**. Streams. |
| Coder URL | **0 bytes in 40s** (full buffering; not a drop). |
| Coder URL, plain `GET /api/auth/me` | Instant, 1689 bytes. |

`code_sandbox` install/subscribe (already sets `X-Accel-Buffering: no`):
- Direct: streams `connected` immediately, **but response has no
  `X-Accel-Buffering` header** → inner nginx ate it.
- Coder URL: still **0 bytes / 12s** → edge never saw the header.

Why the edge (not coderd) is the buffer: coderd is Go `httputil.ReverseProxy`
(auto-flushes `text/event-stream`), and Coder's own web-terminal/code-server
stream fine through this same edge. The generic wildcard nginx ingress has
`proxy_buffering` on with no SSE tuning.

---

## Live edge-honoring experiment (gates A1 vs operator-side)

Reversible test on live `ziee-web-ziee-web-1` (`docker cp` patched nginx.conf →
graceful `nginx -s reload` → re-run Coder SSE curl → restore + reload):

**RESULT: A1 CONFIRMED.** With the inner nginx re-emitting `add_header
X-Accel-Buffering no always;`, `GET /api/sync/subscribe` through the **Coder
URL** delivered `event: connected` **immediately** and the `:` keepalive at
exactly **+15s** — full incremental streaming (vs 0 bytes/40s before). The edge
nginx *consumed* the header (it's absent from the Coder response), proving it
acted on it and disabled buffering. Live container restored to the repo original
afterwards (verified `diff`-identical, `nginx -s reload`). → ship A1 in the repo;
no operator-side (A3) change needed.

---

## Out of scope (per user)
SSH-tunnel lag symptom — no SSH client-config or symptom-B work.
