# DECISIONS — tool-argument-contracts

Every decision the implementation needs, resolved before code. No TBDs.

### DEC-1: Accept `spec.kind` as a fallback, reject unknown `spec` keys, or both?
**Resolution:** BOTH.
**Basis:** convention — the two halves fix different halves of the defect and
neither alone is sufficient. The fallback alone leaves `{"spec":{"cmd":"ls"}}`
(a *typo*, not a misplacement) still failing with the wrong message. Unknown-key
rejection alone would turn today's silent-wrong-thing case
(`{"spec":{"kind":"sandbox_exec","task":"…"}}`) into a hard refusal of a call
whose intent is completely unambiguous — the model said what it wanted, in a
place the schema invited, and refusing it teaches nothing that honouring it
doesn't. Honour what was said; refuse what was never advertised.

### DEC-2: When top-level `kind` and `spec.kind` disagree, which wins?
**Resolution:** Neither — refuse with `BACKGROUND_KIND_CONFLICT`, naming both
values and both locations.
**Basis:** convention — `common/tool_args.rs`: "A supplied-but-undecodable
argument is never turned into the default — that would hide the mistake instead
of correcting it." A contradiction is not a decodable value. Silently preferring
either side reintroduces the exact class this branch exists to remove: the
server quietly resolving an ambiguity the model did not intend to create. An
AGREEING pair (same value in both places) is not a contradiction and is accepted.

### DEC-3: Is the unknown-`spec`-key check the UNION of both kinds' keys, or per-kind?
**Resolution:** the UNION — `{kind, task, system, command, flavor}`. A key
outside the union is refused; a key inside the union but belonging to the OTHER
kind is NOT refused by this check, and is instead diagnosed by DEC-4's targeted
hint on the missing-required-field message.
**Basis:** convention + the plan audit's ITEM-3 CONCERN. Per-kind strictness
would refuse `{"kind":"sandbox_exec","spec":{"command":"…","task":"describe the
output"}}`, where the model's intent is clear and the extra field is harmless.
The union still catches every typo (`cmd`, `prompt`, `instruction`, `script`),
which is the population the check exists for, and the cross-kind case gets a
BETTER diagnosis from the hint than a blunt "unknown key" would give.

### DEC-4: What does a missing required field say when the OTHER kind's field is present?
**Resolution:** it names the real mistake. `kind: subagent` with `spec.command`
present and `task` absent says that `command` belongs to `kind: 'sandbox_exec'`
and that `kind` is a top-level sibling of `spec` — in addition to (not instead
of) the actionable three elements INV-1 requires.
**Basis:** the design's §1 and the reported symptom. `BACKGROUND_TASK_REQUIRED`
telling a model to add `task` when it deliberately sent `command` is the
"blaming the wrong field" half of the defect; a refusal that does not name the
real mistake is not a fix.

### DEC-5: Advertise `"additionalProperties": false` on the `spec` schema?
**Resolution:** yes, on `spec` — paired with DEC-3's server-side check, and with
`kind` added to `spec`'s declared `properties` (documented as "the same value as
the top-level `kind`; supplying it here is accepted").
**Basis:** the design's rule — the advertisement and the enforcement must be the
same statement. Declaring `additionalProperties: false` while accepting anything
is the defect in miniature; enforcing it while advertising an open object is the
defect inverted. The top-level `kind` is deliberately left as the documented
canonical location, with the nested one accepted rather than promoted.

### DEC-6: Where does the canonical flavor allow-list check live?
**Resolution:** `src-app/server/src/modules/code_sandbox/mod.rs`, as
`is_known_flavor` / `known_flavor_names` / `validate_flavor`.
**Basis:** codebase — `KNOWN_FLAVORS` itself lives in the `sdk/` submodule
(`ziee-sandbox/src/types.rs`) and is re-exported as
`crate::modules::code_sandbox::types::KNOWN_FLAVORS`. Putting the helper beside
the const would be marginally better but requires a submodule commit this branch
must not push (it would leave the ziee commit pointing at an unreachable sha).
`code_sandbox/mod.rs` is the module's existing public face, is where all four
current consumers already resolve the const through, and is an existing file — so
the diff adds no new module seam.

### DEC-7: Refactor the two pre-existing ad-hoc flavor scans to use it?
**Resolution:** yes, preserving each site's error code and HTTP status exactly
(`INVALID_FLAVOR`/400 in `mcp/handlers/mod.rs`, `MCP_UNKNOWN_FLAVOR`/422 in
`mcp/user_policy/repository.rs`). Only the PREDICATE is shared; each site keeps
building its own `AppError`.
**Basis:** CODING_GUIDELINES §9 — a third independent copy of the same allow-list
scan is how the next one drifts. Sharing only the predicate is what lets both
error contracts stay byte-identical, which TEST-16 and the pre-existing
`create_system_server_persists_and_validates_sandbox_flavor` pin.

### DEC-8: Also validate the workflow definition's `sandbox.flavor`?
**Resolution:** NO — out of scope, reported to the caller as an observation
rather than silently fixed or silently ignored.
**Basis:** the design's §"Scope boundary". `workflow_def.sandbox.flavor` reaches
the same URL sink and is equally unvalidated (`grep -n flavor
workflow/validate.rs` → nothing), but it is a different trust boundary: workflow
YAML is authored configuration, a sibling of the admin install path whose
safe-token check deliberately permits a flavor published after this binary was
built. Enforcing `KNOWN_FLAVORS` there would remove that escape hatch for
workflows without anyone having asked for it. The defect being fixed is
specifically "an `enum` advertised **in a tool schema to a model** and not
enforced"; the workflow YAML advertises no such enum.

### DEC-9: Fixed constant or admin-configurable settings row for the flavor allow-list?
**Resolution:** fixed constant — `KNOWN_FLAVORS`, unchanged.
**Basis:** it is not a new operational tunable; it is the SAME list the tool
schema already advertises, so making it configurable would let an admin desync
the advertisement from the enforcement — reintroducing this branch's defect by
configuration. The genuine "install a flavor this binary has not heard of" need
is already served by the admin install path's safe-token check
(`version_handlers.rs`), which this branch does not touch. Structured as named
functions rather than inline scans (DEC-6), so it can be promoted later without
a rewrite.

### DEC-10: Error codes for the new refusals.
**Resolution:** reuse the module's existing `BACKGROUND_*` prefix and its
`AppError::bad_request` shape:
`BACKGROUND_KIND_UNKNOWN` (kept, message upgraded),
`BACKGROUND_KIND_CONFLICT` (new),
`BACKGROUND_SPEC_UNKNOWN_FIELD` (new),
`BACKGROUND_TASK_REQUIRED` / `BACKGROUND_COMMAND_REQUIRED` (kept, messages
upgraded), and `SANDBOX_UNKNOWN_FLAVOR` for the two flavor entry points.
**Basis:** codebase — every existing refusal in `background_mcp/tools.rs` uses
`AppError::bad_request("BACKGROUND_*", …)`; the sandbox module's own sibling code
is `INVALID_FLAVOR`, which is already taken by the MCP-server-create path with a
different payload shape, so the tool entry points get a distinct
`SANDBOX_UNKNOWN_FLAVOR` rather than overloading it.

### DEC-11: Is `spec.kind` stripped before the spec is persisted as `inputs_json`?
**Resolution:** yes — consumed and removed, so the stored spec contains only the
vocabulary the schema declares.
**Basis:** convention. A consumed argument left in the payload is a shadow copy
a later reader can resolve differently from the one the spawn used — the
stale-snapshot class. Verified safe: nothing reads a background run's
`inputs_json` back (see PLAN's *Breakage risk*), and before this branch a nested
`spec.kind` was already being persisted verbatim, so this strictly narrows what
is stored.

### DEC-12: Should the chat-path flavor check run before or after `build_context`?
**Resolution:** after — left exactly where the `flavor` argument is read, inside
the `execute_command` branch, immediately before `execute_command_stream`.
**Basis:** codebase — moving a pure argument check above `build_context` would
change which error a caller sees when BOTH the conversation context is broken and
the flavor is bad, for no benefit; the existing ordering (ownership → context →
per-tool argument handling) is the module's established shape. The check is still
strictly upstream of every path to `install_version`, which is what INV-3
requires.
