# FIX_ROUND-4 — re-audit round 4

Two fresh blind reviewers over the round-3-fixed diff → 2 new confirmed, fixed:

- FIX-17 (state-management, medium): `clearDraft` unconditionally also wiped the
  shared `new` bucket for any non-`new` key, so sending in an existing
  conversation destroyed a separately-authored new-chat draft. Fixed by
  capturing the draft key at send START (`beforeSendMessage`, before a new-chat
  conversation is created) and clearing exactly that one key; `clearDraft` now
  clears only the key given (dead TextStore clearDraft machinery removed).
- FIX-18 (a11y, medium): the global Cmd/Ctrl-F handler swallowed native find even
  during the Loading / ErrorState early-returns where the find bar isn't
  rendered. Now no-ops unless a conversation is loaded.

Compiles clean (ui tsc; 13 unit tests pass).

**New confirmed findings:** 2
