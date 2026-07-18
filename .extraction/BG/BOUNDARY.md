# Chunk BG — BOUNDARY (exit checklist)

## Gate results

| Gate | Result |
|---|---|
| `cargo check -p ziee` (lib + `ziee` bin) | **exit 0** — only pre-existing dead-code warnings (KB / notifications / llm_repository), zero new |
| `cargo check -p ziee-desktop` | **exit 0** |
| `cargo check --test integration_tests` (server) | **exit 0** — whole test target compiles against the new seams |
| E8 golden — `types.ui.ts` | **BYTE-IDENTICAL** vs `.extraction/baseline` |
| E8 golden — `types.desktop.ts` | **BYTE-IDENTICAL** |
| E8 golden — `openapi.ui.json` | **CANONICALLY-EQUAL** (`jq -S`) |
| E8 golden — `openapi.desktop.json` | **CANONICALLY-EQUAL** |
| Regenerated openapi/types | restored via `git checkout` |
| Gate grep (`modules/auth` + `modules/user`) | **EMPTY** (exit 1) — no reference to any of the 6 globals |
| Scope guard (≤ ~40 files) | **26 files** (24 modified + 2 new); no seam blew the guard |

## BA-full unblocked — proof

```
$ grep -rn 'crate::core::Repos\|modules::sync::publish\|secrets::storage_key\
  \|core::AppEvent\|EventBus\|url_validator\|core::config::JwtConfig' \
  src-app/server/src/modules/auth src-app/server/src/modules/user
$ echo $?
1        # no matches
```

The auth module (`modules/auth/*`) and the co-located user repos/handlers
(`modules/user/*`) no longer NAME any of the six app-globals the Chunk-BA
STOP_REPORT listed as blocking. They depend only on the injected
`AuthContext`/`AuthEventSink`/`AuthSyncSink` seams (consumer-owned) + threaded
`&PgPool`/`Option<&str>` params. The six globals are now named ONLY in app-side
wiring OUTSIDE the auth/user tree: `core/events.rs` (the installed sink impls +
`build_auth_context`), `core/config.rs` (the `From<JwtConfig>` bridge),
`core/outbound.rs` (the `url_validator` re-export), and `lib.rs`/`main.rs` (the
`Extension<AuthContext>` layer). Chunk BA-full can now move the auth
repos/handlers + `query!` macros into `ziee-auth` behind the auth-only build DB.

## Deferred / reported (not blocking)

- **Seam 5 (`url_validator`)** is re-homed behind `core::outbound`, NOT
  trait-injected — the deliberate "your call" choice (Decision 5): it is
  domain-free framework infra whose true home is `ziee-framework` (impossible to
  move in this in-ziee chunk), and `oauth2.rs`'s process-global-client +
  fn-pointer plumbing makes trait-injection a risky non-equivalence-trivial
  restructure. When `url_validator` lands in `ziee-framework`, `ziee-auth`
  retargets the single `core::outbound` import.
- **Sync sink** still references the app's `SyncEntity`/`Audience` value types
  (not gate-flagged; made app-extensible only in Chunk B5). BG removed the direct
  global `publish()` FUNCTION call, which is its charter.

## Not committed
Per the task, this chunk is NOT committed and NOT pushed; the submodule is
untouched. The orchestrator commits the ziee side. Changed files listed in CUT.md.
