# Gallery findings — real bugs surfaced by rendering every surface on seed data

Rendering every page/component against recorded seed fixtures surfaced defects
that unit tests miss (they only bite when a component actually renders a real,
possibly-edge response). This is the crash/robustness log — separate from
visual-taste issues. Status as of the seeded-gallery fan-out.

## Crashes fixed (defensive-coding bugs)

All five are the SAME class: a store assigns an API response field straight into
state that a component then reads via `.length` / `.map` / iteration, with no
guard that the field is actually an array. A malformed or empty response (`{}`,
missing field) makes the whole row/page throw. Fix: `Array.isArray(x) ? x : []`.

| # | Store | Field | Surfaced on | Prod-latent? |
|---|---|---|---|---|
| 1 | `llm-provider/widgets/LLMProviderGroupWidget.store` | `providers` | **crashed** the `/settings/user-groups` page (its `Group.getProviders` is a per-group path-param endpoint → returned an edge shape) | **yes** — `providers.length` is read unconditionally in the widget header |
| 2 | `mcp/widgets/GroupSystemMcpServersWidget.store` | `allServers` | user-groups (`for..of allServers`) | yes |
| 3 | `mcp/components/system/McpServerGroupsAssignmentCard.store` | `allGroups` | mcp-servers | yes |
| 4 | `llm-provider/components/ProviderGroupAssignmentCard.store` | `allGroups` | llm-providers | yes |
| 5 | `mcp/stores/McpServer.store` | `servers` | mcp-servers | yes |

Only #1 actually threw during the gallery run (its endpoint isn't in the
paramless crawl, so it hit an empty shape); #2–#5 share the identical unguarded
pattern and only avoided crashing because their list endpoints were crawled as
real arrays. All five hardened for consistency — a rendered list must never
assume the response field is an array.

### File-render crashes on a missing filename (surfaced by the rich chat conversation)

Rendering the showcase "every block type" conversation (47 messages, file
attachments) crashed the WHOLE conversation on a file block whose `filename` was
undefined:

| # | Site | Bug | Fix |
|---|---|---|---|
| 6 | `file/registry/fileViewerRegistry.ts::scoreSupport` | `filename.split('.')` on undefined → crashed the viewer resolver for every file | `filename?.split(...)` + guard |
| 7 | `file/components/FileCard.tsx` (×3: lines 125/194/255) | `file.filename.split('.')` / `uploadProgress.filename.split('.')` on undefined | `?.split(...)` |

Both are genuine defensive bugs — a file card / viewer resolver must not crash
the surrounding render when a file block arrives without a filename (some
tool/message payloads carry only a mime type). After the guards, the rich
conversation renders fully (markdown, code, tool results, attachments, branches).

**Result: 0 crashes across all 40 web + 44 desktop rendered pages, 0 console errors.**

## Empty-but-correct (NOT bugs)

These render "empty" because the seeded user is an authenticated admin — expected
redirect/guard behavior, not a defect:

- `/auth`, `/auth/link-account` (partly) — redirect away when authenticated.
- `/settings` — index route redirects to the first settings page.
- `/setup` (desktop) — redirects when first-run setup is already complete.

## Loose-typed fixtures (documented limitation, NOT stale data)

`Hub.getAssistants/getCatalog/getCatalogVersion/getModels`,
`McpServer.listAccessible`, `McpServerSystem.list` are served via the `loose`
cast in `crawl.generated.ts` rather than the `satisfies`-typed block. Root cause:
their response types contain **enum unions** (`TransportType`, `UsageMode`, …),
and a JSON `import` widens the recorded literal (`"http"`) to `string`, which
isn't assignable to the union — a TypeScript JSON-import limitation, **not** a
data mismatch. The recorded data has every required field and **passes the ajv
contract test against `openapi.json`** (which validates the actual enum values).
Re-recording from a main-matching binary would NOT change this — verified: the
recorded `McpServer` objects are missing zero required fields.

## Remaining (logged, not crashes)

- **41 web + 2 desktop `pending`** surfaces = interaction-only overlays
  (drawer / dialog / modal / sheet / menu). Not crashes — they simply need a
  gallery entry that renders them in their OPEN state with seeded data. Listed by
  `npm run check:gallery-coverage`.
- The `user-groups` widget architecture (event-only, no mount refetch) is noted
  as a known issue in `CLAUDE.md`; the defensive guard above stops it crashing,
  but the widget still won't live-update after an out-of-band change without an
  event.

## Multi-state pass (empty / error via cassette-swapping)

Rendering every data-page in `empty` and `error` modes (mock `MockMode`) — where
most bugs hide — surfaced robustness gaps. **No render crashes** (0 ErrorBoundary
catches across all 40 pages × empty/error; the 5 array guards above hold). The
findings are handling gaps, logged not spot-fixed (they're a broad design pattern,
not one-line crashes):

- **Load error is indistinguishable from empty (F14 class)** — ~21 data-pages
  render the SAME "No X yet" empty UI on a 500 as on genuinely-empty data, with
  no error/retry affordance. Verified: `settings-users`, `settings-llm-providers`
  ("No provider selected"), `settings-user-groups`, `settings-assistants`,
  `settings-mcp-servers`, `projects`, `onboarding`, … An admin can't tell "the
  server erred" from "there's nothing here". Recommended: a distinct error state
  with retry when `store.error` is set.
- **Unhandled promise rejection on load error** — ~14 stores let the api-client's
  thrown error escape as an unhandled rejection in `error` mode (page still
  renders, but `window.onerror`/`pageerror` fires: "Gallery error state"). The
  load path should `catch` and set an error flag rather than re-throw. Not a
  crash, but noise + a missed error-state opportunity.
- **`hardware-monitor` renders blank** in empty/error (len 0) — no empty/error
  state at all.

These are tracked, not blocking: they need a per-page error-state design pass,
not a mechanical fix.

## Singleton stores that swap on route param (multi-state isolation)

The per-entry `MemoryRouter` isolates the ROUTER but NOT global Zustand
singletons. Audit of route-param-keyed stores:

| Store | Shape | Bleed risk if all-mounted? |
|---|---|---|
| `Chat` (`core/stores/Chat.store`) | **single-active** `conversation` + `messages: Map<msgId,…>` that SWAP on `/chat/:conversationId` | **YES** — all mounted chat entries would show the last-seeded conversation |
| `ProjectDetail` | **single-active** `project` that swaps on `/projects/:projectId` | **YES** |
| `WorkflowRuns` | `runs: Record<workflowId, …>` — **id-keyed** | no (entries coexist) |

**Isolation strategy (by construction):** swap-type detail surfaces
(`/chat/:conversationId`, `/projects/:projectId`) are NEVER all-mounted on the
browse canvas — they carry a REQUIRED route param that the browse enumerator
leaves unresolved, so they're skipped there. They are rendered ONLY via the
URL-isolation path (`?surface=&state=&conversationId=…`), where each combo is a
FULL PAGE RELOAD → a fresh singleton → **zero cross-entry bleed**. Sequential
per-combo rendering is the same mechanism the multi-state screenshots use.

**Swap-TRANSITION correctness** (navigating A→B leaves no stale `conversation`/
`messages`/`project` state) is a runtime interaction property — an **e2e test**,
NOT a static screenshot. Flagged as a separate test to add (see
SEEDED_GALLERY_PLAN.md); the gallery deliberately does not try to screenshot it.

## Summary

- **Crashes found: 5. Fixed: 5. Remaining render crashes: 0** (incl. across all
  empty/error states).
- Robustness gaps logged (not crashes): error≈empty (~21 pages), unhandled-
  rejection-on-error (~14), `hardware-monitor` blank.
- Singleton bleed risk: `Chat` + `ProjectDetail` (single-active-swap) — isolated
  by rendering sequentially via URL, never all-mounted. `WorkflowRuns` id-keyed
  (safe).
- Empty-but-correct: 3 (redirect/guard routes).
- Loose fixtures: 6 (openapi-valid, enum-widening — documented).
- Pending overlays: 41 (interaction-only, tracked by the coverage gate).
