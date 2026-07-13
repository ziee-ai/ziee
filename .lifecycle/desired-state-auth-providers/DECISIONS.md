# DECISIONS

All decisions resolved by codebase convention or the task file — no genuine
product ambiguity requires an `AskUserQuestion` (the task file prescribes the
manifest shape, env-var names, mode, and behavior exactly).

### DEC-1: How is a secret field resolved / validated — same rule as MCP/user passwords?
**Resolution:** `client_secret` MUST be a single `${VAR}` placeholder, resolved via `resolve_secret_with`; an inline literal → `Skip(InlineSecret)`. `client_id` (not a secret) resolves via `resolve_with` (a placeholder that resolves empty → skip; a literal is allowed but the shipped file uses `${GOOGLE_CLIENT_ID}`).
**Basis:** convention — matches `reconcile_user`'s `password` handling in the same module and the task file's explicit "single `${ENV_VAR}` placeholder" rule.

### DEC-2: Absent auth-provider row — create it (like mcp) or skip?
**Resolution:** SKIP with a warn log; never create. The reconciler only ADDS/UPDATES fields on an existing (seeded) row and never deletes.
**Basis:** task file ("This only ADDS/UPDATES an existing row's fields; it must never delete") + codebase — an auth provider needs a `provider_type` and a full OIDC config shape that only a seed migration (47) supplies; creating one from the manifest would duplicate migration responsibility. Generic by name so a future microsoft/apple provider (each seeded by its own migration) can be enabled the same way.

### DEC-3: `mode` semantics for auth providers — reuse `ensure`/`enforce`?
**Resolution:** Reuse the existing `Mode` enum. `enforce` = re-stamp declared fields + re-assert `enabled` every boot (google ships `enforce`). `ensure` on an existing row = leave fields untouched (a no-op for the pre-seeded google row, so google MUST be `enforce` to actually enable).
**Basis:** convention — identical to `reconcile_mcp_server`. `enforce` is required because ziee's boot health path / an admin could disable the provider; enforce re-enables it on the next deploy (same rationale the mcp servers are enforce).

### DEC-4: Deploy switch — new env var or reuse `ZIEE_APPLY_DESIRED_STATE`?
**Resolution:** Reuse `ZIEE_APPLY_DESIRED_STATE=1`. No new switch.
**Basis:** task file ("Gated by the SAME switch — no new switch") + minimal-diff convention.

### DEC-5: Is Google enablement an operational tunable needing a settings row?
**Resolution:** Fixed by the manifest + env, NOT a new settings table. The enable/config is deploy-time config-as-code (the whole point of this module); the runtime already exposes admin CRUD (`/api/auth/providers`) to change it live afterward.
**Basis:** convention — the desired_state module IS the settings mechanism here; adding a settings row would duplicate the existing admin provider CRUD. (Configurable-settings rule satisfied: the tunable is admin-configurable via the existing provider admin UI; the manifest is the first-boot seed, exactly like `session_settings.seed_from_config_once`.)

### DEC-6: Test — reboot idempotency in compat (keyless) mode or with storage_key?
**Resolution:** Add `secrets.storage_key` (the harness key `test-storage-key-for-pgcrypto-min-32-chars-long`) to the `reboot()` config string so the second boot re-encrypts (matching production, where the key is always set). Without it the reboot process would run keyless and write the secret plaintext into config.
**Basis:** codebase — the main `TestServer` harness sets that key (`harness_inner.rs:553`); the `reboot()` config currently omits it. Aligning them keeps the idempotency test asserting the real (encrypted) production behavior. This is a test-local config change, not a shared-harness change (B3-safe — it edits the reboot helper's own config string, used only by desired_state tests).
