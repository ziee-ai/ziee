# DRIFT-2 — split-chat-multipane (v2 isolation refactor)

Implementation-vs-plan reconciliation for the v2 workspace + isolation work
(ITEM-24..39) and the origin/main merge. Each divergence is recorded and
resolved; every drift is `impl-wins` with an amended-plan rationale or `resolved`.

- **DRIFT-2.1** — verdict: impl-wins — **File composer buffer uses an OWNERSHIP-MAP,
  not nested per-pane buffers or a new store.** ITEM-32 said "per-pane-instanced OR
  pane-keyed". `selectedFiles`/`uploadingFiles` stay top-level Maps (preserving the
  reactive destructures + the global thumbnail/preview caches) with two side maps
  `fileOwner`/`uploadOwner` (fileId/progressId → composerPaneKey). The buffer actions
  filter/scope by pane key. This is the "pane-keyed" branch of the plan — lowest-risk
  on the critical upload path, and it keeps `File.messageFilesCache`/`thumbnailUrls`
  global (id-keyed) exactly as the plan mandated. `SINGLE_PANE_KEY` = single-pane.

- **DRIFT-2.2** — verdict: impl-wins — **MCP approvals are CONVERSATION-keyed, not
  pane-keyed.** ITEM-19/33 said "pane-key the state". Approvals semantically belong to
  a CONVERSATION (they resume that conversation's generation), so a `Map<convKey,
  decisions>` (`approvalKeyOf`) is more correct + pane-portable (pop-out/move keeps the
  approval) than a pane key. This still fixes the wrong-pane bug (a pane reads/clears
  only ITS conversation's decisions) — proven by `approvalRouting.test.ts`. The pure
  routing logic was extracted to `approvalRouting.ts` (enum-free, node-testable).

- **DRIFT-2.3** — verdict: impl-wins — **`getSelectedServersConfigFor(convId)`
  resolves the SENDING pane's MCP config from the keyed `conversationConfigs`, not a
  reshape of the single-active `selectedServers`.** The plan wanted per-pane server
  selection; because `conversationConfigs` is already conversation-keyed, a read-side
  getter delivers per-pane correctness without reshaping the load-bearing active
  pointer — far lower risk on the tool path than the plan's implied full reshape.

- **DRIFT-2.4** — verdict: impl-wins — **ctx-less send hooks resolve the sending pane
  via focus, not by threading `paneId` through every hook.** `paneId` IS threaded where
  a ctx exists (`composeRequestFields(ctx.paneId)`, `provideUserContent(composerPaneId)`)
  — the accurate path. The three ctx-less hooks (`beforeSendMessage` / `onMessageSent` /
  `onMessageEditRestore`) resolve the sending pane via `Stores.Chat.$` / `SplitView.
  focusedPaneId` (the focused pane == the sending pane, because sending/editing
  pointer-focuses the pane first). Avoids widening 3 more hook signatures + their
  registry/store call sites; consistent in practice (all resolve to the same pane).

- **DRIFT-2.5** — verdict: impl-wins — **`PaneExtensionRuntime` wraps the catalog +
  drives ungated init/cleanup loops; single-pane keeps the gated global registry.**
  ITEM-34 said split into `ExtensionCatalog` + `PaneExtensionRuntime`. The registry IS
  the catalog (register/descriptors/dispatch); the runtime is the per-pane lifecycle
  half (own `initialized`, own `ctx`). `initialize()`/`cleanup()` stay as single-pane
  gated wrappers over the shared `initializeExtensions`/`cleanupExtensions` bodies the
  runtime drives per-pane — same architecture, less code churn than a hard file split.

- **DRIFT-2.6** — verdict: impl-wins — **Streaming: per-pane direct callback replaces
  the global `chat:token` bus (ITEM-6/35), with a legacy fallback.** The client takes
  `onFrame`/`onReconnect`; when omitted it still emits the global event, so any
  non-pane consumer is unaffected. Both Chat.store paths pass handlers, so the global
  bus is dead in practice; the fallback is defensive, not a second live path.

- **DRIFT-2.7** — verdict: impl-wins — **Split surface covered via allow-list `via`
  entries + e2e, not standalone gallery stories.** ITEM-15/23 wanted gallery cells. A
  live multi-pane runtime container (N `ChatPaneProvider`s + N SSE streams) is not
  expressible as a backend-free gallery story; `coverage.ts` marks SplitChatView /
  ConversationPickerPane / PaneTabStrip `via` (exercised through the 14-split-chat
  e2e), which is the coverage system's designed mechanism for exactly this. `npm run
  check` (incl. `check:gallery-coverage` + `check:state-matrix`) is green.

- **DRIFT-2.8** — verdict: resolved — **origin/main merged in (kb + voice +
  scheduled-tasks + UI, tip 470d9ff51).** No migration collisions (this branch adds no
  migrations). Generated artifacts (testids / gallery-coverage / state-matrix)
  regenerated from source + the coverage scaffolds updated; the Tauri capabilities
  schema regenerated for the `chat-*` pop-out windows. Backend `cargo check
  --workspace` is green (private target dir). `npm run check` green.

- **DRIFT-2.9** — verdict: impl-wins — **`KnowledgeBaseComposer` stays a global
  `defineStore` (per-conversation-keyed at the SERVER), a bounded, main-inherited
  limitation OUTSIDE the enumerated ITEM-5 composer set.** The blind audit flagged
  its single `currentConversationId` as a per-pane collapse risk. But (a) the store
  file is NOT in this branch's diff — it arrived with main's KB feature (tip
  470d9ff51), and this branch only made the KB *extension's* subscription teardown
  per-pane (`paneKbSubs` WeakMap keyed by `ctx.chatStore`); (b) `search_knowledge`
  resolves a conversation's attached KBs SERVER-side from the conversation id —
  nothing is injected into the send request — so grounding is per-conversation
  correct regardless; the bounded residual is only the composer's *displayed* KB
  selection + a toggle target when two KB-using panes are open at once. Reshaping a
  main-owned store (store + 2 components + extension) under this feature's scope was
  judged higher-risk than the bounded UI-only residual; deferred to a KB-owned
  follow-up. Recorded, not silently dropped.

- **DRIFT-2.10** — verdict: resolved — **TEST-55 re-scoped: "same conversation in
  two panes" is UNREACHABLE by design, so the spec proves the guard + frame
  routing instead.** The phase-6/7 audit proved three independent guards
  (`openPane` / `setPaneConversation` / `reconcileOpen`) FOCUS the existing pane
  rather than duplicate a conversation — so the literal "two panes on the same
  `conversation.id`" state TEST-55 described cannot be produced through the UI (the
  ITEM-35 "same-conversation split" code is defensive hardening, unit-proven via
  the per-instance `onFrame` conversation filter). TEST-55 keeps its ID (A5) and is
  re-scoped in TESTS.md to assert the reachable equivalent: the one-conversation-
  per-workspace guard e2e (opening A into pane B focuses A, no duplicate) + the
  frame-routing correctness (pane A streams; pane B, a DIFFERENT conversation, gets
  none of A's frames). `mobile-columns.spec.ts` (a stale v1 columns-at-all-
  viewports orphan, referenced by no TEST-ID) was removed and replaced by
  `mobile-tabs.spec.ts` (TEST-23's shipped tab-strip vehicle).

- **DRIFT-2.11** — verdict: impl-wins — **The composer MCP status chip is a GLOBAL
  single-active display; per-pane MCP correctness lives in the send/approval path.**
  The FIX_ROUND-3 blind audit found `McpStatusRow` reads the singleton
  `Stores.McpComposer.selectedServers` (no per-pane store / no conversationId prop),
  so both split panes render the same chip. This is acceptable: the per-pane MCP
  behavior that MATTERS — the wrong-pane tool-approval routing (`approvalKeyOf`,
  the flagship ITEM-33 bug) and the per-conversation send config
  (`getSelectedServersConfigFor`) — IS conversation-keyed and unit-proven
  (`approvalRouting.test.ts`). Making the chip DISPLAY per-pane would require a
  per-pane McpComposer store instance (a larger reshape than ITEM-33 scoped); the
  display staying single-active is a bounded, documented limitation. TEST-53 is
  re-scoped accordingly (it asserts the per-pane config SURFACE, not chip isolation).

- **DRIFT-2.12** — verdict: resolved — **`SplitChatView` rendered TABS on desktop /
  COLUMNS on mobile (inverted `!md`); fixed to `if (md)`.** The FIX_ROUND-3 audit +
  an empirical run proved the branch was reversed: `useWindowMinSize().md` is TRUE
  at ≤768px (main's 2026-05 breakpoint-table fix made every key `width <= threshold`),
  so `if (!md) return tabs` put tab mode on desktop (≥769px) and columns on mobile —
  the opposite of the intent. `independent-input.spec.ts` (columns @1280) failed on
  the un-fixed code (`chat-pane-0` hidden in tab mode). Fixed to `if (md) return tabs`:
  desktop (md===false) tiles columns, ≤768px shows the tab strip — which makes the
  column-mode specs (open-in-split / sidebar-reroute / header / workspace-persist)
  and `mobile-tabs.spec.ts` (tabs @390) all correct. A real shipped functional bug
  the blind audit caught.

- **DRIFT-2.13** — verdict: impl-wins — **The send-lifecycle hooks
  (`onMessageSent`/`onStreamError`/`afterStreamComplete`) now receive the OWNING
  pane id, superseding DRIFT-2.4's focus-resolution for these three.** DRIFT-2.4
  resolved the ctx-less send hooks via `focusedPaneId` on the premise "the focused
  pane == the sending pane". FIX_ROUND-4 proved that premise FALSE for the ASYNC
  hooks: `onMessageSent` runs after the send round-trip and the stream hooks fire
  seconds later, by which time focus may have moved — wrong-pane file clear/restore
  (data loss). Amended: the three hooks gained an optional `ownerPaneId` param
  threaded from the dispatching pane's store (`get().paneId`), the stable owning
  pane. `beforeSendMessage` is unaffected (it fires synchronously at send-start
  where focus == sending pane still holds). This is a per-pane-correctness
  improvement, not a signature-churn regression — the param is optional and
  ignored by the other extensions that implement these hooks.

- **DRIFT-2.14** — verdict: impl-wins — **`SplitChatView` renders ONE tree for both
  tab + column modes** (not the two-branch structure the tab-strip work first
  produced). Crossing the `md` breakpoint with a two-branch render changed each
  pane's wrapper element TYPE (`<div>` vs `<Fragment><div>`) at the same key,
  remounting every `ChatPaneProvider` (recreating the per-pane store, tearing down
  live streams). The unified tree (`<Fragment key=paneId><div key="pane">` always;
  divider + tab strip toggle around it, keyed) makes a mode switch a
  className/style/role change only — panes stay mounted (the ITEM-30 guarantee).
  Both modes remain e2e-proven (`independent-input` columns @1280, `mobile-tabs`
  @390).

**Unresolved drifts:** 0
