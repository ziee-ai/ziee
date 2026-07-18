# Chunk B5 — BOUNDARY (phase-8 exit)

## Scope delivered

Moved the app-agnostic realtime-sync core — per-user SSE connection **registry**
(caps / pruning / self-echo / Owner·Perm·Everyone routing), the **audience**
machinery (`Audience`/`PermRule` + typed constructors), and the `SyncOrigin`
**extractor** — into `ziee_framework::sync`, generic over `ziee_identity::Principal`
(the connection permission snapshot) and a new `SyncEntityKind` trait (the
entity seam). ziee keeps its concrete wire/schema types, the process-wide
singleton, the concrete `SyncConnPrincipal`, `publish`/`publish_session_to_users`,
and every emit site unchanged.

## Design gate — RESOLVED (schema-neutral)

`SyncEntity` is NOT genericized. The framework is generic over the minimal
`SyncEntityKind` trait (one method: `session_signal(user_id) -> Event`, the only
wire event the registry itself synthesizes); every other event enters pre-serialized
via `deliver(audience, Event, …)`. So `SyncEntity` keeps its enum + all 59 variants
+ `#[derive(JsonSchema)]` and merely `impl`s the trait. See TRANSFORMS `## Decision`.

## EventBus split — RESOLVED

B2's deferred `AppEvent`/`EventBus` STAY app-side (domain-coupled, schema-irrelevant,
unrelated to SSE sync). Only the genuinely-generic SSE core moved. Framework
`EventHandler` trait unchanged.

## Gates

1. **Clean build (E9/C1)** — all exit 0:
   - `cargo check -p ziee` (lib + bin)
   - `cargo check -p ziee-desktop`
   - `cd sdk && cargo check --workspace`
   - `cargo test -p ziee-framework --lib sync::` → **16 passed; 0 failed**
2. **Golden (E8/C3) — BOTH surfaces:**
   - `types.ts` ui: **BYTE-IDENTICAL** · desktop: **BYTE-IDENTICAL**
   - `openapi.json` ui: canonically-equal (`jq -S`; 612 order-only lines) ·
     desktop: canonically-equal (byte-identical)
   - Regenerated per the recipe, asserted, restored via `git checkout`.
3. **E7 (transforms declared):** every non-byte-identical symbol has a `T-N` +
   `why:`; `## Decision` blocks carry `**Resolution:**`; zero TBD/TODO/ASK.
4. **Blind audit:** LEDGER 15 findings / 9 angles (incl. `equivalence`);
   AUDIT_COVERAGE 10 hunks ≥3 angles each; FIX_ROUND-1 new confirmed findings: 0;
   DRIFT-1 unresolved drifts: 0.

## Ported tests enumerated + PASS

See TESTS-MOVED.md — 16 unit tests moved to `ziee-framework::sync` (14 routing +
2 constructor), all PASS; ziee retains the wire-format + `check_permission_union`
suites.

## Files changed

- **Submodule (`sdk/`, committed):** `crates/ziee-framework/src/sync/{mod,registry,
  audience,extractor}.rs` (new), `crates/ziee-framework/src/lib.rs`,
  `crates/ziee-framework/Cargo.toml`, `Cargo.lock`.
- **Outer (ziee, working tree):** `src-app/server/src/modules/sync/{event,registry,
  extractor,handlers}.rs`, `src-app/server/src/modules/permissions/{mod,types}.rs`.

## Boundary commit (submodule)

`feat(ziee-framework): sync core (registry/event/publish/Audience) generic over app
SyncEntityKind` — sdk sha recorded in the run report. Not pushed.
