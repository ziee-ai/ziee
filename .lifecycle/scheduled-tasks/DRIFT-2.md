# DRIFT-2 — Round 2 (Follow-up & Series) implementation vs plan

Audit of the shipped ITEM-40..48 implementation against PLAN.md / DECISIONS.md / the UX design.

- **DRIFT-2.1** — verdict: impl-wins — PLAN/PLAN_AUDIT described the run-row expand as
  "the fuller result inline (up to the notification cap ~800)". The implementation stores
  only the DEC-20 `result_preview` (~280 chars) on the run, so expand reveals the FULL
  (un-clamped) 280-char preview + the failure message + skipped-tools detail — NOT a
  separate 800-char body or a live fetch. Coherent: the FULL result is reached via "Open
  thread" / "Continue in chat" (which pull the real assistant text / workflow output
  live). The "~800" was aspirational; a second larger column / fetch-on-expand was never
  an ITEM. Plan understanding amended here; no code change owed.

- **DRIFT-2.2** — verdict: impl-wins — DEC-23 (chosen) was "synthesized assistant turn …
  then the user replies", with no leading user message. The implementation seeds a SHORT
  NEUTRAL user framing turn ("What did the latest scheduled run of \"X\" produce?") BEFORE
  the assistant turn carrying the real result. Reason: a conversation whose history begins
  with an assistant turn is INVALID for some providers (Anthropic requires the first
  message to be `user`), so a bare assistant-first seed would break the user's first
  reply. The framing turn is a neutral QUESTION — it does NOT embed the result (that stays
  in the assistant turn), so DEC-23's intent is preserved and this does NOT reintroduce
  the rejected "user-message-embed" option. Surfaced to the human in the session report
  per the audit-vs-user-decision rule; not a silent reversal.

- **DRIFT-2.3** — verdict: resolved — `ListPagination` requires an `onPageSizeChange`
  handler, but DEC-20 fixes the runs page size at 10; the handler reloads page 1 at the
  fixed size rather than changing it. The size selector the component renders is
  effectively inert — acceptable for now (a follow-up could suppress it). No paging bug:
  page-number navigation works correctly.

- **DRIFT-2.4** — verdict: none — task-level "Open thread" (task header) AND per-run "Open
  thread" both shipped, MATCHING the UX design (S2 header + S4 row); not a divergence.

- **DRIFT-2.5** — verdict: none — the series seed (`build_series_seed`) folds only run
  previews + deltas (text), no artifacts, exactly as ITEM-43 specified (artifacts attach
  only on the single-run seed, ITEM-42).

**Unresolved drifts:** 0
