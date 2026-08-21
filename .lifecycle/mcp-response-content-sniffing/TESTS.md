# TESTS — mcp-response-content-sniffing

Tiers mirror the repo convention: pure logic → in-source `#[cfg(test)]`; real
client-vs-server behaviour → `tests/mcp/` integration against `MockMcpServer`.
No frontend path is touched, so no `tier: e2e` is required (phase-3 frontend
gate does not apply). No permission is introduced, so no `[negative-perm]` spec
is required (A9/A10 do not apply).

Every claim below is proven against the REAL code path where one exists: the
integration tests drive `HttpMcpClient::call_tool` end-to-end over HTTP against
a programmable server, mocking only the external boundary (the MCP server
itself), per CODING_GUIDELINES §14 / [[feedback_no_cosmetic_tests]].

## Unit — the extracted parsing function

- **TEST-1** (tier: unit) [acceptance] [invariant: INV-2] [covers: ITEM-1, ITEM-4] file: `src-app/server/src/modules/mcp/client/http.rs` — asserts: a valid JSON-RPC body whose tool-result CONTENT contains the literal `data: ` (the reproduced paper title "Mobilizing the base of neuroscience data: the case of neuronal morphologies") parses as JSON and returns its `result` verbatim. This is the defect: the pre-fix code takes the SSE branch on this input and errors "No data found in SSE response".
- **TEST-2** (tier: unit) [acceptance] [invariant: INV-4] [covers: ITEM-1] file: `src-app/server/src/modules/mcp/client/http.rs` — asserts: predicate and extractor agree — over a table of bodies, `is_sse_framed` is FALSE for every body whose `data:` occurrences are all mid-line (so no such body can enter the SSE branch), and TRUE only when a LINE begins with `data:` (both the spaced and no-space forms), in which case extraction yields a payload rather than a "no data found" error.
- **TEST-3** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/client/http.rs` — asserts: control — an ordinary JSON-RPC body containing no `data:` at all still parses to the same result (proves the fix did not narrow the previously-working set).
- **TEST-4** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/client/http.rs` — asserts: the retained fallback works — a body that is NOT valid JSON but IS structurally SSE-framed (a line beginning with `data:`) under a non-SSE content-type is still recovered via the SSE extractor, so a mislabeled server keeps working.
- **TEST-5** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/client/http.rs` — asserts: a body that is neither valid JSON nor SSE-framed reports the JSON parse error, NOT "No data found in SSE response" — the error names the real problem.

## Integration — the real `call_tool` path over HTTP

- **TEST-6** (tier: integration) [acceptance] [invariant: INV-2] [covers: ITEM-1, ITEM-4] file: `src-app/server/tests/mcp/response_framing_test.rs` — asserts: the LITERAL end-to-end reproduction — a server answering `tools/call` with `Content-Type: application/json` and a result whose text content contains `data: ` yields that content through `HttpMcpClient::call_tool`, with no error.
- **TEST-7** (tier: integration) [acceptance] [invariant: INV-3] [covers: ITEM-1] file: `src-app/server/tests/mcp/response_framing_test.rs` — asserts: the counterpart — a genuine `text/event-stream` `tools/call` response still parses and returns its content. Without this the fix could have "passed" by breaking SSE entirely.
- **TEST-8** (tier: integration) [acceptance] [invariant: INV-3] [covers: ITEM-3, ITEM-4] file: `src-app/server/tests/mcp/response_framing_test.rs` — asserts: a genuine SSE `tools/call` response framed with the spec-legal NO-SPACE `data:` form parses and returns its content (pre-fix, `starts_with("data: ")` at `http.rs:2253` skips the line and the call hangs/fails).
- **TEST-9** (tier: integration) [acceptance] [invariant: INV-3] [covers: ITEM-3, ITEM-4] file: `src-app/server/tests/mcp/response_framing_test.rs` — asserts: a genuine SSE `tools/call` response whose payload is split across MULTIPLE `data:` lines in one event block is concatenated per the EventSource spec and parses (pre-fix, only the first fragment is read and JSON parsing fails).
- **TEST-10** (tier: integration) [acceptance] [invariant: INV-1] [covers: ITEM-2] file: `src-app/server/tests/mcp/response_framing_test.rs` — asserts: the `Content-Type` header is what decides the framing — the SAME JSON-RPC envelope delivered once as raw body under `application/json` and once SSE-framed under `text/event-stream` produces the SAME tool result. The body bytes do not determine the branch; the declared type does.

## Coverage map

| ITEM | covered by |
|---|---|
| ITEM-1 | TEST-1, TEST-2, TEST-3, TEST-4, TEST-5, TEST-6, TEST-7 |
| ITEM-2 | TEST-10 |
| ITEM-3 | TEST-8, TEST-9 |
| ITEM-4 | TEST-1, TEST-6, TEST-8, TEST-9 (the sweep's behavioural claims about D1/N1/N2 are each executable) |

| INV | pinned by `[acceptance]` |
|---|---|
| INV-1 | TEST-10 |
| INV-2 | TEST-1, TEST-6 |
| INV-3 | TEST-7, TEST-8, TEST-9 |
| INV-4 | TEST-2 |

## Mutation check (required by the task, recorded in TEST_RESULTS.md)

After green, the fix is reverted in place and TEST-1 + TEST-6 must go RED. A
test that stays green with the fix reverted proves nothing about the defect.
