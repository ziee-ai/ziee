# FIX_ROUND-1

## Fixes applied to the phase-6 ledger's confirmed findings
- **error-handling / i18n-copy (medium) — misleading FILE_TOO_LARGE message.** The
  message used integer division, rendering a 1.5 MiB file over a 1 MiB cap as
  "1 MiB exceeds 1 MiB". Now: `File size {:.1} MB exceeds the maximum upload size
  of {} MB` (float file size, integer cap), and unified on **MB** across backend +
  frontend. (`file/handlers/upload.rs`)
- **i18n-copy / patterns-conformance / state-management (medium/low) — decoupled
  UI label + MiB/MB mismatch.** `MAX_FILE_UPLOAD_LABEL` is now DERIVED from
  `MAX_FILE_UPLOAD_BYTES` (`${Math.round(bytes/(1024*1024))}MB`), so it can't drift,
  and both surfaces present the cap as MB. (`ui/modules/file/constants.ts`)
- **tests-quality (medium) — route body-limit layer unverified.** Added an ~18 MiB
  upload assertion (above the derived cap+16 MiB limit) so the per-route
  `DefaultBodyLimit` layer is exercised, distinguishing derived-from-cap from a
  hardcoded constant. (`tests/file/mod.rs`)

## Explicit rejections (ledger, not fixed — rationale)
- **security/perf (low) — no upper bound on the cap + full-RAM buffering.**
  Admin-gated config (not user-controllable); the 128 MiB DEFAULT actually LOWERS
  the worst-case route body limit (200 MB → 144 MB) vs before; streaming uploads
  are out of scope for this change. Acceptable by design.
- **i18n-copy (low) — config doc cites "80–200 MB+".** That prose describes
  real-world genomics FILE sizes (decimal MB, as users state them), not the MiB
  cap; not a defect.

## Re-audit (blind, on the fix diff `53220c01...HEAD`)
Two fresh blind reviewers examined only the fix diff:
- **(low, dismissed) `{:.1}` rounding edge.** A file a few BYTES over the cap can
  render as "1.0 MB exceeds 1 MB". Dismissed: strictly better than the prior
  integer truncation, the message still says "exceeds", and the realistic case
  (a genuinely-too-large file) reads clearly; a byte-precise size would be worse UX.
- **(medium, CONFIRMED → fixed) 413 assertion flakiness.** axum answers 413 from
  the Content-Length check before consuming the still-uploading 18 MiB body, so the
  client can observe a mid-stream connection reset instead of a clean 413, making
  `.expect("Request failed")` panic intermittently. FIXED: the assertion now
  accepts a clean 413 OR a non-timeout transport error (both prove the body-limit
  layer engaged), and still rejects a handler response (201 / 400 FILE_TOO_LARGE).

**New confirmed findings:** 1
