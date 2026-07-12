# STATUS — fix: mid-stream flicker + false "empty response" notice

Branch `fix/streaming-empty-notice-flicker` → **PR #139** (base `khoi`) — https://github.com/ziee-ai/ziee/pull/139

## DONE — lifecycle 9/9, merged up to current khoi, PR updated.

### Two root causes (both streaming→persisted handoff)
1. **Single-turn gap** — `complete` handler deleted the streaming assistant row + flipped
   `isStreaming:false`, then re-merged the persisted row only after an awaited `getHistory`.
   Fix: keep the row; swap streaming→persisted in ONE `set()` (`finalizeTailWindow`).
2. **Tool-approval RESUME overwrite** (found by human review; the single-turn fix missed it) —
   each approval re-calls `sendMessage()` and the backend continues the SAME `assistant_message_id`;
   the content handler blanked that row with a `contents: []` placeholder → `ChatMessage` returns
   `null` on zero blocks → bubble vanished. Fix: `resumeOrFreshPlaceholder` reuses the existing row.
Plus a defensive `finalizingTurn` gate on the empty-completion notice.

### Verification
- Unit 22/22; e2e 5 passed (incl. the new handoff spec, **proven to FAIL on base**, + the #135
  approval-scroll regression). `npm run check` green. `gate:ui` 168/173 (5 failures pre-existing /
  khoi-owned, none in this diff).
- **Live** (real gpt-oss + tool approvals, current-khoi backend): before → bubble disappears after an
  approval; after → 1 and 3-approval runs show **no disappear, no notice**.

### Notes
- Merged current `khoi`, which includes `#137` (tool_use/tool_result pairing) + `#138` (stale-artifact).
  Those resolve the multi-tool **empty-completion notice** on the backend; this PR fixes the frontend
  flicker/disappear.
- All my test instances/containers are torn down. The user's live ziee on :8080 is untouched (healthy).
