# Mutation checks — every fix reverted, confirmed RED

The rule this repo has paid for twice: a green test proves nothing until you have
watched it go red. Each fix below was reverted in place, the covering tests run,
and the tree restored.

## Round 0 — the original fixes

| # | mutation | result |
|---|---|---|
| M1 | `kind` read from the TOP LEVEL only (the nested-`kind` fallback removed) | **RED** — 5 failed |
| M2 | unknown-`spec`-key check disabled (silently ignore, as before) | **RED** — 2 failed |
| M3 | `missing_spec_field` restored to the bare `"spec.task must be a non-empty string"` | **RED** — 2 failed |
| M4 | background `spec.flavor` enum check removed | **RED** — 2 failed |
| M5 | chat-path `flavor` enum check removed | **RED** — 1 failed |
| M6 | `is_known_flavor` accepts anything | **RED** — 2 failed |
| — | restored | **GREEN** — 49 passed |

The integration tests needed no synthetic mutation: **unmodified `origin/main` IS
the mutation**, and all four `spawn_contract` tests were observed RED against it
before a line was changed (`tool-argument-contracts-REPRO-red-KEEP.log`).

## Round 1 — the audit fixes

| # | mutation | result |
|---|---|---|
| M7 | per-kind `spec` key rule reverted to the UNION (cross-kind option silently ignored) | **RED** — 1 failed |
| M8b | empty/whitespace `kind` treated as a value again (false CONFLICT) | **RED** — 1 failed |
| M9b | cross-kind hint back to `is_some()` (fires on explicit `null`) | **RED** — 1 failed |
| M10 | a `KindContract.example` given a `"label"` key no schema declares | **RED** — 1 failed |
| M11 | `DEFAULT_TOOL_FLAVOR` renamed out of the catalog | **RED** — 1 failed |
| M12 | the schema's `kind` enum drifted from `KIND_CONTRACTS` | **RED** — 1 failed |
| M13 | `truncate_for_message` reduced to `s.to_string()` | **RED** — 1 failed |
| M14 | `invoke_tool`'s arm skips the enum again | **see below** |
| — | restored | **GREEN** — 67 passed |

### Two mutations that did NOT go red on the first attempt — and what that found

**M8 was a no-op.** The patch targeted the rustfmt'd multi-line form of the arm;
the file has the single-line form, so nothing was mutated and the "green" result
was meaningless. Re-run as **M8b** against the real text: RED. A mutation check
that silently fails to mutate is indistinguishable from a passing one, so each
round-1 mutation now asserts its target string is present before editing.

**M9 was a real coverage gap.** The fix (the cross-kind hint must use the same
non-empty-string predicate the reader uses) was correct, but *nothing tested it*:
the existing case used a spec with no `command` key at all, which behaves
identically under both predicates. The explicit-`null` / wrong-type / empty-string
cases — the entire reason for the fix — had no test. Added, and **M9b** is now
RED. This is the failure mode the whole exercise is about, met on my own work.

### M14 — an honest limit

`invoke_tool`'s `execute_command` arm is unreachable at runtime: `jsonrpc_handler`
intercepts that tool name earlier and answers with the streaming branch. So no
test can drive it, and mutating it only shows that it still compiles (it does).
That unreachability is exactly *why* it was a latent bypass — dead code carrying
its own `"minimal"` default and no enum check, one routing refactor from being
live. The fix removes the bypass; it is verified by inspection and by the compiler,
**not** by a test, and it is recorded that way rather than counted as covered.
