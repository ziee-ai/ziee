# SIBLINGS — every framing-decision / `data:`-extraction site (ITEM-4)

Sweep method (run against the worktree at `origin/main` `7ca09a750`):

```
grep -rn 'contains("data:\|contains("text/event-stream\|starts_with("text/event-stream\
\|starts_with("data:\|strip_prefix("data:' --include=*.rs src-app/
```

`src-app/*/target/` excluded. Both the MCP client directory and the whole
`src-app` Rust tree were swept, so the "is there more than one" question is
answered repo-wide, not just in `mcp/client/`.

## The defect — exactly one instance in production code

| # | site | shape | disposition |
|---|---|---|---|
| **D1** | `mcp/client/http.rs:2765` | `trimmed.contains("data: ")` selecting the branch, `line.strip_prefix("data: ")` (line-structural) doing the extraction | **FIXED (ITEM-1)** |

`http.rs:2765` is the **only** place in production code where a framing branch
is selected by a substring search over a response body. The predicate
(`contains`, anywhere) and the extractor (`strip_prefix`, line-start) do not
agree, so a valid-JSON body containing `data: ` anywhere in its *content* enters
the SSE branch, finds no `data: `-prefixed line, and dies on
`"No data found in SSE response"`.

## Content-Type dispatch sites — both correct in kind, one narrowed

| # | site | shape | disposition |
|---|---|---|---|
| S1 | `http.rs:1637` (`request()`) | `content_type.starts_with("text/event-stream")` | **Correct — unchanged.** This is the precedent the fix mirrors; it is the path used by `initialize`, `tools/list`, `resources/*`, `prompts/*` and every other non-tool-call request, and it has never had the defect. |
| S2 | `http.rs:2741` (`call_tool`, outer) | `content_type.contains("text/event-stream")` | **NARROWED (ITEM-2)** → `starts_with`. Not the reported defect — it inspects the *header*, which is the authoritative signal, not the body. But `contains` on a header would also match a content-type that merely mentions the type in a parameter; `starts_with` matches `request()`. |

Note S2 is what makes D1's severity clear: `call_tool` had **already** dispatched
correctly on `Content-Type` at S2, and D1 sits *inside its `else`* — i.e. the
body sniff was a second, contradictory guess at a question already answered. Its
own comment claims it parses "the same way `self.request()` does", which S1 shows
it did not.

## Near-siblings — structural, not the defect, but spec-lossy

| # | site | shape | disposition |
|---|---|---|---|
| N1 | `http.rs:1860` (`call_tool_with_sampling`) | `.find(\|l\| l.starts_with("data: ")).map(\|l\| &l[6..])` | **FIXED (ITEM-3)** → `sse_event_data` |
| N2 | `http.rs:2253` (`call_tool_with_elicitation`) | identical to N1 | **FIXED (ITEM-3)** → `sse_event_data` |

These are **not** the reported defect: the predicate is line-structural
(`starts_with`), so they cannot misroute a JSON body. They are recorded and
fixed because each is a hand-rolled parallel implementation of an existing,
tested helper, and each silently drops two spec-legal shapes:

1. `data:` with **no space** — the EventSource spec makes the space optional;
   `starts_with("data: ")` skips such a line entirely, and with no other data
   line the event is discarded as a keep-alive.
2. **Multi-line `data:` blocks** — `.find()` takes only the FIRST data line, so a
   payload split across lines is truncated to its first fragment and then fails
   JSON parsing.

The hardcoded `&l[6..]` offset is safe today only because `"data: "` is six ASCII
bytes; it is a byte-index into a `str` and is the kind of coupling that breaks
the moment the prefix is changed.

## Already-correct helpers — the reference implementations

| # | site | disposition |
|---|---|---|
| C1 | `http.rs:230-234` (`extract_response_by_id`) | **Correct — unchanged.** Handles `data: ` and `data:`, concatenates multiple data lines, normalizes CRLF, and correlates by JSON-RPC id. The fix delegates to this. |
| C2 | `http.rs:544` (`sse_event_data`) | **Correct — unchanged.** Handles both prefix forms and joins multi-line data. ITEM-3 delegates N1/N2 to this. |

The module therefore already contained two correct implementations of the rule
D1/N1/N2 each got wrong in its own way — the defect class here is *not* a missing
capability, it is three hand-rolled bypasses of existing correct code.

## Out of scope — different subsystem

| site | note |
|---|---|
| `ai-providers/src/providers/sse.rs:139` | LLM-provider SSE reader, not MCP. `strip_prefix("data: ")` on a stream that is genuinely `text/event-stream` by construction (the provider contract), so there is no framing *decision* to get wrong. Recorded for completeness; unchanged. |
| `ai-providers/src/providers/anthropic.rs:695` | Same as above. |

## Test-side helpers — deliberately untouched

~20 `strip_prefix("data:")` sites across `src-app/server/tests/**` (voice,
workflow, hardware, llm_model, code_sandbox, sync, chat_stream_probe). These are
test harness code reading known-SSE fixtures, not the client under test. Editing
shared harness to accommodate a feature is refused by lifecycle rule B3, and none
of them makes a framing decision. Unchanged.
