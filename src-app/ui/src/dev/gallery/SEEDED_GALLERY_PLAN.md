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

Tracked in `dev/gallery/COVERAGE.md` — components covered/total, stores
covered/total, pages covered/total, per workspace.
