# Seeded page/component gallery — plan

Extends the existing dev-only gallery (`src/dev/gallery/`) from isolated kit
**component stories** to a **fully-seeded** canvas that renders every PAGE, every
STORE's populated state, and every module COMPONENT with realistic data — so the
whole UI can be reviewed visually without a backend. Dev-only
(`import.meta.env.DEV`).

## Approach — MOCK-API (record/replay cassettes)

The gallery boots with `window.fetch` intercepted (`mockApi.ts`). Each store's
**real** `load()` path runs unchanged and populates from per-module SEED
FIXTURES, so pages/components render in realistic populated states — most
faithful, deterministic, no backend.

```
dev/gallery/
  fixtures/            recorded cassette JSON per module (+ typed index)
  mockApi.ts           window.fetch interceptor: METHOD+path -> cassette
  pages.tsx            page-entry registry (auto-enumerated from module routes)
  seed.ts              seed Auth(admin) + install mock; gallery bootstrap
```

## Fixture correctness — THREE layers (a wrong fixture must fail a gate)

1. **Typed** — every fixture is typed against the generated
   `src/api-client/types.ts` response types; the mock router is typed so each
   endpoint returns `ResponseByUrl<url>`. A wrong shape fails `tsc`.
2. **Recorded from a real server** (source of truth) — `scripts/record-gallery-fixtures.mjs`
   boots a server against a throwaway DB, runs first-run setup with a known
   admin, loads `server/seeds/showcase/showcase.sql` (extended as needed), hits
   each endpoint the gallery needs, and saves the ACTUAL JSON as the cassette.
   The gallery replays REAL responses → shapes correct by construction. Prefer
   recorded fixtures over hand-authored ones.
3. **Contract test** — `tests/fixtures/fixture-contract.test.ts` validates every
   cassette against `openapi/openapi.json` (ajv). A fixture that drifts from the
   spec fails CI (ties into the existing `types_ts_parity` discipline).

Net: a stale/wrong fixture fails either `tsc` or the contract test — never
renders silently against a wrong shape.

## Scope (both workspaces)

- **Pages** — auto-enumerate from the 32+ `module.tsx` route registrations so
  none are missed; render each in a `MemoryRouter` at its path with seeded
  stores. Key states per page: loaded / filled / drawer-with-data / empty / error.
- **ALL stores** — every Zustand store (web + desktop) gets a fixture and a
  gallery entry showing populated/empty/error.
- **ALL components** — every web module component (~209) + kit/ui primitives +
  desktop components, each rendered with realistic seeded props/store data +
  key variants/states. Coverage checklist (covered/total for components AND
  stores) is a deliverable.
- **Desktop UI** (`src-app/desktop/ui`) — its own Vite app with its own
  modules/stores/components (incl. desktop-only: auto-login, updater). Stand up
  the same seeded mock-API gallery there (its own `dev/gallery` mirroring web,
  or a shared harness if imports allow), with its OWN `types.ts` +
  `openapi.json` for the 3 correctness layers.

## Sequencing

1. **VERTICAL SLICE (this checkpoint)** — llm-providers settings page (WEB),
   fully populated via mock-API, fixture recorded-from-server + typed +
   contract-validated. Verify render + tsc + push, then STOP for sign-off.
2. After go: fan out to all web modules (pages + stores + components), then
   replicate for desktop/ui, then the coverage checklist + visual matrix.

## Coverage checklist

Tracked in `dev/gallery/COVERAGE.md` — the human-readable rollup (components
covered/total, stores covered/total, pages covered/total, per workspace).

## Multi-state model — states per surface, not one entry per surface

A surface owns MULTIPLE named states, not a single entry. The registry is
`Record<GallerySurface, { kind, states }>`; a screenshot id is
`surface__state__theme`. States are produced intrinsically:

1. **Data states via different cassettes** (the key one — most bugs live in
   empty/error). The mock has a global `MockMode` (`mockApi.setMockMode`):
   `loaded` (recorded), `empty` (arrays deep-emptied + counts zeroed), `error`
   (500 for data endpoints; auth/bootstrap exempt so the page still mounts and
   shows its OWN error UI), `delayed` (latency → loading state). The gallery is
   URL-driven per combo: `?surface=<slug>&state=<mode>&auth=<seed>&theme=`.
2. **Permission/role states** via auth seeds (`seed.ts` `AuthSeed`):
   `admin` / `limited` (non-admin, minimal read perms) / `none` (logged out).
3. **Open states** for overlays — render the drawer/dialog/menu open (tracked as
   `pending` until the per-overlay opener lands).
4. **Variant/disabled** explicit instances (kit stories).
5. **Hover/focus** via Playwright pseudo-state forcing at screenshot time.

## Required-state-set gate (state coverage enforcement — ties to F14)

The gate evolved from "surface has ≥1 entry" to a REQUIRED STATE SET per kind:

| kind | required states |
|---|---|
| `data-page` / `table` | loaded + empty + error |
| `form` | empty + filled + invalid |
| `overlay` | open |
| `static` / `flow` / `via` / `nonvisual` / `pending` | none (escape hatches) |

A surface whose declared `states` miss its kind's required set FAILS
`check:gallery-coverage` (verified: dropping a data-page's `empty`/`error` fails).
Rendering the empty/error states surfaces real bugs → logged in
`GALLERY_FINDINGS.md`.

## Singleton stores that swap on route param — isolation

A per-entry `MemoryRouter` isolates the router, NOT global Zustand singletons.
Route-param detail stores that are **single-active-and-swap** (`Chat`'s
`conversation`/`messages`; `ProjectDetail`'s `project`) would BLEED if multiple
of their entries were mounted on one canvas (all show the last-seeded id).
Id-keyed stores (`WorkflowRuns` = `Record<workflowId,…>`) are safe.

**Policy:** swap-type detail surfaces are rendered ONLY via the URL-isolation
path (`?surface=&state=&conversationId=…`), one per FULL PAGE RELOAD → fresh
singleton → zero bleed. They are never all-mounted on the browse canvas (they
carry a required param the enumerator leaves unresolved, so they're skipped
there by construction). Multiple conversation states (empty / long / tool-calls /
branched / attachments) are separate combos, each its own `conversationId` +
cassette, rendered sequentially.

**Swap-TRANSITION correctness** — that navigating A→B clears A's stale
`conversation`/`messages`/`project` — is a runtime interaction property. It is an
**e2e/interaction test to add** (`tests/e2e/…/chat-nav-no-stale-state.spec.ts`),
NOT a static screenshot; the gallery does not attempt to capture it.

## Coverage is an ENFORCED compile-time gate (not just COVERAGE.md)

Modeled on `testIds.generated.ts`. Adding a page/component without a gallery
entry (or a reviewed allow-list reason) FAILS `tsc` + `npm run check`.

1. **Generator** — `npm run gen:gallery-coverage` walks `modules/**/*.tsx` +
   `components/ui/**/*.tsx` and emits `galleryCoverage.generated.ts`: the
   `GallerySurface` union (the DENOMINATOR — every page/component id) +
   `GALLERY_SURFACES` list.
2. **Typed registry** — `coverage.ts` declares
   `GALLERY_COVERAGE: Record<GallerySurface, Coverage>`. Because it's a total
   `Record` over the generated union, a surface with NO entry is a **tsc error**
   (the "bake into tsc" gate).
3. **Allow-list** — genuinely non-visual surfaces (providers, context, pure
   logic, null-render listeners) map to a reviewed `note('gallery:none — <why>')`
   / `via('<page>')`, which satisfies the `Record` without a visual entry, so the
   gate isn't noisy.
4. **Parity test** — `npm run check:gallery-coverage` regenerates the union and
   fails if `galleryCoverage.generated.ts` is stale (mirrors `types_ts_parity`),
   and lists any surface still marked `pending`.
5. **Wired** — `gen:gallery-coverage` runs in the openapi/gen flow;
   `check:gallery-coverage` runs in `npm run check`. Same machinery in
   `src-app/desktop/ui` with its own generated union + `openapi.json`.
