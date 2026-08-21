# DECISIONS — mcp-response-content-sniffing

### DEC-1: What signal decides SSE-vs-JSON framing?
**Resolution:** The response's `Content-Type` header, exclusively. No body-substring search survives in any framing branch.
**Basis:** codebase — `HttpMcpClient::request()` (`http.rs:1637`) already routes on `content_type.starts_with("text/event-stream")` and has never carried this defect. The MCP spec § Transports makes the header the declared framing. The defective site's own comment claims it parses "the same way `self.request()` does"; honouring that comment IS the fix. Body content is attacker-/third-party-controlled and cannot be a framing oracle.

### DEC-2: Delete the body heuristic entirely, or keep a fallback?
**Resolution:** Keep a fallback, but order it AFTER a strict JSON parse: (1) declared SSE → SSE extraction; (2) else strict JSON parse, success wins; (3) only on JSON-parse FAILURE, and only if structurally SSE-framed, extract as SSE.
**Basis:** convention — the pre-existing tolerance for a server that emits SSE framing under a wrong content-type is real defensive value and removing it would be an unrelated behaviour regression. Ordering it after the JSON parse makes the reported defect impossible *by construction* rather than by a better predicate: a body that is valid JSON is never reconsidered, so no tool content can misroute regardless of what it contains. This also satisfies the task's constraint that any retained heuristic be structural and predicate-matched.

### DEC-3: What is the fallback's structural predicate?
**Resolution:** `is_sse_framed(body)` — true iff some LINE begins with `data:` (covering both the spaced `data: ` and spec-legal no-space `data:` forms). Never `contains`.
**Basis:** convention — the EventSource spec defines `data` as a per-LINE field with an optional single space; the module's own `sse_event_data` (`http.rs:544`) already encodes exactly this rule. The predicate is the same `data:`-line test the extractor applies, so the two cannot disagree — which is precisely the property the defect violated.

### DEC-4: Should the parsing logic stay inline in the spawned async closure?
**Resolution:** No — extract it to a pure free function `parse_non_streaming_response_body(body, expected_id)` with in-source `#[cfg(test)]` coverage.
**Basis:** codebase — the defect shipped and survived because the logic sat inside a `tokio::spawn` closure inside a 100-line branch, where it was unreachable by any unit test; `http.rs` has zero `#[cfg(test)]` today. CODING_GUIDELINES §14 requires pure logic to carry in-source tests. Extraction is what makes the regression test possible at all, and it is why this fix is durable rather than a one-line patch that the next refactor re-breaks.

### DEC-5: Reuse `extract_response_by_id` or write new SSE extraction?
**Resolution:** Reuse `extract_response_by_id` (`http.rs:~220`). Write nothing new.
**Basis:** codebase — it already implements the EventSource rules correctly (both prefix forms, multi-line concatenation, CRLF normalization) and correlates by JSON-RPC id; `tool_call_id` is in scope at the call site. Adding a fourth `data:` parser is the exact root cause being fixed (three hand-rolled bypasses of two correct helpers — see `SIBLINGS.md`).

### DEC-6: Narrow the outer Content-Type dispatch at `http.rs:2741`?
**Resolution:** Yes — `contains("text/event-stream")` → `starts_with(...)` on the trimmed value.
**Basis:** convention — aligns with `request()`'s identical decision (DEC-1). `contains` on a header would also fire on a content-type that merely mentions the SSE type inside a parameter. Low risk: no conforming server sends such a header, and the previously-matching set is a strict superset of the new one only in that pathological case.

### DEC-7: Fix the two near-siblings (`http.rs:1860`, `:2253`) in this change, or only report them?
**Resolution:** Fix them, delegating both to the existing `sse_event_data` helper.
**Basis:** convention — they are not the reported defect (their predicate is line-structural, so they cannot misroute a JSON body), but each is a hand-rolled parallel implementation of an existing tested helper that silently drops two spec-legal shapes (no-space `data:`, multi-line `data:` blocks) and hardcodes a `&l[6..]` byte offset. The task explicitly flags the exact-`"data: "` prefix as fragile. The change is two call sites delegating to already-tested code, each covered by a real-path integration test (TEST-8/TEST-9), so the marginal risk is small and the marginal coverage is real.

### DEC-8: Does this change need a settings row / operational tunable?
**Resolution:** No. There is no tunable here — framing dispatch is a protocol correctness property, not an operational policy, and making it configurable would let an operator re-enable a defect.
**Basis:** convention — the configurable-settings rule targets resource limits, retention, quotas, toggles and model selection. A wire-format parsing rule is none of those; it is a security/correctness boundary of the kind the rule explicitly exempts.

### DEC-9: Is the `ai-providers` SSE reader in scope?
**Resolution:** No. Recorded in `SIBLINGS.md`, unchanged.
**Basis:** convention — it is a different subsystem (LLM provider streaming). Its streams are `text/event-stream` by construction of the provider contract, so it makes no framing *decision* and cannot exhibit this defect. Widening the diff into it would add risk without addressing the reported class.

### DEC-10: Which test tiers apply?
**Resolution:** unit (in-source, the extracted function) + integration (`tests/mcp/`, real `call_tool` over HTTP against `MockMcpServer`). No e2e, no `[negative-perm]` spec.
**Basis:** codebase — no frontend path is touched (phase-3 frontend e2e gate does not apply) and no permission is introduced (A9/A10 do not apply). `MockMcpServer::Raw`/`SseStream`/`SseRaw` already provide exactly the content-type and framing control both directions of the regression need, so the external boundary is the only thing mocked.
