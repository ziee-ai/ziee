# stale-artifact-links — worker status: DONE (lifecycle 9/9, live repro PASS, PR open)

Branch: `fix/stale-artifact-links` (off `khoi`; PR target `khoi`).
PR: https://github.com/ziee-ai/ziee/pull/138 (base: khoi). NOT merged — human to review.
Worktree: `/data/khoi/home-workspace/ziee/tmp/stale-artifact-links-wt`.
One clean commit (3 source files); `.lifecycle` stripped from the tip; authored khoi.

## Root cause (pinned via 3 independent code traces — NOT workspace loss)
The reference the LLM reuses across turns is a **download URL whose JWT token has a
short absolute mint-time expiry** (1h for tool-produced artifacts,
`chat_extension/mcp.rs:959-994` + dup; 5min for `get_resource_link` attachments,
`code_sandbox/tools/files.rs:493-509`). It is stored in the tool-result
`hidden_content`, persisted, and **replayed verbatim on every later turn**
(`content.rs:150-153`) — the token is never re-signed. Past TTL,
`download_with_token` returns 401 → the external MCP tool (RCPA/DSCC) surfaces
"Failed to fetch". The file bytes + the per-conversation sandbox workspace both
survive (30-day reaper); only the embedded credential goes stale — which is why
re-running `get_resource_link` (fresh token) fixes it.

## Fix (guidance/prompt only — token is short-lived BY DESIGN)
Corrected the three guidance sites so the model treats these URLs as ephemeral and
re-obtains a fresh link via `get_resource_link` when handing a file to a tool in a
later turn (never reuse an earlier-turn URL):
1. `get_resource_link` tool description (`code_sandbox/handlers.rs::tool_definitions`)
2. the two saved-artifact `hidden_content` blocks → extracted to one shared helper
   `saved_artifact_hidden_content_guidance` (`mcp/chat_extension/mcp.rs`)
3. `tool_system_guidance` (`mcp/chat_extension/mcp.rs`)
No token/TTL, `persist_links`, or #131 SSRF change. Scope: token-staleness only.

## Coordination with `chat-toolresult-pairing` (the EFFECT worker)
Different files (they fix the malformed-history/tool_result assembler). This fix
does NOT change any tool-result FAILURE text — the corrected guidance is on the
SUCCESS-path hidden_content + tool descriptions. So there is no failure-text overlap
to reconcile; nothing here changes what a failed tool emits.

## Progress — DONE (lifecycle 9/9, live repro PASS)
- [x] Lifecycle phases 1–9 gated green (`--all --base khoi`): plan, plan-audit,
      tests, decisions, implement+drift(0), blind audit (12 angles/hunk), fix loop
      (0 new confirmed), test results, human feedback.
- [x] Unit TEST-1/2/3 + integration TEST-4 PASS; `cargo check -p ziee --lib --tests` clean.
      (3 unrelated stub-engine tests 401'd only on a missing `src-app/target` symlink —
      worktree env, not the diff; green after the symlink.)
- [x] **Live repro PASS** (container on :8090, never 8080; live vLLM gpt-oss-120b +
      RCPA MCP): reproduced the 401 "Failed to fetch" on the artifact download URL
      passed to RCPA `validate_input_file`, and the model — reading the corrected
      `get_resource_link` guidance served verbatim by the fixed binary — RECOVERED by
      re-fetching a fresh link until the fetch succeeded (HTTP 200, real validation).
      Root cause of the 401s here: gpt-oss mis-copies the ~400-char opaque token
      (truncation / char-substitution); same symptom + same remedy as the expiry case.
      Full write-up: `.lifecycle/stale-artifact-links/LIVE_REPRO.md`. Stack torn down;
      live 8080/8097 + RCPA/DSCC untouched.
- [x] `.lifecycle` stripped in the final tip; PR opened vs `khoi`.

**Observation for review (out of scope, DEC-1 guidance-only):** the long opaque
download token is fragile for weak local models to copy verbatim — a shorter/robust
token or a token-free file handle would remove the copy-error 401s at the source.
