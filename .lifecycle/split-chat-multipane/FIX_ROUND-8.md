# FIX_ROUND-8 — convergence

A blind re-audit (`split-chat-fixround8-audit`) of the FIX_ROUND-7 root-cause fix
(the per-init `destroyed` guard on the Chat.store async tail + the unconditional
`chatStreamClient` null-set) across correctness / concurrency / state-management
returned ZERO findings on every angle.

The fix-loop has converged:

| Round | New confirmed findings |
|---|---|
| FIX_ROUND-3 | 9  (SplitChatView tabs-on-desktop `!md`; File.store owner backup; 4 broken specs) |
| FIX_ROUND-4 | 5  (async-hook focus-race data-loss; SplitChatView remount; spec false-greens) |
| FIX_ROUND-5 | 4  (singleton streaming-dead guard; 4th onStreamError; duplicate-testid boot blocker; docstrings) |
| FIX_ROUND-6 | 2  (StrictMode double-client; test title) |
| FIX_ROUND-7 | 1  (StrictMode dropped-teardown — root-caused) |
| **FIX_ROUND-8** | **0** |

Every confirmed finding across all rounds is fixed; every rejected item carries a
recorded rationale (Chat.store re-init guard false-positive in R3 — later found to
be a REAL singleton bug in R5 and fixed; the ctx-less-hook focus resolution
superseded by owning-paneId threading, DRIFT-2.13). Coverage law satisfied
(`AUDIT_COVERAGE.tsv` regenerated each round). The three flagship risks the
directive named are all closed and tested: the wrong-pane tool-approval routing
(conversation-keyed `approvalRouting` + owning-paneId async hooks;
`approvalRouting.test.ts` asserts approval for conv A is invisible to B), the
same-conversation streaming double-apply (per-instance `onFrame` filter), and the
File.store per-pane composer data-loss (per-pane `backupByPane`).

**New confirmed findings:** 0
