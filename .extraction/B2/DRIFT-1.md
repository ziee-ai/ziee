# Chunk B2 — DRIFT round 1

Reconciliation of the moved code against `CUT.md`/`TRANSFORMS.md` and the
equivalence tripwires.

- **DRIFT-1.1** — Every moved dest resolves in the SDK: `module_api.rs`,
  `app_builder.rs`, `events.rs` under `sdk/crates/ziee-framework/src/`; the fleshed
  `ServerConfig` + all 10 relocated sub-types under `sdk/crates/ziee-core/src/
  config.rs`. Every `## Symbols` entry (`AppModule`, `ModuleContext`, `ModuleEntry`,
  `MODULE_ENTRIES`, `create_modules`, `initialize_modules`, `build_api_router`,
  `create_cors_layer`, `apply_rate_limit_layer`, `ServerConfig`, `HttpServerConfig`,
  `PostgreSqlConfig`, …, `JwtConfig`, `LoggingConfig`, `EventHandler`) is present +
  re-exported at its crate root. — verdict: none

- **DRIFT-1.2** — Every changed/new symbol is declared in `TRANSFORMS.md`: T-1
  (`ServerConfig` flesh-out), T-2 (`HttpServerConfig` rename), T-3 (`Config`
  compose+Deref), T-4 (cors/rate-limit param type), T-5 (`ModuleContext` fields),
  T-6 (11 domain-config `init` reads), T-7 (`EventHandler` erasure), T-8 (`EventBus`
  + 3 handlers), T-8b (desktop parallels), T-9 (shims). No undeclared non-byte-
  identical change remains. — verdict: none

- **DRIFT-1.3** — No stale ziee reference points at the old locations: `module_api/
  mod.rs` re-exports the 4 module-system symbols (consumed by all 44
  `#[distributed_slice(MODULE_ENTRIES)]` sites + `lib.rs`/`main.rs`/`openapi/mod.rs`
  — all resolve); `core/app_builder.rs` re-exports the 5 fns + retains
  `register_event_handlers`; `core/config.rs` re-exports the 10 config sub-types +
  composes `Config`; `core/events.rs` re-exports `EventHandler` + retains
  `AppEvent`/`EventBus`. `cargo check -p ziee` (lib + bin) AND `cargo check -p
  ziee-desktop` are green (exit 0). — verdict: resolved

- **DRIFT-1.4** — **Equivalence tripwire: `types.ts` BYTE-IDENTICAL, `openapi.json`
  CANONICALLY-EQUAL.** After `--generate-openapi` for the ui binary,
  `src-app/ui/src/api-client/types.ts` is **byte-identical** to
  `.extraction/baseline/types.ui.ts`, and `jq -S`-canonicalized `openapi.json`
  equals the canonicalized baseline (`openapi.ui.json`) — same paths/schemas, 236
  lines of JSON key-order churn only (the linkme route-registration order is a
  deterministic function of the dependency graph; the new `ziee-framework` path-dep
  perturbs order but adds/removes/renames nothing). Per the E8 REFINEMENT this is
  the pass condition. No config type and no event type appears in either output
  (they are `Deserialize`-only / off-wire), so the Config split + event erasure are
  provably schema-neutral. — verdict: none

- **DRIFT-1.5** — **N2 shim vs. E6 file-absence.** `CUT.md`'s two whole-file `move:`
  sources (`module_api/backend_module.rs`, `module_api/types.rs`) are DELETED from
  ziee (`git rm`); the three symbol-level sources (`core/app_builder.rs`,
  `core/config.rs`, `core/events.rs`) are RETAINED as shims with the moved
  definitions deleted. No divergent duplicate definition remains — `AppModule`/
  `ModuleContext`/`ModuleEntry`/`MODULE_ENTRIES`/the 5 app_builder fns exist ONLY in
  `ziee-framework`; the config sub-types + `ServerConfig` exist ONLY in `ziee-core`;
  `EventHandler` exists ONLY in `ziee-framework`. Single-source preserved. — verdict:
  resolved

- **DRIFT-1.6** — **Cross-crate linkme registration proven.** The `MODULE_ENTRIES`
  distributed_slice is DEFINED in `ziee-framework` and registered into from ziee's
  44 module sites via the `crate::module_api::MODULE_ENTRIES` re-export (no
  find/replace of the slice path was needed). Proof it links: the full-router regen
  (`build_api_router` → `create_modules` → `MODULE_ENTRIES`) emitted every module's
  routes, yielding a canonically-equal `openapi.json` — an empty slice would have
  produced a near-empty spec. The retained `create_modules_instantiates_all_entries_
  in_order` test additionally asserts every registered entry is present. — verdict:
  none

- **DRIFT-1.7** — **No domain leakage into `ziee-framework`.** The framework
  `ModuleContext` carries `Arc<ServerConfig>` + an opaque `Arc<dyn Any>`; `AppModule`
  + the app_builder fns name no ziee domain type; `EventHandler` is event-erased.
  `AppEvent`/`EventBus`/`register_event_handlers` (domain-coupled) stayed app-side.
  The Config-split design gate (framework context free of the app's `Config`) is
  satisfied. — verdict: none

**Unresolved drifts:** 0
