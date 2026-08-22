# FIX_ROUND-1

Two blind angles ran on `git diff origin/main...HEAD` (diff-only context, no
planning docs): **correctness + concurrency/resource** and **tests-quality +
api-friendliness/maintainability**. 25 rows in `LEDGER.jsonl`; 24 confirmed, 1
rejected with rationale.

## The two that mattered most

**A test of mine could have started a ~900 MB download.** The correctness angle
found that `tier3_flavor_contract`'s "happy-path counterpart" fired a real
`execute_command` with a VALID flavor at a sandbox-enabled, rootfs-less server —
and `execute_command_stream` `tokio::spawn`s its work *before* responding, with
the harness writing `require_download_consent: false`, so the spawned task
proceeds to `api.github.com` and `install_version`. Dropping the HTTP response
does not cancel it. A test whose header says it proves the refusal lands *before*
any network call was itself the thing reaching the network. The tests-quality
angle independently flagged the same block as a vacuous control (it `continue`s
on SSE and otherwise only greps for a phrase lifted from the implementation).

Fixed by never invoking `execute_command` with an accepted flavor in that file.
The accepted case is proven exhaustively and for free by the pure-function unit
test; what belongs at the HTTP tier is the wiring plus a control that executes
nothing — now a `read_file` call traversing the identical JWT → permission →
`x-conversation-id` → ownership → `build_context` → `dispatch` chain.

**The "one canonical check" had a second door.** `invoke_tool`'s `execute_command`
arm deserialized `flavor` with its own `default_flavor()` and never consulted the
allow-list. It is shadowed today only because `jsonrpc_handler` intercepts the
name earlier — i.e. dead code that skips a security check, one routing refactor
from being live. Both angles found it. It now resolves through the same
`resolve_execute_flavor`.

## Everything fixed this round

| # | fix |
|---|---|
| 1 | tier3 test no longer reaches the network; real positive control added |
| 2 | tier3's bwrap skip is no longer `#[cfg(linux)]`-only (it compiled OUT on macOS/Windows, turning a skip into a panic) |
| 3 | `invoke_tool`'s second `execute_command` arm routed through the shared resolver |
| 4 | `resolve_spec_flavor` / `resolve_execute_flavor` collapsed into one `code_sandbox::resolve_tool_flavor`; the `"minimal"` default went from three literals to one `DEFAULT_TOOL_FLAVOR` |
| 5 | `spec` keys are now validated PER KIND, not against the union — a cross-kind OPTIONAL field (`flavor` on a sub-agent spec) was accepted and silently ignored, and a test had certified that silence. The other kind's REQUIRED field is still let through, because it earns the misplaced-`kind` diagnosis |
| 6 | `KindContract` carries its `JobKind`; the dispatch matches on it and each spawner writes the row from it, so the arm chosen and the row written cannot disagree |
| 7 | an empty / whitespace `kind` is refused as an unknown kind — it was being reported as a "conflict" against a real sibling value, naming a contradiction that did not exist |
| 8 | `missing_spec_field`'s cross-kind hint uses the same non-empty-string predicate `require_spec_field` does (`is_some()` was true for explicit `null`, steering the model toward a kind whose field was also null) |
| 9 | one `unknown_kind_error` builder replaces three hand-maintained copies of the text |
| 10 | the truncated unknown-key list now says "(and N more)" — a model that fixed the named keys otherwise looped on the same refusal |
| 11 | `BACKGROUND_SPEC_REQUIRED` carries a full ARGUMENTS example; it was handing out a `spec`-level one for an error about a missing `spec` key |
| 12 | `truncate_for_message` now SANITISES as well as bounds — backticks and control characters in a model-supplied fragment closed the surrounding quoting in text the model reads next |
| 13 | the test-local `assert_actionable` was weaker than the shared one it claimed to match (it matched the marker `"Example: {"`, so a wrong/empty example passed). It now requires one of the module's REAL examples and parses it |
| 14 | new test: every example the module hands out round-trips through the parser — the `label` defect generalised |
| 15 | new test: `DEFAULT_TOOL_FLAVOR` is in the catalog. The absent/null arm returns it WITHOUT the allow-list, so a catalog rename would have made the most-travelled path build an unknown-flavor URL |
| 16 | new tests for the untested input classes: nested non-string `kind` (the `location` argument was never asserted), padded `kind`, a stringified `spec` carrying a nested `kind` (the reason decode runs first), >N unknown keys |
| 17 | new test: each contract's `job_kind` matches its advertised name |
| 18 | new tests for `truncate_for_message`: boundary, over-boundary, and multi-byte input (the mid-codepoint panic its own doc warns about was unverified) |
| 19 | TEST-17 was a tautology (expected value derived from the body of the function under test); it now pins the literal admin-facing string |
| 20 | the schema's `kind` enums (top-level AND nested) and `spec` properties are now pinned against `KIND_CONTRACTS`, so a third kind cannot be enforced-but-never-advertised |
| 21 | `run_count`'s "no row was created" assertion gained its positive control (the count must be seen to MOVE for accepted calls) and the helper closes its pool |
| 22 | the `INVALID_FLAVOR` assertion reads the `error_code` field instead of substring-matching the whole body |
| 23 | two doc comments corrected where they overclaimed: "strictly upstream of all of it" (true of the fetch, not of `build_context`) and the admin escape hatch (it survives for INSTALL, not for USE) |

## Rejected

One finding, recorded in the ledger with its reasoning: that the unknown-key
refusal breaks models which copied the old `label` example. `label` was never
implemented, so such a call was ALREADY silently dropping the field; a grace
period preserves exactly the silent-ignore this change removes and re-falsifies
the new `additionalProperties: false`. The stale-example vector is closed by
correcting the example, which this change does.

## Verification

- `cargo check -p ziee --tests` — 0 errors.
- unit: **67 passed, 0 failed** (was 62 before this round's new tests).
- integration `background_mcp::spawn_contract` + `code_sandbox::tier3_flavor_contract`
  + `mcp::run_in_sandbox_test` — **17 passed, 0 failed**.
- mutation re-check of the round's guards: see `MUTATION.md`.

**New confirmed findings:** 0
