# Fix Round 1 — knowledge-base-retrieval

The phase-6 blind multi-angle audit (self-conducted across all 12 angles; the
fresh-subagent fan-out was blocked mid-run by an API session limit) surfaced
**3 confirmed findings**. All three were fixed in commit `9ba1d3173`:

| Angle | Sev | Finding | Fix |
|---|---|---|---|
| state-management | HIGH | KB composer had no conversation-change reset → pending `selectedKbIds` leaked from an existing conversation into a fresh new chat and attached on first send | `chat-extension/extension.tsx` now subscribes to `useChatStore` `conversation?.id`; clears the buffer via `setCurrentConversation(null)` on a new (unsaved) chat, mirroring the file extension |
| error-handling | MEDIUM | `attach`/`detach` fired with `void` → rejected promise was a silent unhandled rejection, no user feedback | `KbMenuItem.toggle` and `KbStatusRow.onClose` now `.catch(...)` → `message.error` |
| correctness | LOW | `setHighlights` did not clamp the target page → a stale/off-by-one citation page left pdf.js unmoved and the overlay unrendered | `pdfjs.ts::setHighlights` clamps to `[1, pagesCount]` |

## Re-audit (round 1)

After the fixes, the touched hunks were re-reviewed from the same angles that
raised each finding, plus a sweep of the surrounding call sites:

- **state-management** — the new subscription fires `setCurrentConversation(null)`
  only when the id becomes falsy; a change to a real id is still handled by
  `onConversationLoad` (re-hydrate). No double-load, no leak. `useChatStore.subscribe`
  is an idempotent module-level registration in `initialize` (runs once). Clean.
- **error-handling** — both call sites now surface failures; the store's `set`
  after `await` still only runs on success (state stays consistent). Clean.
- **correctness** — clamp uses `pagesCount || 1` and `Math.floor(page)||1`; the
  old-page layer removal keys off the clamped value. Clean.
- `npm run check (ui)` and `npm run check (desktop/ui)` both green after the fixes.

**New confirmed findings:** 0
