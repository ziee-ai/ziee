# DRIFT-1 — implementation vs PLAN

Reconciling the written code against PLAN.md / DECISIONS.md after implementing
ITEM-1, ITEM-2 and the tests. Each entry is a place the implementation could have
diverged from the plan; verdict records how it was resolved.

- **DRIFT-1.1** — verdict: none — ITEM-1 landed exactly as planned: `pub const ANTHROPIC_VERSION` added at the top of `anthropic.rs`, the 3 literals replaced (verified value-identical), re-exported from `lib.rs` as `pub use providers::anthropic::ANTHROPIC_VERSION`, and `discover.rs` sends `.header("anthropic-version", ai_providers::ANTHROPIC_VERSION)`. `cargo check -p ai-providers` + `cargo test --lib -p ziee discover` green.
- **DRIFT-1.2** — verdict: none — ITEM-2 landed as planned: `parse_one_live_model` now reads `name` then falls back to `display_name`; the existing 4 parser tests plus the 2 new ones (TEST-1) pass, so OpenRouter/OpenAI/Gemini outputs are unchanged.
- **DRIFT-1.3** — verdict: impl-wins — TEST-2 asserts on a live model id (`claude-probe-test-model`) that is deliberately NOT in the curated catalog, so `source == "discovery"` proves the header-gated live call actually returned (a catalog id would have been indistinguishable from Layer 1). Sharper than the plan's generic "models appear"; same intent.
- **DRIFT-1.4** — verdict: impl-wins — TEST-3 creates the anthropic provider via a direct `page.request.post` with a deliberately-invalid key instead of `createProviderViaAPI`, because that helper would pick up a real `ANTHROPIC_API_KEY` from the CI env and make the fallback note non-deterministic. The inline create forces the exact reported scenario (bad key → live fail → catalog fallback + note) deterministically. Intent (assert non-blocking picker on a fallback note) unchanged.
- **DRIFT-1.5** — verdict: resolved — Infra-only (not a code drift): the repo's committed `src-app/target` symlink points at a non-existent `pbya` path, so builds+harness need a real target dir. Resolved by re-pointing the symlink to a writable dir and `git update-index --skip-worktree`-ing it so the change is never committed. No product-source impact; documented in STATUS.md.

**Unresolved drifts:** 0
