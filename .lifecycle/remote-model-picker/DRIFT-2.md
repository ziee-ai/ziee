# DRIFT-2 — remote-model-picker (round 2, post scoped-test run)

Running the scoped integration tests surfaced one behavioral refinement.

- **DRIFT-2.1** — verdict: impl-wins — the background deprecation sweep now skips **disabled** providers (`!provider.enabled`), not just local ones. The first tick fires at boot; without this it probed every seeded-but-disabled built-in provider (OpenAI/Anthropic/…) against its real SaaS `/models` at every startup, producing pointless outbound calls + 401s. Enabled-only is consistent with DEC-2 ("remote-only") and DEC-5 (no-op safety) intent. The on-demand refresh endpoint (`sweep_provider_once`) is unchanged — it still reconciles any provider the admin explicitly refreshes. PLAN.md ITEM-8 amended in spirit (remote **and enabled**); no test change needed (integration tests drive the endpoint, not the loop).

**Unresolved drifts:** 0
