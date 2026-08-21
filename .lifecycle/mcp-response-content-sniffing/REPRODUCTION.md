# REPRODUCTION — verbatim, before any code change

Two independent reproductions were taken **before** the fix was written: the
live rig (corroboration, non-deterministic) and a deterministic test that drives
the real client path (the regression proof). Both are recorded here; only the
second is what the fix is gated on.

## A. Live rig corroboration (`127.0.0.1:29500`)

The server's own response is correct — this is entirely a client-side misread.

```
$ curl -s -D - -X POST http://127.0.0.1:29500/api/citations/mcp \
    -H "Authorization: Bearer $T" \
    -H 'Content-Type: application/json' \
    -H 'Accept: application/json, text/event-stream' \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/call",
         "params":{"name":"list_citations","arguments":{}}}'

HTTP/1.1 200 OK
content-type: application/json
```

Note the client sent `Accept: application/json, text/event-stream` and the
server still answered `application/json` — so `Content-Type` is authoritative
and available at the decision point.

Analysis of that exact 193,956-byte body:

```
body bytes                       : 193956
line count                       : 1
contains("data: ")               : True
any line.startswith("data: ")    : False
any line.startswith("data:")     : False
valid JSON                       : True
first "data: " at offset         : 74437
surrounding context              : ...ar":"2007"},{"DOI":"10.1038/nrn1885","article-title":
                                   "Mobilizing the base of neuroscience data: the case of
                                   neuronal morphologies","author":"Ascoli","...

>>> OLD predicate picks SSE branch: True
>>> OLD extractor finds a line    : False
>>> RESULT: AppError("No data found in SSE response")
```

The `data: ` occurrence is at offset 74,437, **inside the title of the Ascoli
paper** (`10.1038/nrn1885`). The body is a single line of valid JSON, so no line
can begin with `data:` — the branch predicate (`contains`, anywhere) and the
payload extractor (`strip_prefix`, line-start) disagree, and the call dies.

## B. Deterministic reproduction (the regression test)

`src-app/server/tests/mcp/response_framing_test.rs` drives the real
`HttpMcpClient::call_tool` over a real TCP socket against `MockMcpServer`,
mocking only the MCP server itself. The fixture reproduces the *shape* above
rather than depending on rig data:

- a single-line JSON-RPC `tools/call` result,
- whose text content is the Ascoli title (so `data: ` appears MID-LINE),
- served with `Content-Type: application/json`.

The test asserts the fixture's own defect-triggering properties before using it,
so it cannot silently stop reproducing the defect:

```rust
assert!(body.contains("data: "), ...);
assert!(!body.lines().any(|l| l.starts_with("data:")), ...);
```

The pre-fix result of this suite is recorded verbatim in `TEST_RESULTS.md`
(§ RED), alongside the post-fix run (§ GREEN) and the mutation check.

The discriminating property to look for in that RED run: the plain-JSON tests
FAIL while the genuine-SSE counterpart PASSES. A suite that failed uniformly
would only prove the harness was broken.
