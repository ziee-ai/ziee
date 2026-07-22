# BASE — conflict-surface scoping

Branch `fix/inline-math` off `origin/khoi` @ `68af34059` (merge of #187; already contains
#188's display-math fix, which this branch extends).

## Migrations

**None added.** Server migrations are module-owned (`src-app/server/src/modules/*/migrations/`,
merged at build time by `build.rs::compose_merged_migrations`, plus
`src-app/desktop/tauri/migrations/` whose highest is `10000000000005_create_host_mounts.sql`).
This branch adds no migration, so a migration-number collision with main is structurally
impossible.

## OpenAPI regen

**Not implied.** No Rust type, route, or response shape changes — the diff is confined to
`src-app/ui/src/components/common/**` and one e2e spec. Neither `openapi.json` nor
`api-client/types.ts` is touched in either workspace.

## Files this branch touches that main may also be changing

| File | Collision risk |
|---|---|
| `src-app/ui/src/components/common/normalizeMathDelimiters.ts` | **Highest.** Created by #188 (merged 2f1bf284c), so it is young and the likeliest thing another branch also edits. The display pass is left byte-identical here, which confines any conflict to the new inline block + the header comment. |
| `src-app/ui/src/components/common/normalizeMathDelimiters.test.ts` | Same origin (#188). TEST-2 is rewritten wholesale — a concurrent edit to that block would conflict; the other tests are untouched. |
| `src-app/ui/src/components/common/markdownPreprocess.ts` | Doc-comment lines only (`:66-70`). Low risk. |
| `src-app/ui/src/components/common/markdownPreprocess.test.ts` | Additive (new case appended). Low risk. |
| `src-app/ui/tests/e2e/chat/markdown-rendering.spec.ts` | One spec rewritten, two edited/added. Shared chat e2e file — moderate risk if another branch adds specs to it. |

No backend, no desktop `ui/` override, no shared test harness (`tests/common/*`, gallery
cassette, playwright configs) is touched — B3 is not in play.

## Desktop counterpart (R2-3)

`src-app/desktop/ui/` carries hand-written overrides of `src-app/ui/`. Verified at plan
time: `src-app/desktop/ui/src/components/common/` has no override of
`normalizeMathDelimiters.ts` or `markdownPreprocess.ts` — to be re-confirmed during
implementation, and if an override exists it must receive the identical change.
