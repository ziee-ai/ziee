# MIGRATION GUIDE — moving central gallery entries into per-module gallery.tsx

You are migrating dev-gallery seed entries from the (pre-refactor) CENTRAL files
into per-module `src/modules/<X>/gallery.tsx` files. The mechanism + gate already
exist; you ONLY create/fill module `gallery.tsx` files. Work is in the worktree
`/data/pbya/ziee/tmp/showcase-seed-wt`, all paths under `src-app/ui/`.

## Where the ORIGINAL entries live (pre-refactor, in git)
The central files were flipped to thin aggregators. The ORIGINAL content (with all
entries + local helpers) is at git commit **`d31243b36`**. Read it with:
```
git show d31243b36:src-app/ui/src/dev/gallery/overlays.tsx
git show d31243b36:src-app/ui/src/dev/gallery/deepStates.tsx
git show d31243b36:src-app/ui/src/dev/gallery/seededSurfaces.tsx
git show d31243b36:src-app/ui/src/dev/gallery/seeded/shard1.tsx   # .. shard5.tsx
```
The shared FIXTURE DATA files are UNCHANGED in the working tree at
`src-app/ui/src/dev/gallery/fixtures/*.ts` — read them directly.

## The contract
Each module file: `src/modules/<X>/gallery.tsx`
```ts
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyNamed /* , lazyBound, lazyProps, lazyCompose, holdPatch, holdForever, whenTrue */ } from '@/dev/gallery/support'
// + module components/stores (@/modules/...), fixture data (@/dev/gallery/fixtures/...), Stores, dialog, etc.

export const gallery: ModuleGallery = {
  cassette: <moduleCassette>,   // ONLY if this module has one (see list below)
  overlays: [ /* OverlayEntry objects, verbatim */ ],
  deepStates: [ /* DeepStateEntry objects (chat only) */ ],
  seeded: [ /* SeededSurfaceEntry objects, verbatim */ ],
}
```
The worked reference is **`src/modules/user/gallery.tsx`** (already done) — copy its shape.

## Import-rewrite rules (the ONLY transformation; entries are otherwise VERBATIM)
1. **Preserve every `slug` byte-for-byte.** Slugs are the coverage key — a renamed
   or dropped slug is a regression. Also preserve `surface`, `title`, `note`,
   `path`, `initialPath`, `fullHeight`, `component`, `open`/`setup`, `interactions`.
2. Helpers `lazyNamed`/`lazyBound`/`lazyProps`/`lazyCompose`/`holdPatch`/`holdForever`/`whenTrue`
   → import from `@/dev/gallery/support` (NOT `./seeded/helpers`, NOT a local def).
3. Fixture data (e.g. `adminUser`, `llmProvidersList`, `llmGroupsList`, `skillsList`,
   `deepProject*`, chat-deep exports, `SKILLS_CONVERSATION_ID`, …): the original
   imported them from `./fixtures/<file>` — rewrite to `@/dev/gallery/fixtures/<file>`.
4. `Stores` → `@/core/stores`; `dialog` → `@/components/ui`. Module components/stores
   keep their `@/modules/...` / `@/components/...` paths unchanged.
5. **Local helper functions/consts** the original file defined and an entry's
   `setup`/`open` closure references (e.g. deepStates' `chat`, `tick`, `whenLoaded`;
   seededSurfaces' `seedProjectDetail`, `seedSkills`, `seedBinary`, `seedProjectFiles`,
   the `window.fetch`-shim wrappers, action-patch wrappers): MOVE them into the SAME
   module `gallery.tsx` (module-scoped consts/fns above the `gallery` export), VERBATIM.
   Copy every const/type an entry depends on. Miss nothing.
6. Gallery-local demo components (`DefectRepro`, `TableDemos`, `MessageListLongDemo`)
   live at `src/dev/gallery/*.tsx` — a module gallery importing them uses
   `@/dev/gallery/DefectRepro` etc.

## Module → cassette (set `cassette:` ONLY for these; import from the fixtures file)
- auth → `authCassette` (`@/dev/gallery/fixtures/auth`)
- chat → `chatCassette` (`@/dev/gallery/fixtures/chat`)
- citations → `citationsCassette` (`@/dev/gallery/fixtures/citations`)
- llm-provider → `llmProvidersCassette` (`@/dev/gallery/fixtures/llm-providers`)
- projects → `projectDeepCassette` (`@/dev/gallery/fixtures/project-deep`)
- workflow → `workflowCassette` (`@/dev/gallery/fixtures/workflow`)
All OTHER modules: NO `cassette` field (they render from the shared crawl).

## Verify before finishing
- Every slug you were assigned appears exactly once in your files.
- Every import you write resolves to a REAL export: `grep "export" <the file>` for
  each fixture/helper name you import. Do NOT invent names.
- Do NOT edit any central file (`overlays.tsx`, `deepStates.tsx`, `seededSurfaces.tsx`,
  `fixtures/index.ts`, `seeded/*`, `support/*`, `package.json`) — ONLY create/fill
  `src/modules/<X>/gallery.tsx` for YOUR assigned modules.
- Do NOT run tsc (slow/contended) — be precise; the orchestrator runs the full tsc.
