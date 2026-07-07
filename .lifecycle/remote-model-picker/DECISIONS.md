# DECISIONS — remote-model-picker

### DEC-1: Free-text fallback mechanism, given Combobox isn't creatable?
**Resolution:** A "Enter a custom model ID" toggle in the drawer that swaps the Combobox for a plain `<Input>` (the current field). The picker is the default; custom is one click away.
**Basis:** codebase — `combobox.tsx` is select-from-options only; a toggle avoids extending the shared kit component.

### DEC-2: How deep does orphan handling go?
**Resolution:** Background periodic sweep (~6h, remote-only) that auto-sets `is_deprecated`, plus an on-demand "Refresh models" reconcile and UI badges + manual keep/remove.
**Basis:** user (this session).

### DEC-3: Is OpenRouter a first-class provider type?
**Resolution:** Yes — new `provider_type='openrouter'`, seeded built-in, keyless public `/models` discovery parsed for context + capabilities.
**Basis:** user (this session).

### DEC-4: How much metadata auto-populates, and pricing?
**Resolution:** Auto-fill capability toggles + `context_length`; no pricing persisted or surfaced. OpenRouter pricing is parsed and dropped.
**Basis:** user (this session). Reconciles the "OpenRouter carries pricing" fact against the "no pricing" metadata choice.

### DEC-5: Sweep false-flag safety?
**Resolution:** Only mutate `is_deprecated` when the live `/v1/models` call succeeded and returned a non-empty set; catalog-only fallback (missing key / offline / 401) is a no-op. Providers needing a key but lacking one are never auto-deprecated.
**Basis:** convention — matches the sync re-check "transient DB error keeps the stream" defensive posture.

### DEC-6: Does the sweep run on desktop (embeds the server)?
**Resolution:** Yes — it runs on desktop too (remote providers are valid there); no config gate needed since it only touches remote-provider rows and self-skips locals.
**Basis:** codebase — desktop embeds the server, and `llm_model/prune.rs` already runs unconditionally there; the sweep inherits that with no host dependency.

### DEC-7: max_output_tokens persistence?
**Resolution:** Deferred — no model column exists; it is not persisted this iteration (discovery may still surface it as a picker hint if trivial). Revisit when explicitly requested.
**Basis:** codebase — adding a column is outside the approved scope (capabilities + context only).

### DEC-8: Where does the sweep/reconcile logic physically live?
**Resolution:** Folded into the existing `llm_model/prune.rs` loop as an added per-tick job, with a `sweep_provider_once(pool, provider)` fn reused by the on-demand reconcile handler. No new background loop or module file.
**Basis:** codebase — the module already spawns exactly one best-effort loop at `mod.rs::init()`; a second loop would duplicate the tick/backoff machinery.

### DEC-9: Reconcile endpoint shape + permission?
**Resolution:** `POST /api/llm-providers/{provider_id}/refresh-models` returning the refreshed model list, gated by `llm_providers::read` (same as discover-models). It reuses `sweep_provider_once` and emits the dual sync pair.
**Basis:** convention — mirrors the existing `discover-models` route's permission + the model sync emit already in `create_model`.
