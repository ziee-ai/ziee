# HUMAN_FEEDBACK — chats-page-virtualization

Living ledger of human directives/feedback. One entry per item, verbatim,
resolved before merge (any `[status: open]` fails Phase 9).

- **FB-1** [status: resolved] — "Sync main now — the overlapping feature just merged. live4 sidebar-recent infinite-scroll PAGING is now on origin/main (tip e2b5bba3e) … Merge origin/main into your branch NOW … and RECONCILE the ChatHistory.store overlap: your virtualization must COMPOSE with live4 paging … then RE-RUN your Phase 6-8 … and confirm a genuine lifecycle-check --all 9/9 on top of e2b5bba3e." → Merged `origin/main` (e2b5bba3e). Conflicts were ONLY in mechanically-generated files (testIds/state-matrix/gallery-coverage), resolved by regenerating from merged source; `ChatHistory.store.ts` (live4's +258) took live4's version untouched (this feature never edits the store), and `ConversationList.tsx` auto-merged (my virtualization edits + live4's mount-comment). Composition verified: live4 split the sidebar onto a SEPARATE `recentConversations` cursor, leaving `conversations`/`hasMore`/`total`/`loadNextPage` as the `/chats` list the virtualizer windows — no duplicate paging, no shared-scroll-container conflict; a blind merge-audit + real-path e2e (Load-More pages into `conversations`, virtualization windows it) confirm it. Re-ran phases 6-8 on the merged base (unit 20/20, visual 4/4, real-path+regression 5/5); one post-merge MEDIUM (seed built at fallback width) fixed in FIX_ROUND-4. [generalizable: yes — when two concurrent features touch the same store, whichever composes as a PURE consumer (no store edit) should take the other's store version wholesale and verify field-level composition, rather than hand-merging store logic]

_No human UX-review feedback on the running feature yet — awaiting review. The
feature is functionally complete + gated (8/8); phase 9 reaches 9/9 once the human
reviews the live `/chats` list and any feedback is resolved._
