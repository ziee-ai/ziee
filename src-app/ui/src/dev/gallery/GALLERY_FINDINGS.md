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

## Summary

- **Crashes found: 5. Fixed: 5. Remaining crashes: 0.**
- Empty-but-correct: 3 (redirect/guard routes).
- Loose fixtures: 6 (openapi-valid, enum-widening — documented).
- Pending overlays: 43 (interaction-only, tracked by the coverage gate).
