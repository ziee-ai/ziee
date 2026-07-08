# PLAN — ask-user-decision-ux

Upgrade ziee's built-in `ask_user` elicitation to feel like Claude Code's
`AskUserQuestion` decision UX (per-option descriptions, a 1–4 question wizard,
an always-available "Other" escape, a recommended-first marker, optional
per-option preview) **without losing** the existing input-validation strengths
(typed/validated inputs, decline/cancel, size-cap + injection guard) and
**without changing** the MCP-external elicitation path (must stay
spec-compliant / flat).

## Key architectural fact (verified)

The backend elicitation pipeline is **schema-agnostic**:
`SSEChatStreamMcpElicitationRequiredData.requested_schema` is a
`serde_json::Value`, persisted to `message_contents` JSONB verbatim, re-served
on reload, and the response `content` is an arbitrary `Record<string,unknown>`
keyed by property name (`McpComposer.store.ts::resolveElicitation`). Therefore:

- **Multi-question = the existing flat object schema with N properties** (each
  property = one question/step). The response envelope stays a flat
  `{ [prop]: value }` object — identical to today, so the accepted-state summary
  render and back-compat both hold for free.
- **All rich features are schema conventions + a frontend renderer.** The only
  backend changes are (a) a tiny deterministic marker stamp that tells the FE
  "this is the ziee-internal `ask_user` path" and (b) the tool-descriptor text
  that teaches the model the conventions. No new tool param, no array wrapper, no
  migration, no OpenAPI type change.

## Items

- **ITEM-1**: Backend — stamp an `x-ziee-askuser: true` marker onto the `ask_user` `requested_schema` root via a pure, unit-testable helper `stamp_ask_user_marker(Value) -> Value` in `mcp/chat_extension/helpers.rs`, called AFTER `cap_requested_schema` in `run_ask_user_elicitation` (only when the schema is a JSON object). This is the single authoritative gate the FE uses to enable rich mode; the external-MCP elicitation path never stamps it, so that path is byte-identical. The marker is a few bytes → the 1 MiB size cap and the "measure RAW schema before cap" injection/size guard both remain intact and fire first.
- **ITEM-2**: Backend — rewrite the `ask_user` tool descriptor in `elicitation_mcp/tools.rs` to document the rich schema conventions the model should emit: per-option `enumDescriptions[]` (parallel to `enum`/`enumNames`), `x-ziee-recommended` (a value flagged recommended), `x-ziee-allow-other` (defaults on for choice questions), optional `enumPreviews[]`, and "use multiple properties for a 1–4 question wizard". `inputSchema` stays exactly `{ message, schema }` (full back-compat; old flat schemas still valid).
- **ITEM-3**: Frontend — extract the pure schema helpers into a new `components/elicitationOptions.ts`: the `FieldSchema` type extended with `enumDescriptions?`, `enumPreviews?`, `oneOf/anyOf` entry `description?`/`preview?`/`recommended?`, and field-level `x-ziee-recommended?`/`x-ziee-allow-other?`; `getRichOptions()` returning `{ value, label, description?, preview?, recommended? }[]`; the existing `buildFieldZodSchema`/`buildFormSchema`; `isChoiceField()`/`isMultiChoiceField()`; `orderRecommendedFirst()`; and the `OTHER_SENTINEL` const. `ElicitationFormContent.tsx` imports these (its non-rich `renderField` behavior stays byte-identical).
- **ITEM-4**: Frontend — per-option descriptions: in the rich path, single-select choice questions render as **radio-cards** and multi-select as **checkbox-cards**, each card showing the option label + its description (+ preview from ITEM-8). Reuses kit `RadioGroup`/`Checkbox` primitives with rich `ReactNode` labels; no raw-DOM inputs.
- **ITEM-5**: Frontend — recommended-first: when a choice field carries `x-ziee-recommended` (a value) or a `oneOf/anyOf` entry marks `recommended: true`, that option is ordered FIRST and rendered with a "Recommended" `Badge`.
- **ITEM-6**: Frontend — always-available "Other": choice questions get an auto-appended "Other…" card (unless `x-ziee-allow-other: false`). Selecting it reveals a free-text `Input`; for single-select the typed value BECOMES the field answer, for multi-select it is added as an extra selected value. No zod change needed (choice zod is `z.string().min(1)` / `z.array(z.string())`, not enum-restricted, so free text validates).
- **ITEM-7**: Frontend — wizard: a rich schema with ≥2 properties renders as a Next/Back stepper (one question per step, a "Step k of N" indicator, Back disabled on step 1, a single final Submit); exactly 1 property renders with no wizard chrome (visually today's single form). ONE `useForm` over all fields; Next validates only the current step's field (`form.trigger`); final Submit validates all and submits the whole flat values object (response envelope unchanged). Decline/cancel available on every step.
- **ITEM-8**: Frontend — option preview: an optional monospace/preformatted preview block rendered inside a card when the option carries a preview (`enumPreviews[i]` or `oneOf` entry `preview`). Purely additive; absent → no block (no layout shift).
- **ITEM-9**: Frontend — rich-mode gating: `ElicitationFormContent`'s PENDING branch delegates to the new `AskUserWizardContent` **only** when `requested_schema['x-ziee-askuser'] === true`; otherwise the existing renderer runs unchanged. The accepted/declined/cancelled states are shared and untouched. This keeps external MCP elicitation flat/spec-compliant and the generic elicitation e2e specs green.
- **ITEM-10**: Frontend — gallery coverage: add a rich `ask_user` pending state (radio-cards + recommended badge + Other + a 2-question wizard) to the gallery deep-states so the `check:state-matrix` gate inside `npm run check` stays green after the new render variant is introduced.

## Files to touch

Backend:
- `src-app/server/src/modules/elicitation_mcp/tools.rs` — rewrite descriptor (ITEM-2) + assert-descriptor unit test.
- `src-app/server/src/modules/mcp/chat_extension/helpers.rs` — `stamp_ask_user_marker` helper + call site + unit tests (ITEM-1).

Frontend (all under `src-app/ui/`):
- `src/modules/mcp/chat-extension/components/elicitationOptions.ts` — NEW shared pure helpers (ITEM-3).
- `src/modules/mcp/chat-extension/components/elicitationFields.tsx` — NEW shared non-choice field renderer (`renderInputField`), imported by both the legacy renderer and the wizard so they never drift on input types (avoids a circular import ElicitationFormContent⇄AskUserWizardContent). Introduced during implementation (see DRIFT-1). (ITEM-3)
- `src/modules/mcp/chat-extension/components/AskUserWizardContent.tsx` — NEW rich renderer: wizard + choice-cards + Other + recommended + preview (ITEM-4/5/6/7/8).
- `src/modules/mcp/chat-extension/components/ElicitationFormContent.tsx` — import shared helpers; delegate PENDING branch to the wizard when rich; non-choice branch of `renderField` delegates to `renderInputField` (behavior preserved) (ITEM-3/9).
- `src/dev/gallery/deepStates.tsx` + `src/dev/gallery/fixtures/chat-deep.ts` (the rich ask_user bundle fixture) + the manual registries `src/dev/gallery/coverage.ts` and `src/dev/gallery/stateCoverage.ts` (new component + its `:error` signal), plus the mechanically-regenerated `galleryCoverage.generated.ts` / `stateMatrix.generated.ts` / `STATE_MATRIX.md` / `components/ui/testIds.generated.ts` — rich ask_user gallery cell (ITEM-10).
- Tests: `src/modules/mcp/chat-extension/components/elicitationOptions.test.ts` (NEW unit); `tests/e2e/chat/ask-user-elicitation.spec.ts` (extend) + `tests/e2e/chat/ask-user-decision-ux.spec.ts` (NEW e2e for cards/recommended/other/wizard). Helper `tests/e2e/helpers/sse-mock-helpers.ts` accepts arbitrary schemas already — no change needed.

No migration. No OpenAPI regen (no Rust type change — `requested_schema` is already `serde_json::Value`; the stamp is a runtime value, the descriptor a string literal).

## Patterns to follow

- **Backend ask_user path** — mirror the existing pure-helper + `#[cfg(test)]`
  idiom already in `mcp/chat_extension/helpers.rs` (`cap_structured_content`,
  `ask_user_tool_result`, `cap_requested_schema`): a small pure function with
  colocated unit tests, called from `run_ask_user_elicitation`. The stamp sits
  right after `cap_requested_schema` so the size guard runs first.
- **Frontend renderer** — mirror the existing `ElicitationFormContent.tsx`
  (kit `Form`/`FormField` + `useForm(zodResolver(...))`, kit `Card` with a
  footer button row, the `elicitation-*` / `elicitation-field-<name>` /
  `-opt-<value>` testid conventions, the `Stores.McpComposer.resolveElicitation`
  accept/decline calls). Reuse kit `RadioGroup`/`Checkbox`/`Badge`/`Input`
  (no raw DOM, no antd) per the kit guardrails; option cards mirror the
  settings-card visual idiom (bordered, `data-slot`-clean kit components).
- **Gallery** — mirror the existing elicitation gallery cell in
  `src/dev/gallery/deepStates.tsx` and regen coverage with the gallery
  generator, exactly as other conditional render states are registered.
- **Tests** — mirror the existing `ask-user-elicitation.spec.ts` (page.route
  SSE mock via `sse-mock-helpers`, `captureElicitationResponses`) and the
  in-source `#[cfg(test)]` tiers in `helpers.rs` / `tools.rs`.
