# Chunk BG-2 — BOUNDARY (exit checklist)

## Gate results

| Gate | Result |
|---|---|
| `cd sdk && cargo check --workspace` (ziee-framework + all SDK crates) | **exit 0** — ziee-framework still **build-DB-free** (no build DB needed) |
| `cargo check -p ziee` (lib + `ziee` bin) | **exit 0** |
| `cargo check -p ziee-desktop` | **exit 0** |
| E8 golden — `types.ui.ts` | **BYTE-IDENTICAL** vs `.extraction/baseline` |
| E8 golden — `types.desktop.ts` | **BYTE-IDENTICAL** |
| E8 golden — `openapi.ui.json` | **CANONICALLY-EQUAL** (`jq -S`) |
| E8 golden — `openapi.desktop.json` | **CANONICALLY-EQUAL** |
| Regenerated openapi/types | restored via `git checkout` |
| DRIFT | Unresolved drifts: 0 |
| FIX round 1 | New confirmed findings: 0 |

## Build-DB-free proof (ziee-framework invariant preserved)

```
$ grep -nE 'query!|query_as!|query_scalar!' sdk/crates/ziee-framework/src/secret.rs
$ echo $?
1        # no compile-time query macros — secret.rs uses runtime sqlx::query_as
$ cd sdk && cargo check --workspace   # no DATABASE_URL / no build DB
   Finished ... exit 0
```

`ziee-framework` gains `url_validator` (pure reqwest/url/thiserror infra) +
`secret` (runtime-`query_as` crypto) + `secrets` (a `once_cell` global) — none
requires a build DB. The invariant "ziee-framework MUST stay build-DB-free" holds.

## BA-full unblocked — proof

The two shared-infra prerequisites the BA-full STOP_REPORT named (C1
`common::secret`, C2 `url_validator`) now live in `ziee-framework`. The auth
module's existing references resolve THROUGH the framework:

```
$ grep -rn 'crate::common::secret|utils::url_validator|core::outbound' \
    src-app/server/src/modules/auth src-app/server/src/modules/user
  auth/providers/repository.rs:11: use crate::common::secret::{encrypt_secret, resolve_optional_secret};
  auth/providers/{oauth2,apple}.rs: crate::core::outbound::{OutboundUrlPolicy,build_validated_client,validate_outbound_url}  (12 sites)
```

- `crate::common::secret` → shim → `ziee_framework::secret` (verified: `cargo
  check -p ziee` exit 0).
- `crate::core::outbound` → re-exports `ziee_framework::url_validator` (BG Decision
  5 completed — `core/outbound.rs` now sources from the framework crate).
- auth names NO `crate::utils::url_validator` directly (it goes through
  `core::outbound`), so that path is clean.

When BA-full moves `modules/auth/*` into `ziee-auth`, these become
`ziee_framework::secret` / `ziee_framework::url_validator` directly — no app-crate
dependency remains for either.

## Wire-safety note (why the golden is byte-identical)

`SecretView<T>` carries **no `JsonSchema` impl** (documented at the module top),
so it is absent from every OpenAPI schema; `url_validator` and `secrets` expose no
aide/schemars handler or DTO. The crate boundary therefore moves zero schemars
idents → both surfaces' `types.ts` are byte-identical and `openapi.json`
canonically-equal. Had any moved type entered the wire and drifted `types.ts`, the
task's STOP rule would have applied — it did not.

## Committed
Per the task, the SDK submodule side (the 3 new framework files + lib.rs +
Cargo.toml + Cargo.lock) is committed in `sdk/` only — NOT pushed. The ziee side
(3 shims + `core/outbound.rs` + `src-app/Cargo.lock`) is left uncommitted for the
orchestrator, matching the BG convention. No remote pushed; `/data/pbya/ziee/ziee`
untouched.
