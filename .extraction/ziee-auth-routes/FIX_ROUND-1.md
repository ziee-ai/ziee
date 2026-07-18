# Chunk ziee-auth-routes — FIX_ROUND-1 (blind multi-angle audit → fix)

Blind audit ran the 12 angles in `LEDGER.jsonl` over the full diff
(`git diff HEAD -- src-app` ziee-side + `git diff --cached` sdk-side), with
whole-diff hunk coverage in `AUDIT_COVERAGE.tsv` (≥3 angles/hunk).

## Candidate findings triaged
- **[considered] ProfileEdit split across crates** — moving `ProfileRead`/`ProfileEdit`
  to `ziee-auth` while `UsersRead`/`GroupsRead` stay app-side. Verdict: NOT a defect.
  The self-profile keys gate the moved auth surface; the user-admin keys gate the
  user-admin handlers that (per BA-full C3) stayed app-side. `all_permissions()`
  collects both via shim. Const strings byte-identical → OpenAPI unchanged.
- **[considered] `#[debug_handler]` removed from 10 handlers** — could it change
  behaviour? Verdict: no. `debug_handler` is a compile-time diagnostic macro that
  emits no runtime code and no OpenAPI; removal is required for generic fns and is
  invisible to the wire contract. The remaining 8 non-generic handlers keep it.
- **[considered] `token_response` visibility widened `pub(crate)`→`pub`** — a
  larger public surface. Verdict: acceptable + necessary (the ziee app shim
  re-exports it for `app::handlers`). No behaviour change; it returns the same
  cookie-mode-shaped response.
- **[considered] resolver `R` unconstrained-`Send/Sync`** — would the generic
  builders fail to satisfy axum `Handler`? Verdict: no — `IdentityResolver` already
  bounds `Send + Sync` and the framework `RequirePermissions<R,_>` impl compiled;
  `cargo check` green on all four crates confirms the bounds resolve.

## Result
New confirmed findings: 0
