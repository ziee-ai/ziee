# Office Bridge — Tool Surface Design Note

Handoff note on how to expose Office.js capability to the LLM without drowning it
in tool schemas. Context: the `office_bridge` desktop module (built-in MCP server,
`office_bridge.ziee.internal`) currently ships 7 tools. The open question is how to
grow toward "everything Office.js supports, and maybe more" (formatting, ranges,
tables, slides, find/replace, …) without the tool list exploding.

Grounded in the current Claude tool-use mechanisms (Tool Search Tool, Programmatic
Tool Calling, code execution, MCP connector).

---

## The problem: don't make one MCP tool per Office.js API

Every tool's JSON schema sits in the model's context **on every request**. Thousands
of Office.js methods → thousands of schemas → a huge fixed token cost before the user
even asks anything, AND selection accuracy degrades as the list grows (the model must
pick the right needle from a giant haystack). So "expose all of Office.js as flat MCP
tools" is the one design to rule out.

## Three real levers Claude gives you

1. **Tool Search Tool** (`tool_search_tool_regex_20251119` / `tool_search_tool_bm25_20251119`).
   Define the full catalog but mark the bulk `defer_loading: true`. Their schemas stay
   OUT of context; the model searches the catalog and only the relevant handful get
   loaded per request. Loaded schemas are **appended, not swapped**, so it doesn't blow
   the prompt cache. Constraint: the search tool itself and ≥1 real tool must stay
   non-deferred (else 400). → Use this if you genuinely want a large **typed** catalog.

2. **Programmatic Tool Calling (PTC) / code execution.** Instead of round-tripping every
   op through context, the model writes a *script* (runs in the code-execution sandbox)
   that calls tools as **functions**; tool results flow back into the running code, not
   the context window; only the **final** output returns to the model. Token cost scales
   with the final result, not the intermediate churn. → Ideal for "loop over 200 cells
   and bold the ones matching X."

3. **Keep the tool surface coarse and parameterized** ("bash vs. dedicated tools"
   heuristic): a few high-level tools with rich arguments, not one tool per primitive.

## Recommendation for office_bridge: collapse Office.js into a code surface

The key insight: **the task pane is already a JavaScript runtime with the full Office.js
API loaded.** So the highest-leverage design is not "enumerate Office.js as tools" — it's
**one `run_office_js` tool** where the model writes Office.js and the pane executes it:

```
Tool: run_office_js
  input: { doc_full_name, script }
  // e.g. script = "const r = context.workbook.getSelectedRange();
  //                r.format.font.bold = true; await context.sync();"
```

The broker (`bridge/broker.rs` + the pane duplex in `bridge/server.rs`) already does
daemon↔pane JSON-RPC — this is a natural new pane op: `taskpane.js` `dispatchOp` gains a
handler that runs the provided script inside `Word.run`/`Excel.run` and returns the
result. Benefits:

- **"Everything Office.js supports, and maybe more" — by construction.** Anything the API
  can do, the model can write. No catalog to maintain, no schema drift.
- **~One tool schema of context cost**, not thousands.
- **The model already knows the API.** Office.js is well-represented in training data —
  the model writes it about as well as it writes bash.
- Composes with **PTC**: multi-step manipulations (read → filter → format → sync) run as
  one script that never round-trips intermediates.

Then, ON TOP of that, keep a **small set of dedicated typed tools** for exactly the
operations that deserve guardrails — the "when to promote to a dedicated tool" criteria:
**gating** (`add_comment`, `set_track_changes` — hard to reverse, want per-call
approval), **staleness/consistency** checks, or **custom chat-UI rendering**. That's
essentially the 7 tools already shipped.

**Shape:** one open-ended code tool for breadth + a few gated typed tools for the ops
that need approval/rendering.

## Two caveats that matter for the architecture

- **PTC is NOT compatible with MCP-connector tools** (documented). PTC exposes tools as
  callable functions inside the code-execution sandbox via `allowed_callers:
  ["code_execution_20260120"]` on a **custom** tool; MCP tools are resolved server-side
  over the MCP channel and aren't callable from the sandbox. office_bridge is a built-in
  MCP server today — to get PTC's batching benefit for a `run_office_js` surface you'd
  expose it as **custom tools with `allowed_callers`**, not (only) over MCP. Not
  either/or: keep MCP for the gated/approved tools, offer the code surface as custom
  tools where the token savings matter. (PTC is also incompatible with `strict: true`,
  `disable_parallel_tool_use`, and forced `tool_choice`.)
- **`run_office_js` is arbitrary code execution in the pane** — more powerful and riskier
  than the typed tools; it can touch anything in the user's open document. Mitigations,
  most already present: it runs only in the user's own same-origin pane against their own
  open doc, behind the per-session token; keep it **behind per-call approval** (like the
  mutating tools today — deliberately NOT in `is_builtin_server_id`); consider a
  read-only vs. read-write split so "read" scripts auto-approve and "write" scripts
  prompt.

---

## MCP vs PTC — they're on different axes (not "vs")

**MCP (Model Context Protocol)** — a **transport / integration** protocol: *where tools
live and how they're wired into the model.* An MCP server advertises tools (`tools/list`);
a tool call is proxied over the MCP channel to the server, run, and the result fed back
into the model's context. Answers: "how do I expose a capability in a standard, reusable
way?"

**PTC (Programmatic Tool Calling)** — a **calling pattern / execution mode**: *how the
model invokes tools.*
- Normal tool use (MCP or custom): each call is a round trip — `tool_use` → harness runs
  it → **result lands in the model's context** → model reasons → next call. Every
  intermediate result costs context tokens.
- PTC: the model writes a script (runs in the code-execution sandbox) that calls tools as
  functions; the **result returns to the running code, not the context**. The model only
  sees the script's **final** output.

| | MCP | PTC |
|---|---|---|
| What it is | How a tool is **connected/described** | How tool calls are **executed** |
| Tool-call path | model → app → MCP server → result **into context** | model writes code → sandbox calls tool → result **stays in the code** |
| Intermediate results | every result enters the model's context | stay in the running script; only final output hits context |
| Token cost | scales with number of calls × result size | scales with **final** output only |
| Best for | connecting/standardizing/reusing capabilities | looping, filtering, big intermediate data |

**Example** — "read 200 cells, bold the ones over 100":
- Normal/MCP: ~200 `tool_use`/`tool_result` round trips, all 200 values pumped through context.
- PTC: the model writes a loop; the 200 reads + writes happen inside the sandbox; only
  `"bolded 47 cells"` comes back to the model.

**Why "incompatible":** PTC needs the tool callable as a function inside the code sandbox
(`allowed_callers`), which MCP-connector tools are not.

---

## Reference (Claude tool-use docs)

- Tool Search Tool: `agents-and-tools/tool-use/tool-search-tool`
- Programmatic Tool Calling: `agents-and-tools/tool-use/programmatic-tool-calling`
- Code execution tool: `code_execution_20260521` / `code_execution_20260120`
- Agent design (bash-vs-dedicated-tools, tool-surface scaling): the `agent-design`
  guidance in the claude-api skill.

---

## Update — collapsed to two tools + mode-gated approval (office-mode-gated-approval)

The surface was collapsed to exactly **`list_open_documents`** (native discovery, no
pane) and **`run_office_js`** (everything else). The five typed tools
(`read_document`, `get_selection`, `add_comment`, `set_track_changes`,
`get_tracked_changes`) were removed — `run_office_js` subsumes all of them, and the
model is prompted (via the tool description) with the equivalent Office.js snippets.

`run_office_js` gained **`mode: "read" | "write"`**. The model declares intent; the
**server MCP approval loop** (not the pane) gates on it:

- `mode:"read"` → auto-runs (no prompt).
- `mode:"write"` (or missing / any non-`"read"` value → fail-safe) → the existing
  per-call approval (allow once / **always allow, remembered for the conversation** /
  deny).

This reuses existing machinery: the read-bypass mirrors `control_mcp`'s per-call
`control_call_needs_approval` classifier, and "always allow" is the existing
`auto_approved_tools` per-conversation memory. Implementation:
`mcp/chat_extension/office_approval.rs` (`compute_needs_approval` +
`run_office_js_read_bypass`), gated on the office_bridge server id + tool name + exact
`"read"`.

**Deliberately trust-based — NO read-only enforcement (accepted trade-offs).** We chose
NOT to build the enforced read-only `context` Proxy. Three consciously-accepted risks:

1. **No enforcement.** A prompt-injected `mode:"read"` script that actually mutates
   bypasses approval — the model is trusted to classify honestly.
2. **Auto-approved reads are a silent exfiltration channel.** A "read" can pull the
   entire document (`body.load('text')`, `getFileAsync`, used-range values) into the
   conversation with no prompt — auditable via `mcp_tool_calls`, not blocked. Read-only
   protects the document's **integrity**, not the **confidentiality** of its contents.
3. **"Always allow" blast radius.** Once granted, every later write in the conversation
   runs unprompted, including a later injected one.

The enforced read-only Proxy (default-deny allowlist over the Office.js `context`)
remains the documented upgrade path if the threat model tightens.
