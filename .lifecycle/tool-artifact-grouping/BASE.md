# BASE — conflict-surface scoping

Branch base: `origin/khoi` (tip `72cbfeee`, which already includes the merged
`feat/resource-link-ssrf` PR #131 — the server-side artifact-ingest/SSRF change).
PR target: `khoi`. Gate base ref: `origin/khoi`.

## What current base touches that this branch also touches
- **This is a UI-only change** (`src-app/ui/**`). No backend files, no
  migrations, no OpenAPI types.
- **Highest existing migration:** `00000000000153_scheduled_task_unattended_tools.sql`.
  This branch adds **no migration** → no migration-number collision possible.
- **No `openapi.json` / `api-client/types.ts` regen** — no backend type change.
- **Desktop UI:** `src-app/desktop/ui` has **no hand-written override** of the
  touched files; it aliases `../../ui/src` via `fallbackSrc` in its
  `vite.config.ts`. So editing `src-app/ui/src/...` covers desktop too; **no
  desktop mirror edit needed** and R2-3 (desktop-override security diff) does not
  apply to these files. Only the `ui` workspace is a touched frontend workspace.

## Files this branch will add/edit (all under `src-app/ui/src/`)
- ADD `modules/chat/core/utils/normalizeToolResultOrder.ts` (+ `.test.ts`)
- ADD `modules/mcp/chat-extension/toolRun.ts` (+ `.test.ts`)
- EDIT `modules/chat/components/ChatMessage.tsx`
- EDIT `modules/mcp/chat-extension/extension.tsx`
- ADD an e2e spec + (if needed) a gallery state under `src/dev/gallery/` for the
  tool-group render states (finalized in TESTS.md / phase 3).

## Coordination
- `resource-link-ssrf` worker: **DONE and merged into base** (PR #131). Its change
  is server-side artifact ingest/SSRF trust policy; this branch only changes how an
  already-produced artifact block is grouped/expanded in the UI. No overlap.
- No other active worker is known to touch `chat/components/ChatMessage.tsx` or
  `mcp/chat-extension/extension.tsx`.
