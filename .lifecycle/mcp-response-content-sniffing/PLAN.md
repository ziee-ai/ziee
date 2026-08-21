# PLAN — fix MCP HTTP client response content-sniffing

## Design source

Realizes `.lifecycle/mcp-response-content-sniffing/DESIGN.md` § *The contract* and
§ *Consequences for the fallback*, which in turn record the MCP specification
§ Transports (Streamable HTTP) framing rules, the WHATWG EventSource `data:`
field rules, and the binding in-repo precedent at
`src-app/server/src/modules/mcp/client/http.rs:1637` (`request()` routes on
`content_type.starts_with("text/event-stream")` alone).

## Invariants

- **INV-1**: The SSE-vs-JSON decision is made from the response's `Content-Type` header, never by searching the response body for a substring.
- **INV-2**: A plain-JSON MCP tool result whose *content* contains the literal `data: ` parses successfully and returns its result unchanged.
- **INV-3**: A genuine `text/event-stream` response still parses, including the spec-legal no-space `data:` form and multi-line `data:` blocks.
- **INV-4**: Where a body-based framing heuristic is retained as a fallback, its branch predicate and its payload extractor are the same test — a body that enters the branch always yields a payload — and the predicate is structural (a LINE beginning with `data:`), never a substring search.

## Items

- **ITEM-1**: Replace the `trimmed.contains("data: ")` body sniff in `call_tool`'s non-streaming Branch 3 (`http.rs:~2765`) with a JSON-first parse plus a structural, extractor-matched SSE fallback. Extract the logic into a pure, directly-testable free function rather than leaving it inline in the spawned async closure (it is currently untestable, which is why the defect shipped).
- **ITEM-2**: Align `call_tool`'s outer Content-Type dispatch (`http.rs:~2741`) from `contains("text/event-stream")` to `starts_with(...)` on the trimmed value, matching the `request()` precedent, so a content-type merely *mentioning* the SSE type in a parameter cannot route a JSON response into the streaming branch.
- **ITEM-3**: Replace the hand-rolled `.find(|l| l.starts_with("data: ")).map(|l| &l[6..])` payload extraction in `call_tool_with_sampling` (`http.rs:~1860`) and `call_tool_with_elicitation` (`http.rs:~2253`) with the existing spec-correct `sse_event_data` helper. These are not the reported defect (they are structural, not `contains`), but they are hand-rolled parallel implementations of an existing tested helper and they silently drop two spec-legal shapes: `data:` with no space, and multi-line `data:` blocks.
- **ITEM-4**: Record the complete sibling sweep across the MCP client (and the wider repo) in `SIBLINGS.md`, naming every instance of the framing-decision shape and its disposition.

## Files to touch

- `src-app/server/src/modules/mcp/client/http.rs` — the fix (ITEM-1/2/3) + in-source `#[cfg(test)]` unit tests for the extracted function.
- `src-app/server/tests/mcp/response_framing_test.rs` — new integration tests driving the real `call_tool` path against `MockMcpServer` (both directions).
- `src-app/server/tests/mcp/mod.rs` — register the new test module.
- `.lifecycle/mcp-response-content-sniffing/SIBLINGS.md` — ITEM-4.

## Patterns to follow

- **Content-Type dispatch** — mirror `HttpMcpClient::request()` (`http.rs:1637`), the existing correct implementation of exactly this decision.
- **SSE payload extraction** — reuse `extract_response_by_id` (`http.rs:~220`) and `sse_event_data` (`http.rs:~537`); both already implement the EventSource `data:` rules (no-space form, multi-line concatenation, CRLF). Do not hand-roll a third.
- **Mock-driven client tests** — mirror `tests/mcp/conformance_phase1_test.rs`: `server_config(mock.base_url())` → `HttpMcpClient::new` → `connect()` → drive. Use `MockResponse::Raw { content_type, body }` for the declared-JSON direction and `MockResponse::SseStream` for the genuine-SSE direction.
- **In-source unit tests** — `#[cfg(test)] mod tests` at the foot of the module, per CODING_GUIDELINES §14 ("pure logic → in-source `#[cfg(test)]`").

## Non-goals

- The `ai-providers` SSE reader (`ai-providers/src/providers/sse.rs:139`, `anthropic.rs:695`) is a different subsystem (LLM provider streaming, not MCP) and is out of scope. It is recorded in `SIBLINGS.md` for completeness.
- Test-side `data:` parsing helpers are test infrastructure, not the client, and are out of scope (B3: do not edit shared harness to route around a feature's problem).

---

## Plan audit (phase 2 — audited against the codebase)

### Breakage risk

`call_tool` Branch 3 currently succeeds on any body that is valid JSON and does
NOT contain `data: `; that set is a strict subset of the bodies the new
JSON-first parse succeeds on, so no currently-working call can regress. The
bodies whose behaviour CHANGES are exactly (a) valid JSON containing `data: `
(today: hard error — the defect; after: parses) and (b) non-JSON SSE-framed
bodies (today: parsed if a `data: ` line exists; after: same, via the same
extractor). ITEM-2 narrows the outer dispatch; the only inputs affected are
content-types that mention `text/event-stream` other than as the leading type,
which no conforming server sends.

ITEM-3 changes which `data:` lines are read inside an already-SSE stream: it
widens acceptance (adds the no-space form and multi-line blocks). A stream that
parses today still parses.

### Pattern conformance

- **ITEM-1** — verdict: PASS — the extracted function mirrors `request()`'s dispatch and delegates to `extract_response_by_id`, the helper `request()` itself uses. It adds no third parsing implementation.
- **ITEM-2** — verdict: PASS — `starts_with` is verbatim the predicate `request()` uses at `http.rs:1637`.
- **ITEM-3** — verdict: PASS — replaces hand-rolled logic with `sse_event_data`, the module's own helper, whose doc-comment already states the exact semantics required.
- **ITEM-4** — verdict: PASS — documentation only.

### Migration collisions

None — this change adds no migration. `BASE.md` records the current server max
(`202607200200`) for completeness only.

### OpenAPI regen

None required. The change is entirely internal to the MCP client transport; no
handler signature, request/response type, or `JsonSchema` derive is touched, so
`openapi.json` and `api-client/types.ts` are unaffected in both workspaces. No
frontend file is touched, so the phase-3/phase-8 frontend gates do not apply.
