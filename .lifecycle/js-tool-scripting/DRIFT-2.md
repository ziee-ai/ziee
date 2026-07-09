# DRIFT-2 — js-tool-scripting (phase-8 plan amendment)

A single late drift surfaced while running the phase-8 e2e tier: the plan
enumerated a gated real-LLM e2e (TEST-36), but the only real LLM reachable from
this environment is offline, so the test cannot be a deterministic gate. Resolved
by amending TESTS.md (impl/reality wins over the plan) with the capability's
provider-independence re-covered by existing passing tests.

- **DRIFT-2.1** — verdict: resolved — TEST-36 (gated real-LLM e2e) was temporarily
  descoped because the vLLM engine behind the local bridge (`127.0.0.1:8000`) was
  offline (completions 500 "Cannot connect to host 127.0.0.1:8000"; shared GPU box —
  [[reference_local_llm_coder_ziee]]). The engine was subsequently brought up; the
  spec was RE-RUN against `qwen3.6-35b-a3b` and **passed GREEN** (1 passed, no retries):
  the real model chose `run_js`, the embedded QuickJS runtime executed it end-to-end
  (`ToolUse(run_js)`→`ToolResult(run_js)`), the card reached `completed`, and the answer
  reflected 6*7=42. TEST-36 is therefore **RE-SCOPED back into the gated enumeration**
  (a proper `- **TEST-36** (tier: e2e)` line in TESTS.md) with `TEST-36: PASS` recorded.
  It self-skips without `OPENAI_BASE_URL`/`ZIEE_TEST_LLM_MODEL` (the repo's real-LLM
  convention), so plain CI runs won't hard-fail on engine availability.

  (Historical note: before the engine came up, a stale `macros` proc-macro cache from
  intervening `cargo test --lib` runs made the e2e warmup `cargo build --bin ziee` fail
  with `no variant RunJsApprovalRequired` — the [[project_macros_stale_chat_extensions]]
  gotcha; cleared with `cargo clean -p macros`.)

**Unresolved drifts:** 0
