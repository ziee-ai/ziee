# FIX_ROUND-5 — re-audit round 5

Two fresh blind reviewers → 2 new confirmed, fixed:

- FIX-19 (state-management, medium): a REGENERATE send sets
  `pendingBranchFromMessageId` but not `editingMessage`, so onMessageSent
  preserved the draft + cleared the composer, yet the TextInput restore effect
  never re-fired — the draft was stranded. Fixed by restoring the draft directly
  in onMessageSent for ANY branch (edit/regen) send.
- FIX-20 (i18n, low): stray non-ASCII 'þrowing' → 'throwing' typo in a comment.

Compiles clean.

**New confirmed findings:** 2
