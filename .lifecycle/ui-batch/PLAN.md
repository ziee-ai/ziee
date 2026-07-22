# PLAN — ui-batch

Three independent, user-reported chat-UI defects, shipped as one PR into `khoi`.
All three root causes were located by a prior investigation and re-verified
against the code in this worktree before planning.

This is a **pure frontend** change: no migration, no backend, no OpenAPI regen.
The desktop app consumes the same sources via `@ziee/ui-core` — `git ls-files`
confirms there are **no** desktop mirrors of any file touched here (only
`LeftSidebar.desktop.tsx`, which wraps and renders `CoreLeftSidebar` unchanged),
so one edit covers both apps.

## Items

- **ITEM-1**: The composer model-selector trigger sizes to its CONTENT instead of a
  fixed `max-w-[130px]` cap, so an ordinary model name ("GPT OSS 120B") renders in
  full with no ellipsis when the composer is wide. Drop `max-w-[130px]` and add
  `min-w-0` plus a generous soft ceiling `max-w-[20rem]`, so one pathological name
  cannot swallow the toolbar; `min-w-0` on the component's
  `data-testid="model-selector"` wrapper so it can shrink.
  **No width override** — the kit's own `w-full` is what delivers this: the slot
  wrapper is a flex item whose base size is the label's max-content (content-sized
  when there is room) and which shrinks under pressure. (AMENDED per DRIFT-1.1:
  the plan originally specified `w-auto`. That is wrong — a `<button>` is a form
  control, so `width:auto` is SHRINK-TO-FIT and ignores the space on offer,
  overflowing its container; measured as a 320px trigger inside a 274px composer.)
- **ITEM-2**: The trigger label truncates with a REAL ellipsis under pressure.
  Today the shadcn trigger applies both `*:data-[slot=select-value]:line-clamp-1`
  and `*:data-[slot=select-value]:flex`; Tailwind sorts `display` after
  `line-clamp`, so `display:flex` wins, `-webkit-line-clamp`'s ellipsis goes inert
  and only `overflow:hidden` survives → the label is HARD-CUT into the chevron
  with no "…". Fix via the kit's public `labelRender` seam, rendering the selected
  label as `<span className="block truncate">` — a real block container, so
  `text-overflow: ellipsis` applies, and its `overflow:hidden` gives it a flex
  auto-minimum-size of 0 so it can actually shrink. `labelRender` must return
  `undefined` when nothing is selected, or the kit's `SelectValue` children would
  suppress the `placeholder`.
- **ITEM-3**: In the composer toolbar, the model name yields BEFORE the Send
  button. Today the right group is `shrink-0` as a whole, which forces the LEFT
  toolbar actions to absorb every pixel. Make the right group shrinkable
  (`min-w-0`, drop the group's `shrink-0`), move `shrink-0` onto the Send
  `Button` itself, and give the `toolbar_model` slot `min-w-0`. Send becomes
  structurally incapable of shrinking; the model name gives way first.
  Additionally cap the right group at `max-w-[60%]` of the toolbar ROW. (ADDED
  per DRIFT-1.2: flex shrink alone does not protect the LEFT group, which is
  `flex-1` with a ZERO basis and so never competes for space — it receives only
  what the right group leaves over. Measured at 390px: the right group took its
  full 364px, the left group got 2px, and its `shrink-0` "+" button overflowed
  into the selector. The row has a definite width, so a percentage cap needs no
  container query — DEC-3's rejection of `@container` still stands.)
- **ITEM-4**: A single shared `SidebarSectionTitle` component owns the sidebar
  section-caption padding token AND typography, so the three captions cannot
  drift again. Lives in `src/components/common/` — neutral ground, since the chat
  widget importing from the layouts module would be a cross-module import.
- **ITEM-5**: "Navigation" and "Tools" render via `SidebarSectionTitle` above a
  FLAT `<Menu>`, instead of the kit Menu's `{ type: 'group', label }` wrapper.
  This removes the double-padding stack (Menu `px-2` = 8px + kit group-title
  `px-3` = 12px → 20px) rather than overriding it, landing both captions on the
  same 12px edge as "Recent chats". No menu ROW moves.
- **ITEM-6**: "Recent chats" renders via the same `SidebarSectionTitle`, replacing
  its hand-rolled inline div — which today differs from the Menu group-title in
  weight and tracking (`font-semibold tracking-wide pt-0 pb-0.5` vs
  `font-medium py-1`) despite a comment claiming it "mirrors" it.
- **ITEM-7**: Landing on the standalone New-Chat route collapses the workspace to
  a single pane: `NewChatPage`'s mount effect calls `Stores.SplitView.reset()`
  alongside its existing `Stores.Chat.reset()`. Today, with 2 panes still in the
  store, the URL→workspace reconcile replaces the FOCUSED pane with the new
  conversation and the old split reappears around it. The route boundary is the
  right place for the invariant — `/chat` is a single-pane surface — so it holds
  for the sidebar action, the two `ChatHistoryPage` buttons, the two
  `OnboardingPage` buttons, `useClosePane`'s `navigate('/chat')`, and a deep link
  alike. The in-split "new chat pane" path is UNAFFECTED and must stay working:
  `ConversationPickerPane`'s "Start a new chat" is local state only
  (`setMode('new')`), never a navigation, so `NewChatPage` never mounts and the
  reset never fires.

## Files to touch

Product code:

- `src-app/ui/src/modules/user-llm-providers/chat-extension/components/ModelSelector.tsx` (ITEM-1, ITEM-2)
- `src-app/ui/src/modules/chat/components/ChatInput.tsx` (ITEM-3)
- `src-app/ui/src/components/common/SidebarSectionTitle.tsx` — **new** (ITEM-4)
- `src-app/ui/src/modules/layouts/app-layout/components/LeftSidebar.tsx` (ITEM-5)
- `src-app/ui/src/modules/chat/widgets/RecentConversationsWidget.tsx` (ITEM-6)
- `src-app/ui/src/modules/chat/pages/NewChatPage.tsx` (ITEM-7)

Test + gallery infrastructure:

- `src-app/ui/src/modules/chat/gallery.tsx` — **two** deep-states seeding model
  `display_name`s (siblings of the existing `deep-chat-empty-model-picker` seed),
  so both truncation regimes are renderable backend-free. (AMENDED per DRIFT-1.4:
  planned as one seed. A name that FITS never ellipsizes at any real viewport —
  the correct outcome of content sizing — so the ellipsis path needs a second,
  over-long seed, which also proves the soft ceiling bounds it.)
- regenerated derived artifacts: `gen:gallery-coverage`, `gen:state-matrix`,
  `gen:gallery-seed-registry`
- `src-app/ui/scripts/node-test-hooks.mjs` — **own commit** (DRIFT-1.6 / B3): a
  general extension+realpath fallback in the unit-test resolver. Pre-existing
  breakage, unrelated to this feature, that stopped 10 spec files — including the
  one carrying TEST-1 — from running at all.
- `src-app/ui/tests/e2e/visual/composer-model-selector.spec.ts` — **new**
- `src-app/ui/tests/e2e/layouts/sidebar-title-alignment.spec.ts` — **new**
  (AMENDED per DRIFT-1.3: planned under `tests/e2e/visual/`, but the gallery's
  `PageFrame` renders a route element WITHOUT its layout, so the real sidebar
  never renders there. Moved beside the existing `layouts/sidebar-toggle.spec.ts`
  and run against the real app shell.)
- `src-app/ui/tests/e2e/14-split-chat/new-chat-collapses-split.spec.ts` — **new**
- `src-app/ui/src/modules/chat/core/stores/SplitView.store.test.ts` (extend)

NOT edited after all: `src-app/ui/src/dev/gallery/coverage.ts` (DRIFT-1.5 —
`src/components/common/` is outside the gallery's `surfaceRoots`, so the new
component is not a gallery surface and needs no entry).

Explicitly NOT touched:

- `src-app/ui/src/modules/chat/components/ModelSelector.tsx` — a separate legacy
  selector (`w-[120px]`); nothing imports it. The active one is the
  user-llm-providers component wired through the `toolbar_model` slot.
- `sdk/` (`@ziee/kit`) — a separate submodule repo; a kit edit needs its own PR
  plus a pointer bump. Every fix here is achieved through the kit's PUBLIC API.
- The **settings** sidebar, which also uses kit Menu groups, is deliberately left
  at its current inset.

## Patterns to follow

- **Truncating a kit `Select` trigger label** → the kit's documented
  `labelRender` / `selectedLabel` seam (`sdk/packages/kit/src/kit/select.tsx:49-55`),
  which exists precisely so "the row and the selected value can render
  differently". Not a fork, not an arbitrary-variant override of kit internals.
- **A truncating flex label** → the kit Menu row's own idiom,
  `sdk/packages/kit/src/kit/menu.tsx:217`
  (`<span className="min-w-0 flex-1 truncate text-left">`) — the in-repo
  precedent for "truncate long labels instead of overflowing the rail".
- **`SidebarSectionTitle`** → mirrors the caption it replaces
  (`RecentConversationsWidget.tsx:138-142`) for typography, and the kit Menu
  group-title (`menu.tsx:174`) for its role as decorative section chrome. It is a
  presentational leaf like its `src/components/common/` siblings.
- **Collapsing the split workspace** → `Stores.SplitView.reset()`, the established
  primitive, called exactly as at `core/pane/useOpenConversation.ts:82` and `:143`
  and `core/pane/useNavigateAwayOnDelete.ts:60` ("collapse to single-pane
  (URL-driven)").
- **Store reset in a page mount effect** → `NewChatPage.tsx:10-11`'s existing
  `Stores.Chat.reset()`; the new call sits beside it in the same effect.
- **Gallery deep-state seeding a store** → `modules/chat/gallery.tsx:147-155`
  (`deep-chat-empty-model-picker`, which holds `ModelPicker` empty). The new
  long-name seed is its sibling and uses the same `holdPatch`/`setState` idiom.
- **Geometry-assertion visual spec** → `tests/e2e/visual/chat-collapse-borders.spec.ts`
  (issue #183), the repo's reference for proving a layout EFFECT rather than a
  mechanism: `STANDALONE_PATH` + `?surface=…&theme=…`, `page.evaluate` measuring
  `getBoundingClientRect`/`getComputedStyle`, guards that refuse to pass vacuously.
- **Split-chat e2e** → `tests/e2e/14-split-chat/new-chat-adopt.spec.ts` for the
  split-construction preamble (`chat-split-btn`, `chat-pane-0/1`) and the real
  provider/model setup helpers.

## UI-surface checklist

This feature adds **no new surface** — it repairs three existing ones. The
checklist is answered against what changes:

- **Precedent** — each fix is defined BY its precedent: the model label mirrors
  the kit Menu row's truncation idiom; the two sidebar captions adopt the
  "Recent chats" caption; the split collapse reuses the existing `reset()`
  call-sites. No new visual language is introduced.
- **Scale / cardinality** — unchanged. The model dropdown's option count and the
  sidebar's virtualized recent list are untouched; ITEM-1's soft ceiling is
  precisely the bound on an unbounded model-name length.
- **Device size / responsive** — the model selector is the one size-sensitive
  change, so it is verified in BOTH regimes: a wide composer (name in full, no
  ellipsis) and a narrow split pane (ellipsized, Send intact). The sidebar
  captions and the split reset are width-invariant, and the sidebar is not
  rendered at all in icon-only mode (`!isIconOnly` already guards the captions).
- **Populated-render review** — the gallery deep-state seeds a REAL long model
  name and a populated recent-chats list, so the design-critic pass reviews
  loaded data, not an empty shell.
- **User-visible progress** — n/a; no ingest or long-running work is added.
- **Input economy** — n/a; no new input. ITEM-1/2 make an existing picker's
  current value more legible, which is the same principle.
- **JTBD** — (a) *"which model am I about to send to?"*: the user must read the
  current model at a glance without opening the dropdown — today a long name is
  cut mid-word into the chevron, so they cannot. (b) *"scan the sidebar's
  sections"*: three captions on three different left edges break the vertical
  scan line the sidebar exists to provide. (c) *"start a fresh chat"*: clicking
  New Chat means a clean single conversation — being dropped back into the
  previous two-pane split with the new chat wedged into one pane is the opposite
  of what was asked for.
- **Multi-instance / workspace surfaces** — ITEM-7 touches exactly this. The
  collapse is scoped to the top-level `/chat` ROUTE, so the per-pane new-chat
  path (picker → composer → in-pane adoption via `ConversationPage.tsx:776-780`)
  keeps its own behavior. Both directions are covered by tests (the new collapse
  spec and the existing `new-chat-adopt.spec.ts`).
- **URL-as-view-into-focus** — ITEM-7 is a URL/workspace reconciliation bug. The
  fix restores the invariant at the one route where the URL carries NO
  conversation: `/chat` means "no pane targets anything yet".
- **Platform-provided affordances** — unchanged.

## Accepted tradeoffs (human-approved)

- **ITEM-1/3**: the toolbar reflows when switching between a short and a long
  model name. Fit is preferred over stable width (approved — see DEC-2).
- **ITEM-5/6**: all three captions land at 12px, which leaves each caption 8px
  LEFT of its own rows — exactly the relationship "Recent chats" already has
  today (approved — see DEC-4).
- **ITEM-7**: collapsing also clears the persisted workspace, so browser-Back
  after New Chat does not resurrect the old split (approved — see DEC-6).
