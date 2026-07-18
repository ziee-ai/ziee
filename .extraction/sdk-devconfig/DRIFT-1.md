# Chunk sdk-devconfig — DRIFT round 1

Reconciliation of the implemented diff against CUT.md/TRANSFORMS.md and the
backward-compat tripwires (the equivalence anchor).

- **DRIFT-1.1** — Every CUT.md file exists. SDK: `biome.base.json`, `tsconfig.base.json`,
  `syncpack.base.mjs`, `package.json`, `README.md`, `src/check.mjs`, `src/lint/{roots,
  hardcoded-colors,settings-field,adjacent-inline,logical-direction,tooltip-placement,
  design-spec,kit-manifest}.mjs`, `scripts/config.test.mjs`. ziee: 7 script deletions +
  `package.json`/`biome.json`/`tsconfig.json` edits + `.syncpackrc.json`→`.mjs`. — verdict: none

- **DRIFT-1.2** — Every non-trivial change declared in TRANSFORMS (T-PARAM-1..4,
  T-BIOME-1, T-TSCONFIG-1, T-SYNCPACK-1, T-CHECK-1) + the guardrail-triage table, each
  with a Decision. No undeclared surface. — verdict: none

- **DRIFT-1.3** — **Backward-compat tripwire GREEN.** ziee `ui/` `tsc --noEmit` exit 0,
  `desktop/ui/` `tsc --noEmit` exit 0 (both == baseline). All 8 config-subset `check`
  steps PASS via `npm run`, each byte-identical to baseline stdout (the 5 lints + design-spec
  diffed clean; guardrails identical modulo run-ms). `check:kit-manifest` fixed
  (broken→pass). — verdict: none

- **DRIFT-1.4** — **No codegen / types.ts impact.** `git status` of the sdk submodule +
  ziee main repo contains zero Rust / `emit_ts.rs` / `openapi.json` / `api-client/types.ts` /
  `migrations` paths. golden(types)/golden(openapi) untouched (STOP not triggered). — verdict: none

- **DRIFT-1.5** — **Parameterization proven.** The SDK lints, invoked against ziee's src
  via `--root`/`--css`/`--barrel`, reproduce the pre-extraction results exactly; the smoke
  test independently proves an arbitrary `--root` dir is scanned (clean pass / violation
  fail). — verdict: resolved

- **DRIFT-1.6** — **Config `extends` verified, not assumed.** biome extends → `biome check`
  identical counts + `lint:guardrails` identical; tsconfig extends → `tsc` exit 0 clean;
  syncpack `.mjs` → `syncpack lint` byte-identical. Each was run before/after and diffed. — verdict: resolved

- **DRIFT-1.7** — **Scope discipline.** Gallery/visual checks + `lint-icon-action` +
  `lint-native-scroll` untouched; no files under a gallery path in the diff. The chunk
  stays within the config/design-token layer. — verdict: resolved

**Unresolved drifts:** 0
