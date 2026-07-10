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

**Unresolved drifts:** 0
