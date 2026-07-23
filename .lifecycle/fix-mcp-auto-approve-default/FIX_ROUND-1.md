# FIX_ROUND-1 — fixes applied, then re-audited

## Method note (honest limitation)

This session was explicitly instructed not to spawn subagents, so the phase-6
audit and this re-audit were performed **in-session against the diff**, not by
blind/fresh subagents as the skill's default prescribes. It is therefore weaker
on independence than a true blind pass. Mitigations actually used: each angle was
run as a separate targeted pass over the diff rather than one impressionistic
read; every claim was checked by OPENING the referenced code (e.g. `store-kit.ts`
to confirm the proxy calls `useStore`, `provider-helpers.ts` to confirm the model
default is env-overridable) rather than asserted from memory; and the highest-value
finding was caught by a TEST failing, not by inspection. Recorded here rather than
left implicit.

## Confirmed findings from round 1, and their fixes

| # | Angle | Severity | Fix |
|---|---|---|---|
| 1 | state-management | HIGH | `extension.tsx` read a state prop off the store PROXY inside an async non-component hook — an invalid hook call, since `store-kit.ts:283-286` resolves non-function props via `useStore`. Hoisted to `Stores.McpComposer.$.serverDefaultApprovalMode`. |
| 2 | correctness | HIGH | `auto_approved_tools` bound `Value::Null` (JSON null, not SQL NULL), so its COALESCE preserve-arm was unreachable and an omitted allow-list was destroyed. Bound `Option<Value>` in both upserts (ITEM-14). |
| 3 | api-friendliness | MEDIUM | Internal backend rationale was leaking into the generated TS client as JSDoc. Trimmed the `///` docs to client-facing one-liners; rationale moved to `//` comments. Regenerated. |
| 4 | api-contract | MEDIUM | A newly-REQUIRED response field broke the gallery cassette's `tsc`. Recorded a realistic response + moved the endpoint into the generator's documented `LOOSE` set. |
| 5 | tests-quality | MEDIUM | The e2e selected its model by a literal that an env override can change. It now creates and selects by its own display-name constant. |
| 6 | maintainability | LOW | `saveProjectConfig` called `get()` a second time for one field; reuses the captured `state`. |

Five findings in the ledger were investigated and **rejected** with reasons
(perms-authz loosening, `#[default]` blast radius, concurrency of the preserve
semantics, error-propagation through the `Option` refactor, and the const→fn
shape). Those are recorded as `status: rejected`, not silently dropped.

## Re-audit after the fixes

Re-read every fixed hunk plus everything the fixes touched:

- **`extension.tsx` hoist** — verified the snapshot is taken once at the top of
  `onConversationLoad`, before any await, and that only ACTIONS (`setCurrentConversation`,
  `loadConversationConfig`) remain on the proxy. Confirmed the pre-existing
  `Stores.McpServer.$` read two lines below follows the same rule, so the file is
  now internally consistent. Noted (not a defect): if the defaults fetch has not
  resolved yet, the value is the safe restrictive fallback — the same race the
  code had before, and it only affects a branch reached when the conversation has
  no stored settings at all.
- **`Option<Value>` binding** — re-read both upserts; the `?` on
  `serde_json::to_value` is INSIDE the `Some(...)` arm, so a serialization failure
  still errors the request rather than silently degrading to SQL NULL (which would
  preserve stale data). Verified by the suite: 19/19 integration tests green,
  including the two that previously failed.
- **All 10 `blankMcpConfig` call sites** — checked each resolves `state` legally:
  three read the immer draft inside `set(...)`, one reads a `get()` snapshot
  captured two lines above, the rest likewise. No `get()` inside a `set()`.
  Confirmed `??` vs the original `||` is equivalent here (the value is a non-empty
  enum string), and that the conditional `loopSettings` spread yields the same
  `config.loopSettings === undefined` as the previous explicit-undefined key.
- **`McpConfigModal` destructure** — adding one property to the top-level
  destructure adds one unconditional `useStore` call in a fixed position, so hook
  order is stable across renders; `effectiveApprovalMode` is likewise called
  unconditionally. No new conditional render branch.
- **e2e model pinning** — verified the argument position
  (`createModelViaAPI(apiURL, token, providerId, modelName?, displayName?, providerType)`),
  so the constant lands in `displayName`, which `selectModelInDropdown`'s
  substring/`exact: false` role match then finds.
- **Regenerated artifacts** — `tsc --noEmit` clean; the `types.ts` delta is three
  small client-relevant hunks; the `openapi.json` content delta (verified by
  diffing sorted files) is still exactly the two `required` removals + `anyOf/null`
  additions + the new property.

**New confirmed findings:** 0
