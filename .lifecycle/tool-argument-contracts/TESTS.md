# TESTS — tool-argument-contracts

Every rejection test carries its **happy-path counterpart in the same test**: a
correctly-formed call must still work, and `minimal`/`full` must still be
accepted. That pairing is what keeps a refusal test from passing because the
whole path is broken.

Two of these are **reproductions written RED first** against unmodified
`origin/main` (TEST-11 and TEST-12), and each fix is mutation-checked by
reverting it and confirming its test returns to RED.

## Unit — `background_mcp/tools.rs` (`#[cfg(test)]`)

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/background_mcp/tools.rs` — asserts: `resolve_spawn_kind` returns `SandboxExec` for `{"spec":{"kind":"sandbox_exec","command":"…"}}` (nested-only kind is HONOURED, not dropped), AND — the happy-path counterpart in the same test — still returns `SubAgent` for `{"kind":"subagent","spec":{"task":"…"}}`, `SandboxExec` for a top-level `{"kind":"sandbox_exec",…}`, and the `SubAgent` default when no `kind` appears anywhere.
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/background_mcp/tools.rs` — asserts: a top-level `kind` and a `spec.kind` that DISAGREE are refused with `BACKGROUND_KIND_CONFLICT` whose message names BOTH values; an agreeing pair (same value in both places) is accepted and resolves to that kind.
- **TEST-3** (tier: unit) [covers: ITEM-1, ITEM-9] file: `src-app/server/src/modules/background_mcp/tools.rs` — asserts: after resolution the returned spec no longer carries `kind`, while every other supplied key survives byte-identically (so `inputs_json` matches the declared vocabulary); a spec that never carried `kind` is returned unchanged.
- **TEST-4** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/background_mcp/tools.rs` — asserts: `{"kind":"zee-workflow","spec":{…}}` is refused with a message that names `kind`, lists BOTH valid kinds, and carries a copyable literal-JSON example; the same message is produced when the bad value arrives nested in `spec.kind`.
- **TEST-5** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/background_mcp/tools.rs` — asserts: an unadvertised `spec` key (`{"task":"x","priority":"high"}`) is refused naming the offending key AND the accepted keys AND an example; and — the happy-path counterpart — every advertised key (`kind`,`task`,`system`,`command`,`flavor`) is accepted, and the advertised `spec` schema itself carries `"additionalProperties": false` so the enforcement matches the advertisement.
- **TEST-6** (tier: unit) [acceptance] [invariant: INV-1] [covers: ITEM-2, ITEM-3, ITEM-4] file: `src-app/server/src/modules/background_mcp/tools.rs` — asserts: EVERY refusal `spawn_background` argument-parsing can emit (unknown kind, kind conflict, unknown spec key, missing task, missing command, absent spec) is held to the three-element standard of `common::tool_args::conformance::assert_actionable` — it names the argument, says what is expected, and carries the literal-JSON example the model can copy. Would fail today: `BACKGROUND_TASK_REQUIRED`'s `"spec.task must be a non-empty string"` carries no example and no expected-shape wording.
- **TEST-7** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/background_mcp/tools.rs` — asserts: for `kind: subagent` with `spec.command` present and `task` absent, the refusal names the REAL mistake (that `command` belongs to `kind: 'sandbox_exec'`) rather than only demanding `task`; symmetrically for `kind: sandbox_exec` with `task` present and `command` absent; and a spec with neither cross-kind field still gets the plain actionable message.
- **TEST-8** (tier: unit) [covers: ITEM-6] file: `src-app/server/src/modules/background_mcp/tools.rs` — asserts: `spawn_background{kind:'sandbox_exec'}` refuses `spec.flavor: "zee-workflow"` naming the valid flavors, AND — happy-path counterpart — accepts `"minimal"`, `"full"`, and an absent `flavor` (which resolves to the `minimal` default), all before any run row could be created.

## Unit — `code_sandbox`

- **TEST-9** (tier: unit) [acceptance] [invariant: INV-3] [covers: ITEM-5] file: `src-app/server/src/modules/code_sandbox/mod.rs` — asserts: the canonical allow-list check accepts EXACTLY the flavors `KNOWN_FLAVORS` advertises (derived from the const, not a hardcoded literal, so a future flavor is covered automatically) and refuses everything else — the invented `"zee-workflow"`, an empty string, a case variant, and a path-traversal-shaped `"../../etc"`; and its refusal message enumerates the valid flavors.
- **TEST-10** (tier: unit) [acceptance] [invariant: INV-3] [covers: ITEM-7] file: `src-app/server/src/modules/code_sandbox/handlers.rs` — asserts: the `execute_command` flavor resolver returns `minimal` for absent/null `flavor` (unchanged default), returns `minimal`/`full` verbatim when supplied, and REFUSES `"zee-workflow"` — proving the advertised enum is enforced on the chat path before `execute_command_stream`, which is the only route to `install_version`'s URL construction.

## Integration — `tests/background_mcp/spawn_contract.rs` (new)

- **TEST-11** (tier: integration) [covers: ITEM-1, ITEM-4] file: `src-app/server/tests/background_mcp/spawn_contract.rs` — asserts: the VERBATIM reported repro `{"spec":{"kind":"sandbox_exec","command":"python hello.py"}}` posted to the real `/api/background/mcp` no longer answers `spec.task must be a non-empty string`; the refusal instead concerns the sandbox path it actually asked for. Happy-path counterpart in the same test: `{"kind":"subagent","spec":{"task":"…"}}` on the same server still returns a `run_id`.
- **TEST-12** (tier: integration) [acceptance] [invariant: INV-2] [covers: ITEM-1] file: `src-app/server/tests/background_mcp/spawn_contract.rs` — asserts: the SILENT-WRONG-THING case `{"spec":{"kind":"sandbox_exec","task":"…"}}` no longer succeeds as a sub-agent run. On `origin/main` this returns `structuredContent.kind == "subagent"` with no error; after the fix it must NOT produce a `subagent` run — the supplied `kind` is honoured and the call is refused for the missing `command`. Happy-path counterpart: an explicit `{"kind":"subagent","spec":{"task":"…"}}` on the same server DOES still produce a `subagent` run, which is the control proving the refusal is about the misplaced kind and not about sub-agents being broken.
- **TEST-13** (tier: integration) [acceptance] [invariant: INV-3] [covers: ITEM-6] file: `src-app/server/tests/background_mcp/spawn_contract.rs` — asserts: `{"kind":"sandbox_exec","spec":{"command":"echo hi","flavor":"zee-workflow"}}` over real HTTP is refused with a message naming the valid flavors and NO `workflow_runs` row is created (the DB row count for the user is unchanged), so no URL is ever constructed. Happy-path counterpart: the same call with `flavor:"minimal"` gets past flavor validation (it is not refused for the flavor).
- **TEST-14** (tier: integration) [covers: ITEM-3, ITEM-2] file: `src-app/server/tests/background_mcp/spawn_contract.rs` — asserts: over real HTTP, an unadvertised `spec` key is refused naming the key and the accepted keys, and an unknown `kind` value is refused listing the valid kinds; a spec using only advertised keys still spawns.

## Integration — `tests/code_sandbox/tier3_flavor_contract.rs` (new)

- **TEST-15** (tier: integration) [covers: ITEM-7] file: `src-app/server/tests/code_sandbox/tier3_flavor_contract.rs` — asserts: on a sandbox-ENABLED test server, a real JSON-RPC `tools/call execute_command` with `flavor:"zee-workflow"` comes back as a JSON-RPC error naming the valid flavors, and the response arrives promptly as a plain JSON error rather than the SSE stream the valid path takes. Happy-path counterpart in the same test: the identical call with `flavor:"minimal"` is NOT refused for its flavor (it proceeds into the streaming path). Also asserts the advertised `execute_command` schema still declares the `["minimal","full"]` enum, so the advertisement and the enforcement are pinned together.

## Regression — ITEM-8 must not change existing behaviour

- **TEST-16** (tier: integration) [covers: ITEM-8] file: `src-app/server/tests/mcp/run_in_sandbox_test.rs` — asserts: after the two ad-hoc flavor scans are replaced by the canonical helper, creating a system MCP server with an unknown `sandbox_flavor` still fails at HTTP **400** with error code **`INVALID_FLAVOR`** (the code assertion is added here — the pre-existing test asserted only the status, so the delegation could have silently changed the code), and creating one with `"minimal"` still succeeds — the byte-for-byte preservation ITEM-8 promises. The `mcp_user_policy` sibling's `MCP_UNKNOWN_FLAVOR`/422 message is asserted by TEST-17.
- **TEST-17** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/code_sandbox/mod.rs` — asserts: the canonical helper's refusal text still contains the flavor list in the `{:?}`-of-names form `mcp/user_policy/repository.rs`'s `MCP_UNKNOWN_FLAVOR` message is built from, so the user-policy error a admin sees is unchanged by the delegation; and that the helper is name-only (it never consults `KNOWN_FLAVORS`' size/description fields, which differ per flavor).

## Coverage map

| ITEM | covered by |
|---|---|
| ITEM-1 | TEST-1, TEST-2, TEST-3, TEST-11, TEST-12 |
| ITEM-2 | TEST-4, TEST-6, TEST-14 |
| ITEM-3 | TEST-5, TEST-6, TEST-14 |
| ITEM-4 | TEST-6, TEST-7, TEST-11 |
| ITEM-5 | TEST-9 |
| ITEM-6 | TEST-8, TEST-13 |
| ITEM-7 | TEST-10, TEST-15 |
| ITEM-8 | TEST-16, TEST-17 |
| ITEM-9 | TEST-3 |

| INV | acceptance test |
|---|---|
| INV-1 | TEST-6 |
| INV-2 | TEST-12 |
| INV-3 | TEST-9, TEST-10, TEST-13 |

## Not applicable

- **e2e** — the diff touches no `src-app/ui/**` or `src-app/desktop/ui/**` file, so
  the phase-3 frontend e2e requirement is not triggered (BASE.md).
- **`[negative-perm]` restricted-user e2e (A10)** — no permission is introduced;
  both surfaces are gated by pre-existing perms (`background::use`,
  `code_sandbox::execute`) whose deny paths are already covered.
