# FIX_ROUND-2 — office-bridge (consolidated)

The round-1 re-audit of the fixes caught a real defect in the fix itself:

- **stage_cert_der leak + restart-collision (LOW, error-handling)** — the `StagedCert`
  RAII guard was constructed AFTER `create_dir` but before the cert open/write, so an
  I/O failure orphaned the freshly-created temp dir; and the deterministic `pid+counter`
  name could `EEXIST`-fail (a fail-safe DoS on cert install) after a crash+restart with a
  reused pid. Fixed: build the guard IMMEDIATELY after `create_dir` (cleanup now covers
  every error path) and add a nanosecond nonce to the dir name.

A full blind re-audit of the fixed `stage_cert_der` (temp-dir/file leak on every error
path, TOCTOU/symlink, name-collision after crash+restart, Drop timing vs the privileged
reader, resource/concurrency correctness) returned **zero** findings — the fix is clean.

**New confirmed findings:** 0
