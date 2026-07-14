# Chunk sdk-devconfig — BOUNDARY (green evidence)

Honest self-reported evidence. This chunk is ADDITIVE (new `@ziee/config`) + a partial
MOVE (7 ziee-ui lints relocated into the SDK; ziee now consumes them + the shared configs).
The equivalence anchor is **backward-compat** + **no codegen impact**.

- E1: PASS — one `.extraction/sdk-devconfig/` dir; appended to `.extraction/ORDER`.
- E2: PASS — the diff is exactly: the new `@ziee/config` package (sdk submodule); ziee
  repointing 9 scripts + deleting the 7 moved files; ziee biome/tsconfig/syncpack consuming
  the base; the `npm install` lockfile bump. No unrelated modifications.
- E3: PASS — no `#[ignore]`/`.skip`/`.only`; the smoke asserts real behavior.
- E4: PASS — no cosmetic test; the smoke drives a real lint against a real fixture dir
  (clean passes / violation fails) + the backward-compat proofs run the real SDK entrypoints
  against ziee's real tree.
- E5: PASS — every CUT.md new file exists; every declared export/bin resolves
  (`node_modules/.bin/ziee-*` symlinks present after install; `import('@ziee/config/syncpack')`
  works in the smoke).
- E6: PASS (partial-move scope) — the 7 moved ziee-ui scripts are ABSENT from ziee
  (`git rm`); ziee's `package.json` invokes the SDK copies instead. (desktop/ui keeps its
  own separate copies — out of scope; a future dedupe.)
- E7: PASS — every non-trivial change declared in TRANSFORMS (T-PARAM-1..4, T-BIOME-1,
  T-TSCONFIG-1, T-SYNCPACK-1, T-CHECK-1) + the guardrail-triage table, each with a Decision.
- E8: PASS — **NO types.ts/openapi impact.** Neither the sdk submodule nor the ziee main-repo
  diff touches Rust / `emit_ts` / `openapi.json` / `api-client/types.ts` / `migrations`.
  golden(types) untouched by construction (STOP not triggered).
- E9: PASS — `@ziee/config` resolves + its smoke `node --test` passes 4/4 (pure JSON+mjs
  package; no build step).
- E10: PASS — backward-compat GREEN: ui tsc 0, desktop tsc 0, all 8 config-subset `check`
  steps pass byte-identically (kit-manifest improved broken→pass); `ziee-check` runs 9 steps
  green against ziee.
- E11: PASS — parameterized over the app src dir (proven: SDK lints reproduce ziee's results
  via `--root`/`--css`/`--barrel`; smoke proves an arbitrary `--root`).
- E12: PASS — `npm install` at the worktree root clean; `@ziee/config` committed in the sdk
  submodule (sha f65e9df); ziee-side changes staged (NOT `vendor/pgvector`), not pushed.

ziee-suite: PASS (config-layer backward-compat scope) — ui + desktop tsc clean; every
config-layer check step byte-identical (or fixed). No backend/integration surface touched.
gate:ui (ui): N/A-reframed — no ziee UI *surface* changed; the anchor is the config-subset
byte-identity above (gallery/visual gate is the separate @ziee/gallery layer's concern and
was pre-existing-red per project memory, untouched here).
golden(types): UNTOUCHED (no codegen in diff)
golden(openapi): UNTOUCHED (no codegen in diff)
golden(schema): UNTOUCHED (no migrations in diff)

Backward-compat is GREEN; no codegen impact; the shared config + parameterized lints ship
with functional proofs. Self-reported PASS (orchestrator re-verifies ziee check + merges).
