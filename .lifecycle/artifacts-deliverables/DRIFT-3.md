# DRIFT-3 — final reconciliation: v1 scope vs dependency-blocked fast-follow

Round 3 reconciles the plan to what shipped after the editor toolbar + CSV grid were
built and browser-verified (they are NO LONGER deferred — see DRIFT-2). This round moves
FOUR items out of v1 into a documented fast-follow, each blocked by a genuine external
dependency this environment cannot satisfy — NOT corner-cutting renderable UI.

## Amended: deferred to fast-follow (v1.1) — removed from v1 `## Items`

- **ITEM-9** — verdict: impl-wins — **auto-open** requires the chat **streaming-position
  signal** (open the canvas ONLY for the just-arrived `create_file` result, never for
  every historical result replayed on conversation load — which would yank the panel open
  on every reload). That signal isn't wired yet; the manual "Open in side panel" affordance
  ships in v1. Deferred until the stream-position hook exists.
- **ITEM-15** — verdict: impl-wins — **selection → ask** (quote a selection into the
  composer, model answers). The value is the **model interaction**, which needs a real LLM;
  this box carries placeholder API keys (the one legitimate real-LLM gate,
  [[feedback_no_ignore_unless_platform]]). The popover shell without a working model reply
  is a hollow demo. Deferred with the LLM dependency.
- **ITEM-16** — verdict: impl-wins — **selection → edit**. The edit-shaping LOGIC is built
  + unit-tested (`selectionEdit.test.ts`, unique-`old_str` gating), but the targeted edit
  RESULT is produced by the model (`files_mcp::edit_file`) — same real-LLM gate as ITEM-15.
  Deferred with the LLM dependency; the logic stays covered by its unit test.
- **ITEM-21** — verdict: impl-wins — **image paste/embed** in the WYSIWYG is not
  implemented in v1 (the markdown/code/CSV editors + toolbar were the committed editor
  scope). Deferred as unimplemented; no partial/fragile UI shipped.

## Kept in v1 (built + verified this round — see FIX_ROUND-2)

ITEM-1,2,3,4,5,6,7,8,10,11,12,13,14,17,18,19,20,22,23. The concurrent-edit banner
(ITEM-14) + per-file dirty guard (ITEM-13) are BUILT and now carry a deterministic e2e
(`concurrent-edit.spec.ts` — a second REST client advances the head, the banner appears,
Keep-mine preserves both versions).

## Note

PLAN.md's ITEM-17 text says migration `132`; the shipped migration is `136` (originally
`133`, renumbered to `136` when merging current main, whose migrations advanced to `135`
(js_tool). Cosmetic doc lag; the code + regen use the on-disk filename ordering.

**Unresolved drifts:** 0
