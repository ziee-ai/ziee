# DECISIONS — ask_user decision UX

All human/product inputs resolved up front so implementation runs nonstop.

### DEC-1: Is a multi-question request an array wrapper, a new tool param, or the existing flat-object schema?
**Resolution:** The existing flat-object schema. `ask_user` keeps `{ message, schema }`; `schema.properties` holds N entries and **each property is one question/step**. No new tool param, no array wrapper.
**Basis:** codebase — the response `content` is already a flat `{prop: value}` object (`McpComposer.store.ts::resolveElicitation`) persisted verbatim, so N-properties gives multi-question with ZERO envelope/persistence change and perfect back-compat.

### DEC-2: How is "rich mode" (cards/wizard/Other) restricted to the ziee-internal ask_user path so external MCP elicitation stays flat/spec-compliant?
**Resolution:** The backend stamps `"x-ziee-askuser": true` onto the `ask_user` `requested_schema` root (after `cap_requested_schema`, only when it's an object). The FE enables rich mode iff `requested_schema['x-ziee-askuser'] === true`. External MCP elicitation flows through a different path and is never stamped → renders exactly as today.
**Basis:** convention — a single deterministic authoritative marker is more robust than sniffing conventions (which would wrongly wizard-ify a plain multi-field external form); mirrors the task's "keep MCP-external elicitation flat" constraint.

### DEC-3: Schema convention for per-option descriptions?
**Resolution:** `enumDescriptions: string[]` parallel to `enum`/`enumNames` (index-aligned) for the legacy-enum form; and a `description` field on each `oneOf`/`anyOf` entry for the titled form. Both are read by `getRichOptions`.
**Basis:** convention — parallels the existing `enumNames[]` mechanism already in the renderer; the titled `oneOf` form already carries `title`, so adding `description` is the natural extension. Stays valid JSON-Schema-ish (`enumDescriptions` is an established react-jsonschema-form idiom).

### DEC-4: Schema convention for the recommended marker?
**Resolution:** Field-level `x-ziee-recommended: <value>` naming the recommended enum value; OR a per-`oneOf`/`anyOf` entry `recommended: true`. Renders that option first + a "Recommended" badge. At most one recommended option (first wins).
**Basis:** convention — an `x-ziee-*` vendor extension keeps it out of the standard keyword space; value-reference matches how `default` names a value.

### DEC-5: "Other" free-text — convention and default?
**Resolution:** Choice questions get an auto-appended "Other…" affordance BY DEFAULT (gap #3 = "always-available"). Opt out with `x-ziee-allow-other: false`. Selecting Other reveals a free-text input.
**Basis:** user — the task states the Other escape should be always-available so the user is never trapped; default-on with an explicit opt-out realizes that.

### DEC-6: Option preview (gap #5) — implement now or defer?
**Resolution:** Implement now. Convention: `enumPreviews: (string|null)[]` parallel to `enum` (null = no preview for that option), and a `preview` field on `oneOf`/`anyOf` entries. Rendered as a monospace `<pre>`-style block inside the option card.
**Basis:** convention/effort — once radio-cards exist the preview block is a few lines and rounds out the Claude-Code parity; cheaper to ship with the cards than to retrofit.

### DEC-7: When does the wizard engage vs a single form?
**Resolution:** Rich schema with ≥2 properties → Next/Back wizard, one question per step, "Step k of N" indicator, single final Submit. Exactly 1 property → no wizard chrome (visually today's single form, just card-rendered). Guidance in the tool description clamps the model to 1–4 questions (soft; not hard-rejected).
**Basis:** user — matches Claude Code's 1–4 question next/back UX; the 1-property no-chrome case preserves the overwhelmingly-common single-question visual and keeps existing specs green.

### DEC-8: Wizard state + how answers map back to the response envelope?
**Resolution:** ONE `react-hook-form` `useForm` over ALL properties (as today). The wizard is a presentational `stepIndex` over the property list. `Next` calls `form.trigger([currentField])` and only advances if valid; `Back` decrements (values preserved by the shared form state). Final `Submit` validates all and calls `resolveElicitation(id,'accept', values)` with the whole flat `{prop: value}` object — identical envelope to today.
**Basis:** codebase — reuses the existing single-form/zodResolver machinery; a flat values object is exactly what the store + accepted-state summary consume.

### DEC-9: Back-compat for old flat-schema ask_user?
**Resolution:** Fully preserved. `inputSchema` stays `{ message, schema }`. An old schema with no rich conventions still stamps `x-ziee-askuser` (so it enters rich mode) and renders correctly: plain enums become radio-cards (no descriptions/badge/preview, Other still offered), 1 property → no wizard. The accept/decline/cancel envelope is unchanged. External (unstamped) schemas use the untouched legacy renderer.
**Basis:** user — the task requires old flat-schema ask_user to still work; the marker + shared zod guarantee it.

### DEC-10: Rich single-select control — dropdown or cards? Multi-select?
**Resolution:** Rich single-select renders as **radio-cards** (kit `RadioGroup` with `ReactNode` labels), NOT a dropdown. Rich multi-select renders as **checkbox-cards** (kit `Checkbox` per option). Both show label + description (+ badge + preview).
**Basis:** user — Claude Code shows selectable cards with per-option descriptions; the kit `RadioGroup.label`/`Checkbox.label` accept `ReactNode` so no new kit component is needed.

### DEC-11: The "Other" sentinel value?
**Resolution:** `OTHER_SENTINEL = "__ziee_other__"`. When a single-select value equals the sentinel, the field's submitted value is the free-text input instead. For multi-select, the sentinel toggles the Other input; its typed value is appended to the selected array (sentinel itself is stripped before submit).
**Basis:** convention — an unlikely-to-collide reserved token, stripped from the emitted content so the model never receives the sentinel.

### DEC-12: Where does the rich renderer live?
**Resolution:** A new `AskUserWizardContent.tsx` (feature-local, same components dir) holds the wizard + choice-cards + Other + preview. `ElicitationFormContent.tsx`'s PENDING branch delegates to it when rich; everything else (shared helpers moved to `elicitationOptions.ts`, the non-rich renderer, the resolved-state cards) stays put.
**Basis:** convention — isolates the new surface, keeps the ElicitationFormContent diff small, and makes the non-rich path provably untouched.

### DEC-13: Injection guard + size cap — changed?
**Resolution:** Unchanged. The RAW-schema size check (`> MAX_STRUCTURED_CONTENT_BYTES` → error result) and `cap_requested_schema` run BEFORE the stamp; the stamp adds a few bytes and cannot push a within-cap schema over. Untrusted description/preview text is still rendered as inert data (never executed), same as today.
**Basis:** user — the task requires keeping the size-cap + injection guard; the stamp is deliberately placed after them.

### DEC-14: Does this feature touch `desktop/ui`?
**Resolution:** No. `ElicitationFormContent` exists only under `src-app/ui/src/modules/mcp/`; there is no desktop mirror (verified). Only the `ui` workspace gate applies.
**Basis:** codebase — `find desktop/ui -name ElicitationFormContent*` is empty.
