# Chunk sdk-fe-batteries — FIX round 1

Findings from the blind multi-angle audit (`LEDGER.jsonl`), triaged.

- Two issues were caught + fixed **during** the build loop (before this round),
  recorded as the reasons for their transforms:
  - The FE-2 smoke failed to import because the framework's TS sources use
    extensionless relative imports and Node ESM needs the extension → added the
    minimal `scripts/ts-resolve*.mjs` resolver (T-INFRA-1). Smokes then pass.
  - The FE-3 proof initially asserted the compiled CSS contains `--color-primary`,
    but `@theme inline` INLINES that variable (it is not emitted as a standalone
    declaration) → corrected the assertion to `.bg-primary` + `--primary` (the raw
    token), which is the semantically correct proof. Proof then PASS.

- The `security` angle (FE-2 is an auth-token boundary) confirmed **no leak
  introduced**: the injected provider only supplies a bearer token to the
  existing `Authorization: Bearer` header path; the default localStorage source is
  unchanged; no token is logged or persisted by the new code. The foot-gun the fix
  CLOSES (silent-unauthenticated for non-`auth-storage` apps) is the security
  improvement. No fix needed.

- All other ledger entries are `status: ok` (verified backward-compatible /
  correct), requiring no fix.

**New confirmed defects introduced by this chunk:** 0
