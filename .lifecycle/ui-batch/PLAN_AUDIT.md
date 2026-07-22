# PLAN_AUDIT — ui-batch

Audit of PLAN.md **against the codebase**, before writing any code. Every claim
below was checked by reading the actual source (app + the `sdk`/`@ziee/kit`
submodule), not inferred.

## Breakage risk

**ITEM-1/2 — the `Select` trigger.** `cn` in the kit is
`twMerge(clsx(...))` (`sdk/packages/kit/src/lib/utils.ts:5`), and the kit composes
the trigger class as `cn('w-full', className, showClear && 'pr-14')`
(`kit/select.tsx:174`) — the app's `className` is passed AFTER `w-full`, so
`w-auto` wins its conflict group and the trigger really does become
content-sized. `max-w-[130px]` likewise disappears cleanly because the app string
is the only source of a `max-w-*`. **No other consumer is affected**: this
`className` is passed at ONE call site (the composer's own selector); the kit
component itself is untouched, so every other `Select` in the app keeps its
current `w-full` behavior.

Risk checked and cleared: `labelRender` is a **first-class public prop**
(`kit/select.tsx:52`, documented as controlling "the selected value in the
trigger"), not an internals reach-through. Its return value flows to
`customDisplay` → `<SelectValue placeholder={placeholder}>{customDisplay}</SelectValue>`
(`kit/select.tsx:149-176`). Base UI's `Value` renders `children` INSTEAD of the
placeholder when children are non-null, so `labelRender` MUST return `undefined`
for the no-selection case — this is the one way to break the placeholder, and it
is called out explicitly in ITEM-2. The `loading`/`disabled`/error-state branches
are upstream of the `<Select>` entirely and unaffected.

**ITEM-3 — the composer toolbar.** Removing `shrink-0` from the RIGHT group is
the only behavioral risk in the file, and it is contained: the Send `Button`
receives `shrink-0` in the same edit, so the element the old comment was
protecting ("`shrink-0` keeps Send fully visible") stays protected — by a tighter
guard than before. The left group's `+` trigger is already
`inline-flex shrink-0` (`ChatInput.tsx:89`), so it cannot collapse either. Net:
the two things that must never shrink still cannot; only the model label can.

**ITEM-5 — dropping the kit Menu group wrapper.** `navigationItems` and
`toolsItems` are ALREADY flat `MenuItem[]` (`LeftSidebar.tsx:122-132`); the group
wrapper is constructed inline at the render site only. So the change is
`items={[{type:'group',…,children:navigationItems}]}` → `items={navigationItems}`
plus a sibling title — no data reshaping, no change to `selectedKey` /
`ancestorKeys` / `onSelect`, which key off `item.key` and are indifferent to
nesting. The derived per-item testids (`${testid}-item-${key}`) are computed from
the item key and are **unchanged**; only the group testids
(`${testid}-group-${i}`) disappear, and those are derived at runtime INSIDE the
kit rather than being source literals. Grepped: no spec, helper, or registry in
`src-app/ui/tests/**` or app code references `layout-sidebar-nav-menu-group-*`
(the only repo hits are `settings-nav-menu-group-12` inside the generated
`GEOMETRY_FINDINGS.*` audit artifacts — a different menu, and a regenerated file).

**ITEM-7 — `SplitView.reset()` on the `/chat` route.** Checked every existing
navigation to `/chat`: `useClosePane` (`useOpenConversation.ts:141-145`) and
`useNavigateAwayOnDelete.ts:60` BOTH already call `reset()` immediately before
navigating there, so the new call is idempotent on those paths — it cannot
double-clear anything, since `reset()` assigns fixed empty values rather than
mutating incrementally (`SplitView.store.ts:220-226`). `ChatHistoryPage.tsx:136,188`
and `OnboardingPage.tsx:129,161` navigate to `/chat` WITHOUT resetting today;
those become newly-correct rather than newly-broken (landing on the new-chat page
should not leave a stale split behind it).

The in-split new-chat path was traced end-to-end and is **not** reachable from
this change: `ConversationPickerPane`'s "Start a new chat" is
`onClick={() => setMode('new')}` (`ConversationPickerPane.tsx:117`) — pure local
component state, no `navigate`, no route change — so `NewChatPage` never mounts
and the reset never fires. Adoption continues through
`ConversationPage.tsx:776-780`.

## Pattern conformance

- **ITEM-2's truncating span** matches the kit's own in-repo idiom for exactly
  this problem — `menu.tsx:217`'s `<span className="min-w-0 flex-1 truncate text-left">`,
  commented "truncate long labels instead of overflowing the rail". The fix uses
  the same primitive (`truncate` on a block-level span inside a flex row) rather
  than inventing a mechanism.
- **ITEM-7** matches `reset()`'s three existing call sites verbatim in intent
  ("collapse to single-pane (URL-driven)"), and sits beside the sibling
  `Stores.Chat.reset()` already in that effect (`NewChatPage.tsx:11`).
- **ITEM-4's placement** — audited and it CORRECTS a factual error in PLAN.md.
  PLAN.md claimed `components/common/` "already carries 5 `coverage.ts` entries";
  those 5 are `modules/mcp/components/common/*`, a module-scoped directory, not
  the top-level `src/components/common/`. The gallery scanner's roots are
  `["src/modules", "src/components/ui"]` (`src-app/ui/gallery.config.json:3`,
  walked at `gen-gallery-coverage.mjs:37-75`), so a `.tsx` under
  `src/components/common/` is **not a gallery surface**: it needs no coverage
  entry, no state-matrix cell, and no regen. Confirmed empirically — none of the
  existing `components/common/*.tsx` (`ListPagination`, `SettingsSectionStatus`,
  `BlockedImage`, …) appear in `coverage.ts` or `galleryCoverage.generated.ts`.
  This makes the chosen location MORE conformant than planned, and reduces the
  expected churn. PLAN.md's "Files to touch" is corrected accordingly in phase 5's
  drift log rather than silently.
- **Kit boundary respected** — every fix uses the kit's public API
  (`labelRender`, `className`, `items`). Nothing forks or overrides kit internals,
  which matters concretely here because `sdk/` is a **separate submodule repo**: a
  kit change would need its own PR plus a pointer bump, outside this PR's scope.

## Migration collisions

**None possible.** This branch adds zero migrations. Migrations live per-module
under `src-app/server/src/modules/<module>/migrations/` (there is no longer a
single `src-app/server/migrations/` directory); the globally highest is
`202607146095_workflow_grant_permissions.sql`. No backend file is touched. See
BASE.md.

## OpenAPI regen

**Not required.** No Rust type, route, handler, or permission changes, so neither
`openapi/openapi.json` nor `src/api-client/types.ts` moves — in `ui/` or in
`desktop/ui/`. The `types_ts_parity` golden test is therefore not implicated, and
`just openapi-regen` is not part of this feature's gate.

Related check — **no new permission is introduced**, so the A9 (backend deny) and
A10 (restricted-user `[negative-perm]` e2e) obligations do not attach to this
diff. The three fixes are cosmetic/behavioral on surfaces that are already
permission-gated upstream (the sidebar's widget slots filter on `isAllowed`,
`LeftSidebar.tsx:85-87`, unchanged).

## Desktop parity (R2-3)

`git ls-files src-app/desktop/ui/src/modules/{chat/pages,chat/widgets,user-llm-providers}`
returns **empty** — the desktop app has no hand-written override of any file this
branch touches; it consumes them through `@ziee/ui-core`. The one desktop file in
the neighbourhood, `LeftSidebar.desktop.tsx`, renders `<CoreLeftSidebar …/>`
(`:136`, `:205`) with only outer-chrome props (`rootStyle`/`rootClassName`), so it
inherits ITEM-5/6 unchanged and carries no security-relevant logic that could be
dropped. No desktop-side edit is needed, and none is planned.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — `w-auto`/`max-w-[20rem]` resolve correctly through the kit's `cn('w-full', className)` (twMerge, app string last); single call site, no other `Select` consumer affected.
- **ITEM-2** — verdict: PASS — `labelRender` is a documented public seam; the one hazard (a non-null return suppressing the placeholder) is identified and handled in the item text. Mechanism mirrors `menu.tsx:217`.
- **ITEM-3** — verdict: PASS — moving `shrink-0` from the group onto the Send button tightens rather than loosens the "Send never shrinks" guarantee; the `+` trigger is independently `shrink-0`.
- **ITEM-4** — verdict: CONCERN — placement is correct and MORE conformant than planned, but PLAN.md's stated rationale ("5 sibling coverage entries") is factually wrong: `src/components/common/` is outside the gallery's `surfaceRoots`, so there is NO coverage/state-matrix entry to add and no regen. Carry this correction into DRIFT-1 rather than leaving PLAN.md's "Files to touch" overstated.
- **ITEM-5** — verdict: CONCERN — the mechanical change is safe (items are already flat; no test depends on the group testids), BUT the kit's `collapsed` prop is NEVER passed by this sidebar (`grep collapsed LeftSidebar.tsx` → only a comment at `:110`); icon-only mode is implemented by nulling each item's `label` (`:119`, `:125`, `:131`). So the kit's `{!collapsed && …}` caption guard (`menu.tsx:174`) is inert here and the "Navigation"/"Tools" captions **already render as text in icon-only mode today**. The replacement must render under the SAME (unconditional) condition to avoid smuggling an unrelated behavior change into an alignment fix. Whether a text caption belongs in an icon rail is a pre-existing question, explicitly OUT OF SCOPE for this PR — recorded here so it is a deliberate non-change, not an oversight.
- **ITEM-6** — verdict: PASS — `headerOnly` is a single local referenced from all four return branches (`RecentConversationsWidget.tsx:150,169,178,232`); swapping its body reaches every state (error / loading / empty / loaded) at once.
- **ITEM-7** — verdict: PASS — idempotent against the two existing `reset()`-then-`navigate('/chat')` call sites; strictly corrective on the four navigations that do not reset today; provably unreachable from the in-pane picker path, which never navigates.

**No BLOCKED verdicts.** The two CONCERNs are both "record and constrain", not
"amend the plan": ITEM-4's is a documentation correction to carry into the drift
log, and ITEM-5's is an explicit scope boundary (preserve icon-only rendering
exactly as-is).
