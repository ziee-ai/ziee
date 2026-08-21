# Design — MCP HTTP client response framing dispatch

## Why this doc exists

This is a bugfix, and no prior design doc governs the decision it gets wrong.
Per the lifecycle rule ("if there is genuinely no prior design doc, WRITE one
and name it"), this records the framing contract the client must honour, lifted
from the two authorities that already govern it:

- **MCP specification § Transports (Streamable HTTP)** — a server answers a
  JSON-RPC POST with EITHER `Content-Type: application/json` (a single JSON-RPC
  response) OR `Content-Type: text/event-stream` (an SSE stream carrying the
  response, possibly interleaved with notifications/requests).
- **WHATWG EventSource / SSE specification** — an event block is a set of
  `field: value` LINES. The payload field is `data`, written `data:` with an
  OPTIONAL single leading space, and a block MAY carry MULTIPLE `data:` lines
  which are concatenated with `\n`.

There is also binding in-repo precedent: `HttpMcpClient::request()`
(`http.rs:1637`) already routes on `content_type.starts_with("text/event-stream")`
and nothing else, and the two SSE payload helpers `extract_response_by_id`
(`http.rs:~220`) and `sse_event_data` (`http.rs:~537`) already implement the
EventSource `data:` rules correctly, including the no-space form and multi-line
concatenation.

## The contract

**The framing of a response is declared by the sender, in the `Content-Type`
header. It is not a property of the bytes, and it is not inferable from them.**

A tool result is arbitrary third-party text. It can contain any byte sequence,
including sequences that look like SSE framing — a paper title, a log excerpt, a
code snippet, a YAML fragment, a chat message quoted back. Any decision
procedure that inspects the body to guess its framing will therefore be wrong on
some legitimate content, and will be wrong *silently*, because the failure
surfaces as an empty tool result rather than an error the user can act on.

## Invariants

- **INV-1**: The SSE-vs-JSON decision is made from the response's `Content-Type`
  header, never by searching the response body for a substring.
- **INV-2**: A plain-JSON MCP tool result whose *content* contains the literal
  `data: ` parses successfully and returns its result unchanged.
- **INV-3**: A genuine `text/event-stream` response still parses, including the
  spec-legal no-space `data:` form and multi-line `data:` blocks.
- **INV-4**: Where a body-based framing heuristic is retained as a fallback, its
  branch predicate and its payload extractor are the same test — a body that
  enters the branch always yields a payload — and the predicate is structural (a
  LINE beginning with `data:`), never a substring search.

## Consequences for the fallback

The client historically tolerated a server that emits SSE framing under a
non-SSE content-type. That tolerance is worth keeping, but it must not be able
to capture a valid JSON body. So the fallback is **ordered after** a strict JSON
parse:

1. `Content-Type` declares `text/event-stream` → SSE extraction.
2. Otherwise attempt a strict JSON parse. Success → done.
3. Only if that parse FAILED, and only if the body is structurally SSE-framed
   (a line beginning with `data:`), extract as SSE.

Step 2 preceding step 3 is what makes INV-2 hold *by construction*: a body that
parses as JSON is never reconsidered as SSE, so no tool content can misroute,
whatever it contains. And because step 3's predicate is "the extractor found a
`data:` line", the predicate and the extractor cannot disagree (INV-4).
