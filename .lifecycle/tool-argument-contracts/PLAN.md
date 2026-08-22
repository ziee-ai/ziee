# PLAN — tool-argument-contracts

Fix two defects that are the same failure: **a schema advertised to the model
that the server does not enforce, failing silently or blaming the wrong field.**

- **Defect 1** — `spawn_background` reads `kind` from the top level ONLY, so a
  `kind` nested inside `spec` (which the open `spec` object invites) is dropped,
  the `subagent` default is substituted, and the call is refused with
  `spec.task must be a non-empty string` — naming a field the model deliberately
  did not send. Worse: when the nested-`kind` spec *also* carries `task`, the
  call **succeeds** and runs a sub-agent instead of the requested shell command,
  with no error at all.
- **Defect 2** — `flavor` is advertised as `"enum": ["minimal","full"]` in the
  background tool schema and the chat `execute_command` schema, and enforced in
  neither. Any non-empty string reaches
  `format!("ziee-sandbox-rootfs-{arch}-{flavor}.{ext}")` → a live GitHub Releases
  request. A model invented `"zee-workflow"` and it reached the network.

## Design source

Realizes `.lifecycle/tool-argument-contracts/DESIGN.md` — §"The rule" and its
three sub-rules (1 actionable refusals, 2 no silent default substitution,
3 advertised enums enforced before use), plus its §"Scope boundary".

`DESIGN.md` itself is derived from the in-tree contract already written in
`src-app/server/src/common/tool_args.rs` (module docs) and
`agent-kit/docs/CODING_GUIDELINES.md` §2 / §10 / §11.

## Invariants

Lifted verbatim from the design.

- **INV-1**: Anything that cannot become the declared shape is an `ArgError` whose message names **what was received**, **what is expected**, and **a literal JSON example the model can copy**.
- **INV-2**: A supplied-but-undecodable argument is **never** turned into the default — that would hide the mistake instead of correcting it.
- **INV-3**: A value the tool schema advertises as an `enum` is validated against that enum by the server **before** it is used to construct any URL, filesystem path, or process argument.

## Items

- **ITEM-1**: `spawn_background` resolves `kind` from the top-level `kind` **or** `spec.kind`. A nested-only `kind` is honoured (the fallback), so it is never silently replaced by the `subagent` default. Top-level and nested `kind` that DISAGREE are refused, naming both values — never silently preferring one.
- **ITEM-2**: An unrecognised `kind` value is refused with a message that names the argument, lists the valid kinds, and carries a copyable literal-JSON example. (Today `BACKGROUND_KIND_UNKNOWN` is the bare text `unknown background kind '<x>'`.)
- **ITEM-3**: `spec` is validated against the property set the schema advertises (`kind`, `task`, `system`, `command`, `flavor`). An unknown key is refused naming the offending key(s), the accepted keys, and an example — and the `spec` schema is amended with `"additionalProperties": false` so the advertisement matches the enforcement.
- **ITEM-4**: The per-kind required-field refusals (`BACKGROUND_TASK_REQUIRED`, `BACKGROUND_COMMAND_REQUIRED`) name the argument, what is expected, and a copyable example. When the OTHER kind's required field is present instead, the message additionally names the real mistake ("you sent `spec.command`, which belongs to `kind: 'sandbox_exec'`") rather than demanding a field the model did not want.
- **ITEM-5**: One canonical rootfs-flavor allow-list check lives beside the server-side `KNOWN_FLAVORS` re-export (`code_sandbox::{is_known_flavor, known_flavor_names, validate_flavor}`), so no call site can ship a weaker check than its siblings.
- **ITEM-6**: `spawn_background{kind:'sandbox_exec'}` validates `spec.flavor` against `KNOWN_FLAVORS` **before** the `workflow_runs` row is created and therefore before any URL is constructed.
- **ITEM-7**: The chat `execute_command` path validates `flavor` against `KNOWN_FLAVORS` before entering `execute_command_stream` (which is what reaches `ensure_fetched` → `install_version` → the GitHub URL).
- **ITEM-8**: The two pre-existing ad-hoc `KNOWN_FLAVORS` scans (`mcp/handlers/mod.rs::validate_sandbox_flavor`, `mcp/user_policy/repository.rs`) delegate to the canonical helper, preserving their existing error codes and HTTP statuses byte-for-byte.
- **ITEM-9**: `spec.kind`, once consumed, is stripped before the spec is persisted as `inputs_json`, so the stored spec matches the vocabulary the schema declares and no later reader can re-derive a stale kind from it.

## Files to touch

- `src-app/server/src/modules/background_mcp/tools.rs` — ITEM-1/2/3/4/6/9 + unit tests
- `src-app/server/src/modules/code_sandbox/mod.rs` — ITEM-5 + unit tests
- `src-app/server/src/modules/code_sandbox/handlers.rs` — ITEM-7 + unit test
- `src-app/server/src/modules/mcp/handlers/mod.rs` — ITEM-8
- `src-app/server/src/modules/mcp/user_policy/repository.rs` — ITEM-8
- `src-app/server/tests/background_mcp/spawn_contract.rs` (new) + `tests/background_mcp/mod.rs`
- `src-app/server/tests/code_sandbox/tier3_flavor_contract.rs` (new) + `tests/code_sandbox/mod.rs`

No migration. No permission. No frontend file. No `openapi.json` regen (see BASE.md).

## Patterns to follow

| area | mirror |
|---|---|
| model-facing refusal wording + the "names arg / expected / example" rule | `src-app/server/src/common/tool_args.rs` (`refuse()`, and `conformance::assert_actionable`) |
| arg decoding at a built-in MCP tool entry point | `background_mcp::tools::decode_spec_arg` (already in-tree, immediately above the code being changed) |
| flavor allow-list check + error shape | `mcp/handlers/mod.rs::validate_sandbox_flavor` (bad_request `INVALID_FLAVOR`) and `mcp/user_policy/repository.rs` (unprocessable `MCP_UNKNOWN_FLAVOR`) — both are folded into the canonical helper by ITEM-8 |
| background MCP integration test (JSON-RPC over the real route, `x-conversation-id`, `background::use` user) | `src-app/server/tests/background_mcp/mod.rs` helpers + `runs.rs` |
| sandbox HTTP handler test | `src-app/server/tests/code_sandbox/tier3_http.rs` |
| in-source unit tests for a tools module | `background_mcp/tools.rs` existing `#[cfg(test)] mod tests` (e.g. `spawn_kind_enum_advertises_sandbox_exec`) |

## UI-surface checklist

Not applicable — this branch adds no page, drawer, card or panel, and touches no
`src-app/ui/**` or `src-app/desktop/ui/**` file. The only user-visible surface is
the text a model sees in a tool refusal, which is covered by INV-1.

---

# Plan audit (phase 2, folded in)

Verified against the codebase at `db2347928` before writing any code.

## Breakage risk

- `spawn_background`'s current behaviour for a **correctly-formed** call is
  unchanged by every item: top-level `kind` still wins, absent `kind` still
  defaults to `subagent`, `spec.task`/`spec.command` are read the same way. The
  only calls whose outcome changes are ones that are already wrong (dropped
  nested `kind`, unknown key, invented flavor) or already silently wrong
  (nested `kind` + `task`).
- ITEM-3 (`additionalProperties: false` + server-side unknown-key refusal) is the
  one item that can refuse a call that previously "worked". It cannot break a
  call that used only advertised fields, and a call that used an unadvertised
  field was already having that field ignored — so nothing that previously *did*
  something stops doing it. Verified: `spec` is consumed only by
  `spawn_subagent` (`task`, `system`) and `spawn_sandbox_exec`
  (`command`, `flavor`); its other use is verbatim persistence into
  `inputs_json`.
- ITEM-9 (strip `spec.kind` before persistence) — verified no reader: `grep -rn
  inputs_json src-app/server/src` shows background runs WRITE `inputs_json` at
  `tools.rs:268` and `:503` and nothing reads a background run's `inputs_json`
  back (`runner.rs:1616` is the workflow-run resume path, and the background
  surface refuses `job_kind='workflow'` rows). Before this branch a nested
  `spec.kind` was *already* persisted verbatim, so stripping it is strictly a
  narrowing of stored data, not a regression.
- ITEM-8 preserves both existing error codes/statuses; the two call sites keep
  building their own `AppError`, so `INVALID_FLAVOR` (400) and
  `MCP_UNKNOWN_FLAVOR` (422) are unchanged. Existing tests that assert those
  codes stay green.
- ITEM-7 rejects before `execute_command_stream`. Verified this is upstream of
  the only path to the network: `handlers.rs` → `streaming::execute_command_stream`
  → `runtime_fetch::ensure_fetched` → `version_manager::install_version` →
  `format!("ziee-sandbox-rootfs-{arch}-{flavor}.{ext}")` → `build_download_url`.

## Pattern conformance

- `tool_args.rs` is the in-tree reference for a model-facing refusal and already
  exposes `conformance::assert_actionable`, which asserts exactly INV-1's three
  elements. The new refusals are asserted with the same three-element standard.
- The canonical flavor helper mirrors the two existing scans it replaces; it is
  placed in `code_sandbox/mod.rs` (an existing file, next to the module's other
  public surface) rather than a new file, so the diff adds no new module seam.
- `KNOWN_FLAVORS` itself lives in the `sdk/` submodule (`ziee-sandbox/src/types.rs`)
  and is re-exported as `crate::modules::code_sandbox::types::KNOWN_FLAVORS`. The
  helper wraps the re-export; the submodule is not touched (which would require a
  pointer bump this branch must not push).

## Migration collisions

None — the branch adds no migration. Server max in use is `202607200600`,
desktop max `10000000000005`; both untouched. (BASE.md.)

## OpenAPI regen

Not required. The edited schemas are runtime MCP tool descriptors returned by
`tool_list()` / `tool_definitions()` over JSON-RPC, not aide/schemars types. No
handler signature, request/response type, or `SyncEntity` variant changes, so
`openapi.json` and `api-client/types.ts` are byte-identical in both workspaces
and `openapi::emit_ts::tests::types_ts_parity` is unaffected. `tsc --noEmit` is
still run in both workspaces as an explicit gate.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — `tools.rs:195` confirmed to read `args.get("kind")` only; `grep -n 'spec.*kind\|get("kind")' background_mcp/*.rs` finds no `spec.get("kind")` anywhere in the module. The dispatch is a 12-line `match` with two arms; adding a resolver above it is additive.
- **ITEM-2** — verdict: PASS — the existing arm is `AppError::bad_request("BACKGROUND_KIND_UNKNOWN", format!("unknown background kind '{other}'"))`; upgrading the message changes no control flow.
- **ITEM-3** — verdict: CONCERN — this is the only item that can refuse a previously-accepted call. Mitigated by (a) restricting the accepted set to the UNION of both kinds' advertised keys plus `kind`, not the per-kind subset, so a cross-kind key is diagnosed by ITEM-4's targeted hint rather than a blunt refusal; (b) an explicit happy-path test that every advertised key is accepted. Recorded as DEC-1/DEC-3.
- **ITEM-4** — verdict: PASS — `BACKGROUND_TASK_REQUIRED` / `BACKGROUND_COMMAND_REQUIRED` are each raised at exactly one site (`tools.rs:222-228`, `tools.rs:470-478`); both are message-only edits.
- **ITEM-5** — verdict: PASS — `KNOWN_FLAVORS` is a `&[FlavorMetadata]` const with a `flavor: &'static str` field; a `.iter().any()` wrapper is exactly what the two existing sites already do.
- **ITEM-6** — verdict: PASS — `spawn_sandbox_exec` reads `flavor` at `tools.rs:479-485` and does not use it until `CreateBackgroundRun` at `:497`; the check inserts cleanly between.
- **ITEM-7** — verdict: PASS — `handlers.rs:216-220` reads `flavor` and passes it straight to `execute_command_stream` at `:225`; the check inserts between, returning the same `error_response(id, StatusCode::OK, JsonRpcError::invalid_params(..))` shape the `x-conversation-id` branch already uses.
- **ITEM-8** — verdict: PASS — both sites verified to be pure `.iter().any(|m| m.flavor == f)` scans (`mcp/handlers/mod.rs:44-58`, `mcp/user_policy/repository.rs:136-148`) that then build their own error; delegating the predicate keeps both errors byte-identical.
- **ITEM-9** — verdict: PASS — see *Breakage risk*; no reader of a background run's `inputs_json` exists.

## Observations recorded, deliberately NOT in scope

- `workflow_def.sandbox.flavor` (`workflow/runner.rs:1106/1293/1605` →
  `dispatch.rs:751` → `execute_command_with_mounts`) is a **third** instance of
  an unvalidated flavor reaching the same URL sink. `grep -n flavor
  workflow/validate.rs` returns nothing — the workflow YAML `sandbox.flavor` is
  not checked against `KNOWN_FLAVORS` either. It is excluded here as a different
  trust boundary (authored workflow config, sibling to the admin install path's
  deliberate future-flavor escape hatch) and is reported to the caller rather
  than silently fixed or silently ignored. See DEC-8.
