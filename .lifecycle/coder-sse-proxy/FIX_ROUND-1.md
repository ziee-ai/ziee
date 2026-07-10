# FIX_ROUND-1

## Fixes applied for phase-6 confirmed findings
- **Cache-Control duplicate / contract change** (correctness+api-contract+config-correctness, med) →
  **dropped** the `add_header Cache-Control no-cache always;` line entirely.
  Buffering is disabled solely by `X-Accel-Buffering`; the SSE handlers already
  emit `cache-control: no-cache` server-side. Re-verified live: the final config
  streams through Coder and the direct response shows a single `cache-control:
  no-cache` (no duplicate).
- **Guard regex false-passes `no-cache`** (tests-quality, low) → tightened to
  `X-Accel-Buffering\s+no(?=\s|;)` so the value must be exactly `no`. Verified:
  injecting `no-cache` now FAILS the guard (exit 1).
- **Brace-matcher ignores `#` comments** (correctness+patterns, low) → added
  `stripComments()` before brace-matching. Verified: a stray brace inside a
  comment in `location /api` no longer desyncs the matcher (guard still OK).
- **`readFileSync` throws ugly stack** (error-handling, low) → wrapped in
  try/catch emitting a friendly `FAIL` message; still fails closed (exit 1).
- **Guard wired into no build/CI step** (tests-quality, med) → added a Dockerfile
  `config-check` stage (`node:22`) that runs the guard; the runtime now
  `COPY --from=config-check … nginx.conf`, so the build graph depends on it and
  the guard runs on **every image build**. Verified by building
  `--target config-check` (guard prints OK in-image; a missing directive would
  exit non-zero and fail the build). nginx.conf shipped is byte-identical.
- **Comment overstates default buffering + `X-Accel-*` wildcard** (i18n-copy, low)
  → reworded to "holds frames in its proxy buffers until they fill or the stream
  ends" and to the specific `X-Accel-Buffering` header.

## Rejected / out-of-scope (not fixed, with rationale)
- **worker_connections 1024 SSE cap** (concurrency, med) — PRE-EXISTING, not
  introduced by this diff; relates to the out-of-scope SSH-lag symptom. Rejected.
- **X-Accel-Buffering on all `/api` incl. downloads / leaks to browser** (perf+
  correctness, low) — accepted by design: applying at the `location` level is the
  standard idiom, un-buffered streaming of downloads is desirable, and browsers
  ignore the header. No change.

## Re-audit round 1 (blind, on the fixed diff)
One new confirmable finding:
- **Comment misattributes `always`** (config-correctness/comment-accuracy, low) —
  nginx's default `add_header` already covers 200 responses, so SSE gets the
  header without `always`; `always` only extends coverage to non-2xx codes.
  **Fixed**: reworded the comment to state this accurately.

**New confirmed findings:** 1
