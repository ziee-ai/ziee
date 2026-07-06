# Exhaustive-state mechanism — guaranteeing EVERY renderable state

The seeded gallery's state list came from agent judgment, so it missed states —
empty/error variants and, critically, the active-conversation deep states (file
attachments, elicitation prompts, right-panel viewers, streaming tool-calls).
This mechanism replaces judgment with three mechanical layers so **every**
renderable state is either rendered by a gallery entry or explicitly excused.

## Part 1 — static extraction → a tsc-ENFORCED state gate

`scripts/gen-state-matrix.mjs` (ts-morph) walks every `.tsx` under `src/modules`
+ `src/components/ui` and extracts, per surface:

- **conditional renders** + their governing condition (`if (c) return …`,
  `c ? <A> : <B>`, `c && <JSX>`), classified `loading` / `error` / `empty` /
  `branch` by the identifiers in the condition;
- **overlay open-triggers** (Dialog/Drawer/Sheet/Popover/Dropdown with a
  controlled `open`/`visible` prop);
- **panel + slot registrations** (`registerPanelRenderer('file'|'literature')`,
  `sidebarContent`, `settings*Pages`, …) — how the right-panel viewers become
  discoverable.

It emits:

- `stateMatrix.generated.ts` — the matrix + a generated **`RequiredState`**
  union: one `"<surface>:<state>"` member per named state
  (loading→`delayed`, error→`error`, empty→`empty`, overlay→`open`,
  panel→`panel-open`).
- `STATE_MATRIX.md` — the human-readable review artifact.

`stateCoverage.ts` then declares
`STATE_COVERAGE satisfies Record<RequiredState, StateCoverageEntry>`, exactly
mirroring how `galleryCoverage.generated.ts`'s `GallerySurface` gates
`coverage.ts`. So:

- a **newly-added conditional render** → stale generated union → `check:state-matrix`
  (the parity guard, wired into `npm run check`, like the openapi `types_ts_parity`
  test) fails → regen adds a `RequiredState` member → **`tsc` fails** on
  `stateCoverage.ts` until that state gets a gallery entry (`{ via }`) or an
  explicit allow-listed reason (`{ skip: true, reason }`).

`npm run gen:state-matrix` to regenerate; `--scaffold` appends missing keys
(pre-classified from `coverage.ts` kinds) for review.

## Part 2 — dynamic proof via branch coverage

`scripts/gallery-coverage.mjs` renders EVERY gallery combo (browse + each page in
empty/error/delayed + every overlay + every chat deep-state) under istanbul
instrumentation (`plugins/vite-plugin-gallery-coverage.js`, enabled only when
`GALLERY_COVERAGE=1`), merges the per-render `window.__coverage__`, and reports
every **uncovered conditional-render branch** — a fork no combo exercised = a
state that never rendered.

- Output: `UNCOVERED_STATES.md` (file:line + the condition text) — the work queue.
- Gate: every uncovered branch must be rendered by a new entry OR allow-listed in
  `coverage-allowlist.json` with a reason. `--gate` exits non-zero on residuals.
- This is the runtime complement to Part 1: Part 1 proves each NAMED state has an
  entry; Part 2 proves the entries actually EXERCISE the branch (and catches the
  generic `branch` conditionals the tsc gate doesn't name).

Instrumentation is opt-in, so the heavy Playwright pass is a separate CI-able
script — the fast `npm run check` keeps the Part-1 tsc parity gate.

```bash
GALLERY_COVERAGE=1 npm run dev -- --port 1466 --strictPort   # instrumented server
npm run gallery:coverage -- --url=http://localhost:1466/gallery.html
npm run gallery:coverage:gate                                # CI gate
```

## Part 3 — deep-state cassettes (chat, serverless)

Chat's live states are SSE-driven, not JSON endpoints, so:

- `mockApi.ts` **replays recorded SSE frames** — the `/api/chat/stream` request
  is answered with a real `text/event-stream` from an `SseFrame[]` cassette
  (`setSseCassette`), the exact wire shape `ChatStreamClient` parses.
- `fixtures/chat-deep.ts` holds typed synthetic conversation bundles (tool-call
  running / failed, message attachments) merged into the chat cassette, plus the
  transient-state seeds (streaming frames, a pending elicitation, right-panel
  payloads).
- `deepStates.tsx` renders the REAL `ConversationPage` per state and seeds the
  transient piece through the REAL Chat store path (`applyStreamFrame` for
  streaming, `displayInRightPanel` for the file/literature viewers,
  `addElicitationRequest` for the pending prompt). Driven one-per-page-load via
  `?surface=deep-chat-<name>`.

Deep states delivered: `streaming`, `tool-running`, `tool-failed`, `attachments`,
`elicitation`, `right-panel-file`, `right-panel-literature`, `branched`, `long`.
