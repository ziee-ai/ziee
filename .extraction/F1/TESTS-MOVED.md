# Chunk F1 — TESTS-MOVED

F1 is a behavior-preserving MOVE of a UI component library; there is no new
behavior to prove. Coverage-preservation:

## Moved INTO the SDK (`sdk/packages/kit/src/`)

| Test | Origin | Note |
|---|---|---|
| `kit/table-view-core.test.ts` | ziee `components/ui/kit/table-view-core.test.ts` | The kit's one in-tree unit test (a `node:test` suite over the pure `table-view-core` selection/sort logic). Moved byte-for-byte; the kit tsconfig gained `allowImportingTsExtensions` (it imports `./table-view-core.ts` with an extension) + node types so it type-checks in the kit. Runs under a Node test runner, not tsc; type-checked green by the kit's `tsc --noEmit`. |

## The equivalence gate for this chunk (in place of new tests)

Since this is a wire-irrelevant frontend MOVE, the E8 openapi/types golden does
NOT apply. The enforced equivalence checks are:

| Check | Result |
|---|---|
| Per-file byte-diff of all 117 moved component files vs `HEAD` | 0 non-import differing lines (22 identical, 95 import-only) |
| `cn` / `use-mobile` / `DivScrollX` vendored copies vs app originals | `diff`-identical |
| `tokens.css` vs `index.css` 47–205 | byte-identical |
| No `@/` import remains in `sdk/packages/kit/src` | grep: none |
| No `@/components/ui` import specifier remains in ziee | grep: none |
| `@ziee/kit` standalone `tsc --noEmit` | **exit 0** |
| ziee `ui/` `tsc --noEmit` | **exit 0** |
| ziee `desktop/ui/` `tsc --noEmit` | **exit 0** |

## Stayed in ziee (unchanged)

`ui/tests/e2e/testid.ts` — retained; its `import type { TestIdLike }` was
repointed from the moved file's old relative path to `@ziee/kit/testIds.generated`
(a type-only import; no behavior change). All app/gallery/E2E specs that import UI
components keep their assertions unchanged — only the import prefix moved
(`@/components/ui` → `@ziee/kit`), which is exactly the coverage-preservation
posture (import-path edits OK; no behavioral assertion edited).

## Coverage-shrink guard

No covering test was dropped or renumbered. The kit's single unit test moved with
the kit; every consumer test still compiles against the identical component
surface via `@ziee/kit`.
