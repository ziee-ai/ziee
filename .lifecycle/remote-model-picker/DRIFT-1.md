# DRIFT-1 — remote-model-picker (implementation vs plan)

Round 1 audit of the implemented diff against PLAN.md / DECISIONS.md.

- **DRIFT-1.1** — verdict: resolved — ITEM-7 needed a debug-only loopback SSRF seam (`LLM_DISCOVER_ALLOW_LOOPBACK`, `cfg!(debug_assertions)`) added to `discover.rs::discovery_url_policy` so integration tests can point a provider at a 127.0.0.1 mock `/models`. Not in the original plan text but required by the no-cosmetic-tests rule (mock only the HTTP boundary); mirrors web_search's `WEB_SEARCH_FETCH_ALLOW_LOOPBACK`. Release behavior unchanged (PUBLIC_HTTP_OR_HTTPS).
- **DRIFT-1.2** — verdict: none — the sweep decision logic was extracted into a pure `decide_deprecations` fn (network/DB-free) so TEST-5 is a true unit test; `sweep_provider_once` calls it. Consistent with DEC-8 (folded into prune.rs) — a refinement, not a divergence.
- **DRIFT-1.3** — verdict: none — the sweep threads the loop's `pool` via free repo fns (`list_llm_providers`, `list_llm_models_by_provider`, `set_model_deprecated`) instead of the global `Repos`, to avoid a boot-order race where the loop's first tick could precede `Repos` init. Matches the module's existing `prune.rs` style; DEC-8 unaffected.
- **DRIFT-1.4** — verdict: resolved — the e2e specs live in `src-app/ui/tests/e2e/llm/` (the real dir), not the `05-llm/` path TESTS.md originally named. TESTS.md updated; no behavioral change.
- **DRIFT-1.5** — verdict: none — the add drawer surfaces the auto-populated capabilities as editable toggles + a context-window field (flat form fields matching the existing onValid mapping), rather than only defaulting them. This realizes ITEM-2's "all fields stay user-overridable"; no plan contradiction.
- **DRIFT-1.6** — verdict: none — OpenRouter icon uses `FaRoute` (react-icons/fa); PLAN left the specific icon unspecified.
- **DRIFT-1.7** — verdict: none — the reconcile route is `POST /llm-providers/{id}/refresh-models`, registered in `llm_model/routes.rs` (ITEM-10) with the handler in `llm_model/handlers/models.rs`; matches DEC-9 (perm `llm_providers::read`, returns the refreshed list).

**Unresolved drifts:** 0
