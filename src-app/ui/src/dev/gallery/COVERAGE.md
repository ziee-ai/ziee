# Seeded gallery — coverage checklist

Human-readable rollup. The ENFORCED gate is `coverage.ts`
(`GALLERY_COVERAGE satisfies Record<GallerySurface, Coverage>`) — a surface with
no entry fails `tsc`; `npm run check:gallery-coverage` guards the generated union
+ lists `pending`. See `SEEDED_GALLERY_PLAN.md`.

## Web UI (`src-app/ui`) — 407 surfaces (every `.tsx` under modules + components/ui)

| Kind | Count | Meaning |
|---|---:|---|
| `page` | 38 | Route element rendered as a seeded gallery page (real, on-screen). |
| `via` | 321 | Transitively rendered: kit/shadcn primitives (kit stories) or a module component within its module's page. |
| `allow` | 7 | Non-visual (context / provider / listener / types) — no visual entry needed. |
| `pending` | 41 | Interaction-only (drawer / dialog / modal / sheet / menu) — needs an open-state entry. The honest remaining work. |

- **Pages**: 37/40 enumerated routes render populated via mock-API with real
  recorded data (0 console errors). The 3 non-rendering are redirect routes
  (`/auth` when authenticated, `/settings` index) + one `user-groups`
  undefined-length race.
- **Stores** (~107): populated through the mock-API load path when their page /
  component renders. Store-only entries (for stores behind no page) tracked via
  the `pending` interaction-only set.
- **Fixtures**: recorded from a real server; typed vs `types.ts` (tsc); ajv-
  validated vs `openapi.json` (71 values, 0 fatal).

### Remaining work (the `pending` set)

The 41 `pending` surfaces are interaction-only overlays (drawers/dialogs/modals).
Each needs a gallery entry that renders it in its OPEN state with seeded data.
Run `npm run check:gallery-coverage` for the live list.

## Desktop UI (`src-app/desktop/ui`)

Pending — mirror the web harness (own `types.ts` + `openapi.json`), covering the
desktop-only surfaces (auto-login, updater, tunnel-auth, remote-access,
host-mount, window, file-dialog).

## Commands

```
npm run gen:gallery-coverage      # refresh the surface union (denominator)
npm run check:gallery-coverage    # parity guard + list pending
npm run gallery:record            # re-record fixtures from a real server
npm run gen:gallery-crawl         # regenerate the typed crawl cassette
npm run gallery:check-fixtures    # ajv contract test vs openapi.json
npm run gallery:screenshots       # light+dark per-page capture
```
