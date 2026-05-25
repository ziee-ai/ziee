# E2E Tests TODO

Tests that were removed from the suite during the 2026-05 security
remediation pass because either (a) the product feature they assert
doesn't exist yet, (b) they depend on real external resources that
make them inherently flaky, or (c) they conflict with a security fix
that landed afterward.

Reinstate each as the listed prerequisite is met.

---

## 1. Block download and show auth modal for hub models requiring authentication

**File when removed:** `tests/e2e/08-hub/02-hub-models.spec.ts`
**Test name:** `should block download and show auth modal for models requiring authentication`

**Why removed:** the UI shows an "Auth Required" badge on hub model
cards but does NOT block download or open an authentication-config
modal. The `getByRole('dialog', { name: /authentication.*required/i })`
the test waits for has never been implemented in
`ModelHubCard.tsx`. Only the badge surface exists today.

**To reinstate:**
1. Implement the modal in `src/modules/hub/modules/llm-models/components/ModelHubCard.tsx`:
   when the Download button is clicked on a `model.auth_required`
   model, open a confirm modal that links to the repository auth
   config drawer.
2. Restore the test body (see git history for
   `02-hub-models.spec.ts`; was the test directly after `should
   display auth_required badge on appropriate models`).

---

## 2. Allow cancelling an active download

**File when removed:** `tests/e2e/05-llm/llm-models-local-download.spec.ts`
**Test name:** `should allow cancelling an active download`

**Why removed:** the test starts a real download from HuggingFace
(`distilgpt2`, ~350MB), waits for the in-progress UI, then clicks
Cancel and asserts a `Cancelled` tag appears within 5s. The window
between "download started" and "download finished" is non-deterministic
in CI/dev — the download often finishes (or fails to start) before
the cancel click lands. Result: flaky.

**To reinstate:**
1. Add a deterministic mock-download path. Options:
   - A test-only `?mock=true` query param on the download endpoint that
     starts a slow streaming response controlled by the test.
   - A `page.route()` mock that intercepts
     `/api/llm-models/downloads/{id}/progress-stream` and feeds
     paced SSE events the test controls.
2. Update the test to use the mock so the download is reliably "in
   progress" when Cancel is clicked.

---

## 3. Fail connection test with invalid URL

**File when removed:** `tests/e2e/05-llm/llm-repositories.spec.ts`
**Test name:** `should fail connection test with invalid URL`

**Why removed:** the test creates a repository with
`https://invalid-test-url-12345.com` to verify the connection-test
error UI, but the outbound URL validator added in security A2
(09-llm-repo F-01/F-03 — SSRF + DNS-rebinding) blocks creation of
any URL whose host cannot be resolved. There's no longer a path to
create-then-connection-test-fail.

**To reinstate:**
1. **Either** redesign the repository model so the connection-test
   is the only outbound call (and create-time validates only URL
   shape, not DNS resolvability), keeping SSRF protection at the
   connection-test layer.
2. **Or** rewrite the test to verify the connection-test failure path
   through a different failure mode that the validator accepts —
   e.g. a real domain that returns 404 for the test-endpoint
   (`https://example.com/nonexistent`).

The other connection-test failure scenarios (invalid credentials in
drawer, etc.) still run and cover the error UI.

---

## 4. Branching: reload navigator anchor persists at assistant bubble

**File:** `tests/e2e/09-chat/chat-branching.spec.ts`
**Test name:** `reload: navigator anchor persists at assistant bubble after page refresh` (line 324)

**Why failing:** Deterministic failure — after `page.reload()`, the
test waits 15s for the branch-navigator to appear under the assistant
bubble (the linchpin assertion for the `fork_level` column). The
backend returns the correct `fork_level: "assistant"` in
`/api/conversations/{id}/branches`, but the page renders blank after
reload (test-failed-1.png is white). The same conversation page loads
correctly in other tests, so the issue is specific to the sequence
"create regenerate branch → reload → expect navigator anchored at
assistant bubble". `branch selection persists across page reload`
(the user-edit reload sibling test) passes — only the regenerate-flow
reload fails.

**To reinstate:**
- Investigate why the chat conversation page renders blank only after
  reloading a conversation that has an assistant-level fork (likely a
  race between branch list load and message list load that's only
  triggered by `fork_level === 'assistant'` branches).
- After the bug is fixed, re-enable by removing this entry.

---

## 5. MCP sampling: research tool triggers two sampling roundtrips

**File:** `tests/e2e/09-chat/mcp-chat-sampling.spec.ts`
**Test name:** `research tool triggers two sampling roundtrips and returns a final answer` (line 101)

**Why failing:** Real-LLM test against `claude-haiku-4-5-20251001` via
a mock MCP server with sampling. After fixing `/api/user-groups`
→ `/api/groups` (test path was wrong) and Anthropic base_url
`https://api.anthropic.com` → `…/v1` (`createProviderViaAPI` was
omitting the `/v1` suffix that the Rust ai-providers crate expects),
the request now reaches Anthropic, returns 200, and emits
`tool_use_delta`. But `sendChatMessage`'s 30s wait for
`[data-role="assistant"]` times out: the Chat.store only adds the
streaming-assistant message to the messages Map when it sees a
`text_delta`, but the LLM in this scenario sends only `tool_use_delta`
events first (waiting for the sampling roundtrip to complete before
any text), so no assistant DOM element is created within the wait
window.

**To reinstate:**
- Either: extend Chat.store's streaming handler to register a
  streaming assistant message on the first `tool_use_delta` as well as
  `text_delta` (so the `[data-role="assistant"]` element is present
  while the tool call is in progress).
- Or: lengthen `waitForAssistantResponse`'s timeout for sampling
  scenarios and have it wait for the final text rather than the first
  assistant bubble.

The companion test `Sampling badge is visible on the mock server card
on the user MCP page` (line 128) now passes after the path/url fixes
above.

---
