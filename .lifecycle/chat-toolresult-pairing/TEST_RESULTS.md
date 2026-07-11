# TEST_RESULTS — Phase 8 (backend diff)

Backend-only diff (no `src-app/ui/**` / `src-app/desktop/ui/**` touched) → no frontend
`npm run check` / `gate:ui` gate applies. No new permission → no A9/A10 authz tests.

## Enumerated tests (all PASS)

- **TEST-1**: PASS — `group_assistant_blocks_pairs_partial_parallel_batch` (unit)
- **TEST-2**: PASS — `group_assistant_blocks_matched_parallel_batch_unchanged` (unit)
- **TEST-3**: PASS — `group_assistant_blocks_preserves_failed_tool_result` (unit)
- **TEST-4**: PASS — `group_assistant_blocks_drops_orphan_tool_result` (unit)
- **TEST-5**: PASS — `tests/chat/assistant_block_grouping_test.rs` integration:
  `partial_parallel_batch_synthesizes_missing_and_stays_valid`,
  `multi_iteration_single_message_stays_valid` (+ existing
  `trailing_tool_use_without_result_is_emitted_as_assistant`,
  `parallel_tool_uses_then_another_tool_use_groups_per_iteration`,
  `corrupted_interleaving_still_pairs_every_tool_use` all green — no regression)
- **TEST-6**: PASS — `apply_block_snaps_cut_past_orphan_tool_result` (summarizer unit)
- **TEST-7**: PASS — `group_assistant_blocks_drops_mid_stream_orphan_result` (unit)
- **TEST-8**: PASS — `group_assistant_blocks_dedups_duplicate_result` (unit)

## Broader regression sweep (touched modules)

- `cargo test --lib -p ziee chat::core::services::streaming:: summarization::` — **74 passed, 0 failed**.
- `cargo test --test integration_tests summarization:: chat::assistant_block_grouping
  chat::test_single_assistant_message_architecture chat::append_content_ordering` —
  48 passed. **3 failures are ENVIRONMENTAL, not regressions:**
  - `chat::…::test_single_assistant_message_with_tool_execution` and
    `summarization::real_llm_test::custom_full_summary_prompt_is_sent_to_the_model` are
    **real-LLM tests** — `tests/.env.test` carries no provider API key on this host, so they
    fail their own precondition ("No AI provider API keys found" / provider-create setup).
    Expected per CLAUDE.md (real-LLM tests skip/fail without keys).
  - `summarization::after_llm_call_test::after_llm_call_skips_summary_for_brand_new_short_branch`
    failed only because the `stub-engine` binary wasn't built and the harness resolves it via
    the committed-broken `src-app/target` symlink. After `cargo build -p stub-engine` + the
    worktree target symlink, the non-real-LLM summarization suite (`after_llm_call_test`,
    `per_conversation_mode_test`, `failsoft_test`, `admin_settings_test`) is **23 passed, 0 failed**.

## Live end-to-end repro (per the live-symptom verification rule)

Booted the **fixed** server binary on `127.0.0.1:18099` (embedded PG, scratch data dirs),
pointed it at the running local vLLM `openai/gpt-oss-120b` (`127.0.0.1:8001`, OpenAI adapter),
attached the built-in `fetch` MCP tool.

1. Sent a turn asking the model to fetch two unreachable URLs. The model produced an assistant
   message with the exact malformed shape from the bug report — **co-located
   `thinking, tool_use, tool_result(is_error "Failed to fetch"), thinking, tool_use,
   tool_result(is_error), thinking, text`** (2× tool_use + 2× tool_result in ONE assistant
   message).
2. Sent the **next** turn — the send that 400'd pre-fix. It generated a normal assistant
   response (thinking + text). The server log shows **no** `tool_use ids were found without
   tool_result`, **no** `AI provider error`, **no** 400 — the assembler split the co-located
   message into a valid `[assistant{…tool_use×2}, tool{tool_result×2}]` pair sent to vLLM.

Confirms the fix removes the symptom against the real model-driven flow, not just unit tests.
(The original RCPA/DSCC 400 additionally involved the summarizer boundary on a long
conversation — reproduced deterministically by TEST-6; the RCPA/DSCC cause-side is the
`stale-artifact-links` worker's domain.) Server torn down after the run.
