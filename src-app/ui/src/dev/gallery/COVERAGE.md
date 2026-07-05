# Seeded gallery — coverage checklist

Human-readable rollup. The ENFORCED gate is `coverage.ts`
(`GALLERY_COVERAGE satisfies Record<GallerySurface, Coverage>`) — a surface with
no entry fails `tsc`; `npm run check:gallery-coverage` guards the generated union
+ lists `pending`. See `SEEDED_GALLERY_PLAN.md`.

## Web UI (`src-app/ui`) — 407 surfaces (every `.tsx` under modules + components/ui)


| Kind | Count | Meaning |
|---|---:|---|
| `data-page` | 34 | Route page rendered in loaded + empty + error via cassette-swapping (required-state set). |
| `overlay` | 14 | Drawer/dialog rendered in its OPEN state with seeded data (delivered). |
| `via` | 321 | Kit/shadcn primitives (kit stories) or a module component within its module's page. |
| `static` | 27 | Overlays whose open-state needs live context (hub item / provider+model / file / prop-driven) — allow-listed, open-state verified by the e2e interaction suite. |
| `flow` | 4 | Auth/setup flow (no data grid). |
| `nonvisual` | 7 | Context / provider / listener / types — no visual entry. |
| `pending` | 0 | — 100% of surfaces accounted (delivered state set or reviewed allow-list). |

- **Pages**: 37/40 enumerated routes render populated via mock-API with real
  recorded data (0 console errors). The 3 non-rendering are redirect routes
  (`/auth` when authenticated, `/settings` index) + one `user-groups`
  undefined-length race.
- **Stores** (~107): populated through the mock-API load path when their page /
  component renders. Store-only entries (for stores behind no page) tracked via
  the `pending` interaction-only set.
- **Fixtures**: recorded from a real server; typed vs `types.ts` (tsc); ajv-
  validated vs `openapi.json` (71 values, 0 fatal).

### Multi-state states

### Multi-state delivery

- **Data pages** (35): loaded + empty + error via cassette-swapping (`gallery:states`).
- **Overlays** (14): rendered OPEN with seeded data; 5 form drawers additionally
  get filled + invalid via Playwright interaction (`gallery:forms`).
- **Chat**: the rich showcase conversation (47 messages: markdown, code, tool
  results, attachments, 3 branches) replays via the chat cassette, rendered in
  isolation (`?surface=chat-detail&conversationId=…`); empty state via empty mode.

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
