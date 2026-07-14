# Chunk sdk-devconfig — FIX round 1

Findings from the blind multi-angle audit (LEDGER.jsonl), triaged.

- Two issues were caught + fixed **during** the build loop (before this round):
  - The parameterized lints initially would have resolved roots relative to the SDK
    file (like the originals), which pointed at the SDK dir, not the app → replaced with
    `parseRoots()` (CWD-relative, `--root`). Verified byte-identical vs baseline.
  - The baseline capture initially used `npm run tsc` (no such script — `tsc` is a direct
    binary) and surfaced `check:kit-manifest` as failing. Investigation revealed the kit
    had moved to `@ziee/kit`, leaving ziee's kit-manifest script pointing at a dead barrel
    (pre-existing break, not caused by this chunk). Re-pointing at the SDK kit via the
    parameterized tool FIXED it (broken→pass) — recorded as the reason for T-PARAM-4.

- The `config-merge` angle (biome/tsconfig/syncpack `extends`) is the highest-risk surface;
  it was verified EMPIRICALLY (before/after diff of `biome check`, `tsc`, `syncpack lint`),
  not assumed. All three merges are result-identical. No fix needed.

- The `golden-impact` angle confirmed **no `types.ts`/openapi/Rust** in either diff — the
  STOP condition is not triggered. No fix needed.

- All other ledger entries are `status: ok` (verified backward-compatible / correct).

**New confirmed defects introduced by this chunk:** 0
