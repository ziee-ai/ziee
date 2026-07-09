# TEST_RESULTS — model-param-constraints

Backend-only diff → backend gate. Commands (from `src-app/`, `CARGO_TARGET_DIR` redirected because the
committed `src-app/target` symlink is a stale `/data/pbya` path in this env; `DATABASE_URL` unset →
harness auto-isolates on the `:54321` cluster):

```
cargo test -p ai-providers                 # 79 lib tests + doc/integration files
cargo test --lib -p ziee apply_model_params
cargo test -p ziee --test integration_tests -- --test-threads=1 stub_chat_tier2   # 6 passed
```

## Enumerated tests (from TESTS.md)
- **TEST-1**: PASS  (`adaptive_thinking_omits_sampling_on_allowed_model`)
- **TEST-2**: PASS  (`enabled_thinking_omits_sampling_keeps_budget`)
- **TEST-3**: PASS  (`no_thinking_allowed_model_keeps_temperature`)
- **TEST-4**: PASS  (`sonnet_5_thinking_adaptive_and_sampling_restricted`)
- **TEST-5**: PASS  (`dated_sonnet_5_resolves_to_base_and_stays_restricted`)
- **TEST-6**: PASS  (`sonnet_5_omits_sampling_via_registry`)
- **TEST-7**: PASS  (`is_unsupported_sampling_error_matches_only_unsupported` — hardened from the enumerated predicate test)
- **TEST-8**: PASS  (`strip_sampling_params_removes_only_sampling_keys`)
- **TEST-9**: PASS  (`clean_http_error_prefers_and_sanitizes_message`)
- **TEST-10**: PASS (`apply_model_params_maps_and_defaults`, streaming.rs — unset ⇒ temperature None)
- **TEST-11**: PASS (`stream_chat_self_heals_unsupported_sampling_400_and_retries_once` — loopback)

## Additional tests added during the audit (fix rounds)
- PASS `stream_chat_surfaces_invalid_value_400_without_retry` (no masking of value errors)
- PASS `stream_chat_retries_at_most_once_on_persistent_sampling_400` (bounded-retry guard)

## Cross-stack integration (stub provider)
- PASS `chat::stub_chat_tier2_test::empty_model_params_omit_temperature_and_default_max_tokens` (the
  regressed test, updated to the new no-forced-0.7 contract)
- PASS `model_params_reach_provider_request`, `thinking_*` (5 more) — 6/6 in the module.

`cargo check -p ziee` clean (only a pre-existing unrelated `mcp/repository.rs` dead-code warning).

## End-to-end reproduction of the two failures

A live docker + real-Anthropic reproduction (the task's `ZIEE_WEB_PORT` stack with Claude Sonnet 5 /
Sonnet 4.6) is **not runnable in this environment**: docker is not accessible to the shell and no
Anthropic credentials are present (no `ANTHROPIC_API_KEY`, none in config). The failure mechanisms and
fixes are instead reproduced deterministically at the layer that produces the 400:

- **Failure #2 (thinking + temperature → 400 "temperature may only be set to 1 …")** — reproduced by
  building the Anthropic request with adaptive/enabled thinking + temperature set; TEST-1/TEST-2 prove
  the assembled body now omits `temperature`/`top_p`/`top_k` (the exact 400 condition), and TEST-3
  proves non-thinking allowed models still send temperature.
- **Failure #1 (unknown `claude-sonnet-5` → sampling params rejected → 400)** — reproduced two ways:
  TEST-4/TEST-6 prove the registry now marks Sonnet 5 sampling-restricted so the param is dropped
  up-front; and **TEST-11 drives the full `stream_chat` path against a loopback mock Anthropic server
  that returns the real 400 body**, proving the self-heal strips the offending param, retries once,
  and yields a successful stream — the durable fix for any unknown/changed model.
