# FIX_ROUND-2

Fixed the single `confirmed` finding from round 1, then re-ran a blind round.

## Confirmed finding fixed (from FIX_ROUND-1's re-audit)

- **Nondeterministic collapsed-row size (correctness, low).**
  `conversation_model_authored_files` now restores the deterministic order of the
  `ids` (which `model_authored_file_ids` returns `ORDER BY created_at, id`) after
  `get_by_ids_and_user` (whose `WHERE id = ANY(...)` does not preserve order),
  via a `HashMap<Uuid, usize>` rank + stable `sort_by_key`. So a collapsed
  same-name `list_files` row now advertises a stable `size` every call, and a
  same-name read pick is deterministic. Verified: every returned record's id is a
  member of `ids` (get_by_ids_and_user filters to them), so the `usize::MAX`
  fallback is unreachable defensive code — no panic/overflow.

## Blind round 2 result

One fresh blind auditor applied all angles (correctness, security, error-handling,
concurrency, perms/authz, api-contract, state-management, patterns-conformance,
performance, i18n/copy) over the complete diff. It confirmed the determinism fix is
correct (unreachable fallback, stable sort, both goals met) and found **no**
correctness/security/concurrency/authz/panic defect.

It raised ONE `api-contract` observation, **rejected as accepted-design (not a
defect)**:

- *"list_files collapses two same-named artifacts to one row, but read_file
  returns AMBIGUOUS_FILENAME for that name."* — This is the intended behavior, not
  a bug: showing one row SURFACES the file's existence, and `read_file`'s
  AMBIGUOUS error is ACTIONABLE (it tells the model to read a specific one by id
  via the files read_file tool). The alternative — hiding the name entirely — would
  make tool-produced files invisible to the model, which is worse. This mirrors
  `read_file`'s own ambiguity philosophy (surface + guide, never silently drop),
  and the degenerate case (two tool artifacts with byte-distinct content but an
  identical filename in one conversation) is rare. No code change.

No new finding requires a fix.

**New confirmed findings:** 0
