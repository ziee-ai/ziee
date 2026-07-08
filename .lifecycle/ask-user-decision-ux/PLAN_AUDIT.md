# PLAN_AUDIT — ask-user-decision-ux

Audit of PLAN.md against the actual codebase (read: `elicitation_mcp/{tools,handlers}.rs`,
`mcp/elicitation/models.rs`, `mcp/chat_extension/{helpers.rs, extension.rs}`,
`ui/.../ElicitationFormContent.tsx`, `McpComposer.store.ts`,
`ui/src/components/ui/kit/{radio-group,checkbox,select}.tsx`, the ask_user e2e spec).

## Breakage risk

- **Existing `elicitation_mcp/tools.rs` unit test** (`exposes_ask_user_tool`)
  asserts: exactly one tool named `ask_user`, `required` contains `message` +
  `schema`, and the two property `type`s. ITEM-2 rewrites only the `description`
  string and keeps `inputSchema` identical → test stays green. (Will still
  re-run it.)
- **`run_ask_user_elicitation` callers**: the only production call is in
  `execute_tool` (helpers.rs:404) and there are in-source tests. ITEM-1 adds a
  stamp AFTER `cap_requested_schema` and does not change the function signature
  or return type → no caller breakage. The stamp only mutates the forwarded
  `requested_schema` value (adds one key).
- **External MCP elicitation** flows through `mcp/client/http.rs` (NOT
  `run_ask_user_elicitation`) and is never stamped with `x-ziee-askuser`, so the FE
  rich-mode gate (`schema['x-ziee-askuser'] === true`) is false for it → its
  rendering is byte-identical. The two generic elicitation e2e specs
  (`mcp-elicitation-form-rendering`, `mcp-elicitation-submit-roundtrip`) exercise
  UN-stamped schemas and remain green.
- **`ElicitationFormContent.tsx` refactor** (ITEM-3): moving `getOptions` /
  `buildFieldZodSchema` / `buildFormSchema` into `elicitationOptions.ts` is a
  pure extraction; the non-rich `renderField` branch and the accepted/declined/
  cancelled branches stay in-place and behaviorally identical. Risk contained by
  ITEM-9's top-of-PENDING gate: non-rich path is the untouched code path.
- **Response envelope**: wizard collects into ONE flat `{prop: value}` object via
  a single `useForm`, exactly what `resolveElicitation(id,'accept',content)` and
  the accepted-state `Object.entries(responseContent)` summary already expect →
  no store or persistence change.
- **"Other" free-text vs zod**: choice zod is `z.string().min(1)` (single) /
  `z.array(z.string())` (multi) — NOT enum-restricted — so an arbitrary Other
  string validates without any zod change. Verified in `buildFieldZodSchema`.

## Pattern conformance

- Backend: ITEM-1/2 mirror the existing pure-helper + colocated `#[cfg(test)]`
  idiom already present in `helpers.rs` (`cap_structured_content`,
  `ask_user_tool_result`) and `tools.rs`. Conformant.
- Frontend: ITEM-4/5/6/7/8 reuse kit `RadioGroup`/`Checkbox`/`Badge`/`Input`/
  `Card`/`Form`/`FormField` (all exported from `@/components/ui`); no raw DOM /
  antd, satisfying the biome guardrails. `RadioOption.label` and
  `CheckboxProps.label` are `ReactNode`, so rich card labels (title +
  description + badge + preview) are supported without a new kit component.
  Conformant with [[feedback_check_library_before_custom]] and the kit reuse rule.
- Testid conventions preserved: `elicitation-*`, `elicitation-field-<name>`,
  `-opt-<value>` (kit Select already derives `${testid}-opt-${value}`; the rich
  cards replicate the same `-opt-<value>` suffix so selectors stay analogous).

## Migration collisions

- None. No DB schema change (elicitation content is JSONB `message_contents`,
  already agnostic; no new table/column). `ls migrations/` is not consulted —
  the plan adds zero migrations.

## OpenAPI regen

- **Not required.** `SSEChatStreamMcpElicitationRequiredData.requested_schema`
  is already `serde_json::Value`; the `x-ziee-askuser` stamp is a runtime value
  inside it, not a type change. The `ask_user` descriptor edit is a string
  literal in a hand-built `json!` value, not an aide/schemars type. The response
  envelope (`RespondToElicitationRequest`, `ElicitationResponse`) is unchanged.
  No `just openapi-regen` needed in either `ui` or `desktop/ui`; no
  `api-client/types.ts` delta. (Confirmed: no `#[derive(JsonSchema)]`/aide route
  signature is touched.)

## Per-item verdicts

- **ITEM-1** — verdict: PASS — pure helper + call after `cap_requested_schema`; mirrors existing helper idiom; size-cap/injection guard run first and are untouched.
- **ITEM-2** — verdict: PASS — description-only edit; `inputSchema` unchanged; existing descriptor test still asserts the invariants it keeps.
- **ITEM-3** — verdict: PASS — pure extraction into a new module; non-rich rendering paths stay in place; enables unit-testing the helpers.
- **ITEM-4** — verdict: PASS — reuses kit `RadioGroup`/`Checkbox` with `ReactNode` rich labels; no new kit component, no raw DOM.
- **ITEM-5** — verdict: PASS — ordering + `Badge`; purely presentational on the rich path; `orderRecommendedFirst` is pure/unit-testable.
- **ITEM-6** — verdict: PASS — Other reveals a kit `Input`; free text validates under existing choice zod; no envelope change.
- **ITEM-7** — verdict: CONCERN — wizard changes the visual for ANY rich schema with ≥2 properties; mitigated because rich mode is gated to the stamped ask_user path only, existing specs use 1 property, and the response envelope is unchanged. Must add an e2e that proves per-step validation + Next/Back + single final submit (budgeted in TESTS).
- **ITEM-8** — verdict: PASS — additive preview block; absent → no render; low risk.
- **ITEM-9** — verdict: PASS — single `schema['x-ziee-askuser'] === true` gate at the top of the PENDING branch; the untouched non-rich code path guarantees external-MCP parity.
- **ITEM-10** — verdict: CONCERN — the `check:state-matrix` gate inside `npm run check` requires a gallery cell for the new render variant; must add the deep-state and regen coverage or phase 8 fails. Explicitly budgeted (ITEM-10 + TESTS).
