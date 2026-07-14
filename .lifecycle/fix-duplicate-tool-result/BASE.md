# BASE — fix-duplicate-tool-result

Conflict surface between this branch and current base at plan time.

- **Base branch**: `khoi` @ `c66cd5d76` (per the task brief: branch from `khoi`, PR to `khoi`,
  NOT `main`). Worktree: `/data/khoi/home-workspace/ziee/ziee-worktrees/dup-toolresult-wt`.
- **Highest existing migration**: `00000000000157_remove_unused_builtin_mcp_servers.sql`.
  This branch adds **158** — no collision at plan time. Re-check at merge (the merge-gate's C2
  does this against real base).
- **OpenAPI regen implied?** **No.** The diff adds no request/response type, no route, no enum
  variant, no permission. `openapi.json` / `api-client/types.ts` are untouched in BOTH `ui/` and
  `desktop/ui/`. (Verified: the changed fns are internal assembly helpers with no serde-exposed
  surface.)
- **Frontend touched?** **No.** Backend-only (`src-app/server/**`). The phase-3 e2e gate and the
  phase-8 `npm run check` / `gate:ui` chain therefore do not apply.
- **New permission?** **No** — A9/A10 do not apply.

## Files other live workers are changing

Checked every active worktree/branch for overlap on my files:

| Worker / branch | Files | Collides with me? |
|---|---|---|
| `fix/mcp-tool-title-generation` (`wt-title`) | `chat/core/extension/registry.rs`, `tests/mcp/tool_call_history_test.rs` | **No** — 0 hits on `streaming.rs` / `chat_extension/mcp.rs` (verified by `git diff --name-only khoi...fix/mcp-tool-title-generation`) |
| `split-deploy-web`, `mcp-delete-perm`, `files-read-file` | deploy/compose, mcp perms, files_mcp | **No** overlap on my four files |

## Prior art on the same code (must not regress)

`streaming.rs`'s pairing logic was rewritten by the sibling `chat-toolresult-pairing` feature —
commits `b75a12ebe` (`always emit valid tool_use/tool_result pairing to providers`), `51b5928a8`
(blind-audit hardening), `95c02d72b` (dedup test defers the flush). My ITEM-1/2/3 sit directly on
that code:

- Its `group_assistant_blocks_*` unit tests (`streaming.rs:2259-2423`) and
  `tests/chat/assistant_block_grouping_test.rs` must stay green.
- The atomic `MAX+1` `sequence_order` fix in `contents.rs` (parallel-tool ordering) must not
  regress — ITEM-5 touches its COMMENT only, no code.
- ITEM-3 changes `flush_assistant_tool_pair` behavior; its TEST-8
  (`group_assistant_blocks_dedups_duplicate_result`) is the guard.
