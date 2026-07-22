# INFRA_INTEGRATION — ui-batch

The three mandatory per-item walks (user-experience, infrastructure-integration,
entity-lifecycle), plus the environmental defects the implementation surfaced.

---

## ITEM-1 / ITEM-2 / ITEM-3 — composer model selector

### User-experience walk

A user glances at the composer to answer one question before hitting send:
*which model am I about to spend tokens on?* Before this change a name longer than
130px was cut mid-word into the chevron, so the answer required opening the
dropdown — on every single send. After: the name is simply readable. The user
only meets an ellipsis when the composer is genuinely too narrow (mobile, or a
name past the ceiling), and even then the full name is one click away in the open
list, which sizes past the trigger.

The accepted cost (DEC-2) is that the toolbar reflows when switching models. A
user switching models is *looking at* the toolbar, so the shift is attributable
rather than mysterious — unlike the previous silent truncation.

### Infrastructure-integration walk

- **Extension-slot system** — the selector is not imported by `ChatInput`; it
  arrives through the `toolbar_model` slot from the `user-llm-providers` chat
  extension. `ExtensionSlot` renders a real wrapper `<div className={…}>`
  (`sdk/packages/framework/src/slots.tsx:198`), which IS the flex item, so the
  `min-w-0` needed for shrinking has somewhere to land. Verified by reading the
  factory, not assumed.
- **Split panes** — the composer renders once per pane, and the model selection
  is per-pane (`selectedByConversation`, keyed by the pane's conversation). The
  changes are pure layout inside one composer, so they hold per pane; the narrow
  regime is exactly the split-pane case, which is why it is measured.
- **The `@ziee/kit` boundary** — `sdk/` is a separate submodule repo, so the fix
  had to work through the kit's public API. `labelRender` is a documented prop
  for precisely this ("the row and the selected value can render differently").
  No kit file is touched.
- **Other `Select` consumers** — the `className` is passed at ONE call site, and
  the kit component is untouched, so no other Select in the app changes.
- **Error / loading branches** — the `error && providers.length === 0` fallback
  button and the `loading` placeholder sit upstream of the `<Select>` and are
  unchanged; `labelRender` returning `undefined` for "nothing selected" is what
  keeps the placeholder working.

### Entity-lifecycle walk

The entity here is *the selected model*.
- **Added** — a provider/model appearing (sync `sync:user_llm_provider` refetch)
  re-renders the options; the trigger re-measures naturally, since its width is
  content-derived rather than stored.
- **Removed / disabled** — if the selected model disappears, `selectedModelId`
  no longer resolves to an option, `customDisplay` is `undefined`, and the
  placeholder shows. This is the pre-existing `deep-chat-no-models` gallery
  state, which still renders correctly (it is unrelated to `labelRender`, which
  is only consulted for a RESOLVED option).
- **Mutated** — a renamed model changes the label text; because nothing caches a
  measured width, the trigger simply re-lays out. This is the reflow of DEC-2.
- **Access-loss** — `loadProviders()` is permission-gated on
  `user_llm_providers::read` and returns early without it; the picker then has no
  providers and takes the same empty path as above. Unchanged by this work.

---

## ITEM-4 / ITEM-5 / ITEM-6 — sidebar section captions

### User-experience walk

The sidebar's job is to be scannable. Three captions on two different left edges
break the vertical line the eye follows, and the misalignment reads as an
accident rather than a hierarchy. After the change the captions form one column,
hanging 8px left of their rows so section headings and row content are visually
distinguishable (the relationship "Recent chats" already had).

### Infrastructure-integration walk

- **Slot system** — Navigation and Tools are built from the `sidebarNavigation` /
  `sidebarTools` slots, filtered by `evaluatePermission`. The caption renders
  under `items.length > 0`, so a user whose permissions filter a section down to
  nothing still sees NO caption — the permission behavior is preserved exactly,
  because the guard was left on the same expression.
- **Kit Menu contract** — `selectedKey` / `ancestorKeys` / `onSelect` key off
  `item.key` and are indifferent to group nesting, so flattening `items` changes
  no selection behavior. Per-item testids (`${testid}-item-${key}`) are unchanged;
  only the group testids disappear, and nothing references them (grepped).
- **Desktop app** — `LeftSidebar.desktop.tsx` wraps and renders `CoreLeftSidebar`
  with only outer-chrome props, so it inherits the change with no desktop edit
  and no security-relevant logic dropped (R2-3).
- **Icon-only rail** — audited and deliberately unchanged (DEC-11): this sidebar
  never passes the kit's `collapsed` prop (icon-only is implemented by nulling
  each item's `label`), so the kit's caption guard is inert and the captions
  already rendered in the rail before this change. Preserved as-is rather than
  smuggling an unrelated behavior change into an alignment fix.
- **Gallery** — `src/components/common/` is outside the gallery's `surfaceRoots`
  (`["src/modules", "src/components/ui"]`), so the new component needs no
  coverage entry (DEC-10).

### Entity-lifecycle walk

The entity is *a sidebar section*.
- **Added** — a module registering a new nav/tool slot item appears under the
  existing caption; no per-item caption logic exists to drift.
- **Removed** — the last item in a section removing itself hides the caption via
  the same `length > 0` guard (unchanged).
- **Access-loss** — a permission revocation re-filters `sortedNavigation` /
  `sortedTools` reactively (they read `user`/`permissions`), so a section that
  empties out takes its caption with it. Both directions — the local
  `/auth/me` refresh and the cross-device `sync:session` re-bootstrap — land on
  the same reactive read, so there is no separate handler that could be missed.
- **Recent chats** — the caption is rendered from all FOUR return branches
  (error / loading / empty / loaded), so a conversation list that empties, fails,
  or reloads never loses or shifts its header.

---

## ITEM-7 — `/chat` collapses the split workspace

### User-experience walk

"New Chat" is the most unambiguous intent in the app: give me a blank
conversation. Landing back in the previous two-pane split with the new chat
wedged into one pane contradicts that so directly that the user cannot tell
whether the new chat was created at all. The fix makes the route mean what it
says.

The accepted cost (DEC-6) is that browser-Back no longer resurrects the split.
That is coherent: the split was dismissed deliberately, not incidentally.

### Infrastructure-integration walk

- **Router** — `/chat` and `/chat/:conversationId` are separate routes with
  separate components, so the reset is scoped to the standalone new-chat page and
  cannot fire while a conversation is open.
- **Workspace persistence** — `reset()` empties the layout; the store's debounced
  `saveWorkspace` REMOVES an empty workspace rather than writing one, so no stale
  blob is left behind. Restore is additionally gated on a same-tab reload, so a
  pop-out/new tab could not resurrect it either.
- **The reconcile loop** — with `panes: []`, `ConversationPage`'s URL→workspace
  effect early-returns (`panes.length < 2`) and the workspace→URL effect
  early-returns (`panes.length === 0`), so the single-pane path is genuinely
  URL-driven and the two effects cannot ping-pong.
- **Existing `reset()` callers** — `useClosePane` and `useNavigateAwayOnDelete`
  already reset before navigating to `/chat`; `reset()` assigns fixed empty
  values, so the second call is idempotent.
- **The in-pane picker** — traced end to end: "Start a new chat" is
  `setMode('new')`, pure local state with no navigation, so `NewChatPage` never
  mounts and the reset never fires. The pane's own adoption effect is untouched.

### Entity-lifecycle walk

The entity is *a pane*.
- **Added** — unaffected; opening panes still goes through the reducer.
- **Removed (local)** — closing the last pane already resets and navigates to
  `/chat`, which now also resets — idempotent, no double-handling.
- **Removed (sync / cross-device)** — a conversation deleted on another device
  prunes its pane via the store's `sync:conversation` handler; the local delete
  path is `useNavigateAwayOnDelete`. Both were already wired and neither is
  changed here — this item adds a reset on a route that has NO pane-bearing
  entity, so it cannot interact with either.
- **Mutated** — n/a.

---

## Environmental defects surfaced (pre-existing, NOT caused by this diff)

Both were found by running the gates, and both are proven pre-existing by
reproducing them without this feature's code.

### 1. `npm run test:unit` — 10 spec files died at import

Every spec importing a store that pulls in `@ziee/framework/store-kit` failed
with `ERR_MODULE_NOT_FOUND` before a single assertion ran. The package's exports
map is `"./*": "./src/*"`, which yields an EXTENSIONLESS path; Vite resolves
that, Node's ESM resolver does not. Reproduced on the untouched main checkout
(`/home/khoi/ziee/ziee`) for a file this branch never touches
(`voice/VoiceModel.store.test.ts`), so it is not a worktree artifact.

Fixed generally in `src-app/ui/scripts/node-test-hooks.mjs` — the ONLY shared
test-infra file this branch touches, kept to its own commit per B3, and written
as a general extension/index fallback rather than anything specific to this
feature. Three layers had to be peeled:
1. `ERR_MODULE_NOT_FOUND` on an extensionless file → retry `.ts`/`.tsx`/`/index.*`;
2. `ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING` → a workspace package is a
   SYMLINK under `node_modules` and Node refuses to strip types there, so the
   hook returns the `realpathSync` location instead;
3. `ERR_UNSUPPORTED_DIR_IMPORT` on a directory import (`./events`) → same retry.

Result: **10 failing files → 8**, recovering `SplitView.store.test.ts` (which
carries this feature's TEST-1) and `MessageViewState.store.test.ts`; total
executed tests rose 456 → 476.

The remaining **8 are pre-existing failures of a DIFFERENT class** and are out of
scope: `ERR_UNSUPPORTED_TYPESCRIPT_SYNTAX` ("TypeScript enum is not supported in
strip-only mode") in the voice/scheduler/auth/chat-history store specs, and one
test-local `TypeError: Cannot read properties of undefined (reading 'config')`.
Fixing those means either removing `enum`s from product source or adding a real
transpiler to the unit runner — a separate piece of work, in modules this branch
does not touch.

### 2. `npm run check` → `check:testid-registry` is stale on the BASE commit

`sdk/packages/kit/src/testIds.generated.ts` is out of date with the app tree
independently of this branch. Proven by stashing the only two files this feature
adds testids to and re-running the check on the otherwise-pristine tree: it still
reports *"testIds.generated.ts is stale — run `npm run gen:testid-registry` and
commit."* A regen adds 15 ids, of which only 3 are this feature's
(`layout-sidebar-nav-title`, `layout-sidebar-tools-title`, `chat-recent-title`);
the other 12 (`chat-pane-*`, `chat-split-btn`, `chat-picker-pane-*`,
`chat-open-in-new-window`) are from earlier split-chat work that never
regenerated.

**Deliberately NOT fixed here.** The registry lives in the `sdk` SUBMODULE, so
regenerating it produces a commit in a different repository plus a submodule
pointer bump. Bumping the pointer to a commit that has not been pushed to the sdk
remote would break every other clone, and the scope for this work is a single PR
into `khoi`. Recorded for the human in TEST_RESULTS.md as a pre-existing gate
failure with its reproduction, rather than silently absorbed or worked around.
