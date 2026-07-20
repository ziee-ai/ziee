# FIX_ROUND-1

## What happened to the blind round (recorded, not hidden)

Three blind subagents were spawned with diff-only context covering 13 angles
(correctness, concurrency, error-handling, sql-correctness / security,
perms-authz, api-contract, log-injection, secrets / test-reality, a11y,
patterns-conformance, state-management, maintainability, scale-performance).
**All three went idle without returning findings**, across four explicit
requests. The audit was therefore carried out by the author.

That is genuinely weaker than the blind round the process calls for — an author
auditing their own diff shares the blind spots that produced it. It is recorded
as a `process` finding in `LEDGER.jsonl` rather than presented as a clean pass.
Anyone reviewing this PR should treat the audit as self-review.

## Confirmed findings fixed this round

Four real defects, three of them in code written earlier in this same change:

1. **Flaky SSE-ordering assertion** (high) — `title_approval_test.rs`. Claimed
   `titleUpdated` always precedes the terminal frame; the driver multiplexes the
   chunk stream and the extension channel through one `tokio::select!`, so the
   order is arbitrary. Observed failing under `--test-threads=4` while the title
   was correctly persisted. Assertion removed with the reasoning documented;
   verified stable 5/5 afterwards.
2. **e2e seeded messages would 422** (high) — `untitled-conversation-label.spec.ts`.
   `seedConversation` omitted `model_id`, which `SendMessageRequest` marks
   required, so every seeded message would have failed and all four specs would
   have failed with a misleading "row shows Untitled" assertion. Now seeds a
   provider + model and fails loudly on a non-ok response.
3. **Log forging** (medium) — `mcp.rs`. The new diagnostic rendered third-party
   tool names unescaped; a name containing newlines could forge log lines. Now
   uses the existing `sanitize_prompt_field`.
4. **Unbounded log line** (medium) — `mcp.rs`. The same diagnostic rendered every
   advertised tool; capped at 40 with the omitted count reported.

## Reviewed and confirmed sound (no change)

The LATERAL join's effect on the `COUNT` aggregate and `GROUP BY`; owner-scoping
of the newly-exposed message content; hook placement in both break arms and the
deliberate non-coverage of the error/failsafe breaks; the swallow-and-log
contract of the new fan-out; the additive wire change plus dual OpenAPI regen;
absence of a Rules-of-Hooks reactive-read hazard; the a11y effect of the derived
`aria-label`.

**New confirmed findings:** 0
