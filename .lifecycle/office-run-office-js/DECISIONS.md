# DECISIONS — office-run-office-js

Every human/product input resolved up front so implementation runs nonstop.

### DEC-1: What timeout governs a `run_office_js` pane round-trip?
**Resolution:** Reuse the shared `broker::CALL_TIMEOUT` (15s). A script that
exceeds it surfaces the existing typed `OFFICE_PANE_TIMEOUT`, which the model
self-corrects on by reducing scope (smaller batch). No new/separate timeout in
v1; a configurable per-call timeout is a documented future enhancement.
**Basis:** convention — reuse the existing broker timeout (DEC-2 of the prior
pane-rpc feature); the writeup's "structured errors so the model self-corrects"
makes the timeout a recoverable signal, not a hard failure.

### DEC-2: Does `run_office_js` need any per-call-approval gating code?
**Resolution:** No. `office_bridge` is deliberately ABSENT from
`is_builtin_server_id` (server `mcp/chat_extension/mcp.rs:122-127, 204-221`), so
every office_bridge tool — including `run_office_js` — already requires per-call
user approval. Do NOT add office_bridge to the approval-bypass set. The security
requirement is satisfied by the existing posture with zero new code.
**Basis:** codebase — the auto-attach-but-not-bypass seam is explicit and
commented for exactly this ("mutating office tools stay behind per-call approval").

### DEC-3: Remove `edit_document` entirely, or keep it as a native no-pane append?
**Resolution:** Remove `edit_document` (and its `op` enum + the sole-consumer
native `act_on_document`/`DocOp`/`ActResult` path). `run_office_js` subsumes
append (`context.document.body.insertParagraph(text, "End")`). Accepted
trade-off: this drops the ONE capability that worked without an open task pane
(native osascript/COM append); acceptable because the entire pane-mediated
surface (read/selection/comments/track) already requires a pane, so the bridge's
value proposition already assumes one.
**Basis:** user — the handoff writeup is explicit: "Delete the `edit_document`
`op` enum … simplest is to drop the enum and let edits go through run_office_js."

### DEC-4: Ship the read/write declared-intent split (auto-approve read scripts) in v1?
**Resolution:** Defer. v1 ships write-always-prompts — `run_office_js` always
requires per-call approval (inherited, DEC-2), no `mode: read|write` parameter,
no context-proxy read-only enforcement. A future item can add the declared-intent
split + proxy enforcement.
**Basis:** user — the writeup: "If that's too much for v1, ship write-always-
prompts and defer the split."

### DEC-5: How is the host runtime chosen, and what about an unknown host?
**Resolution:** Pick `Word.run` / `Excel.run` / `PowerPoint.run` by the pane's
`HOST` global (set on `Office.onReady`). `run_office_js` is host-agnostic — NO
PowerPoint pre-gate (unlike the Word-only comment/track tools). An unknown /
undefined host (or the matching `*.run` API absent) → `replyErr(id,
ERR_UNSUPPORTED_HOST, …)` (−32002), which the daemon maps to
`OFFICE_UNSUPPORTED_ON_HOST`.
**Basis:** codebase — mirrors `opReadDocument`'s `HOST`-branching and the
existing `ERR_UNSUPPORTED_HOST` → `OFFICE_UNSUPPORTED_ON_HOST` mapping.

### DEC-6: What real LLM backs TEST-11, and how is it gated?
**Resolution:** The OpenAI-compatible LiteLLM proxy on `coder.ziee:4000`
(model `qwen3.6-35b-a3b`; verified it emits a well-formed `run_office_js` tool
call with valid Office.js). Reached from this Mac via an SSH tunnel
(`ssh -fN -L 4000:127.0.0.1:4000 coder.ziee`). TEST-11 reads
`ZIEE_OFFICE_REAL_LLM_URL` (the OpenAI `/v1/chat/completions` base), with optional
`ZIEE_OFFICE_REAL_LLM_MODEL` (default `qwen3.6-35b-a3b`) and
`ZIEE_OFFICE_REAL_LLM_KEY` (LiteLLM key if required). Soft-skips (eprintln +
early return) when the URL env var is unset, mirroring `injection_test`'s
`ANTHROPIC_API_KEY` soft-skip, AND is `#[cfg(target_os="macos")] #[ignore]` since
it drives a live Excel pane. Scope note: TEST-11 calls the model directly (real
model + real shipped tool schema + real Office.js execution via the live pane);
it does NOT route through ziee's OpenAI-provider chat pipeline (the desktop test
harness has no chat-send helper, and building one is out of this feature's scope)
— the daemon-side tool routing is covered non-LLM by TEST-7.
**Basis:** user — instructed to use the `coder.ziee` `:4000`/`:8000` endpoint for
the real-LLM test; soft-skip pattern from the codebase.

### DEC-7: What is the `run_office_js` result shape, and how are big/odd returns handled?
**Resolution:** The pane replies `{ result, truncated }` — `result` is the
script's `return` value JSON-serialized then capped via the existing
`capText`/`MAX_READ_CHARS`; `truncated` flags the cap. No return / `undefined` →
`result: null`, `truncated: false`. A non-JSON-serializable / circular value
degrades to `String(value)` and never throws. The daemon wraps this via
`pane_tool_result` (result in `structuredContent`, readable text in `content`).
**Basis:** convention — mirrors `read_document`'s `{ text, truncated }` cap and
the `pane_tool_result` wrapper.

### DEC-8: How does the pane execute the model-supplied script string?
**Resolution:** Compile it as an async function body —
`new Function('context', '"use strict"; return (async function(){' + script +
'\n})()')` — and invoke it INSIDE the host `run`:
`Excel.run(function(context){ return theFn(context); })` (and Word/PowerPoint).
The script may `await context.sync()` and `return` a value; `run()` auto-syncs on
resolve and rolls back on throw. The trailing `\n` guards a `//`-comment last line.
**Basis:** convention — the standard Office.js embedding; the `context` object is
exactly what `opReadDocument` uses inside `Word.run`/`Excel.run`.

### DEC-9: What structured error does a failed script return?
**Resolution:** On `catch(e)`, `replyErr(id, ERR_OP_FAILED, msg)` where `msg`
combines `e.name`/`e.message` and, when the error is an `OfficeExtension.Error`,
its `.code` and `.debugInfo` (JSON-stringified). The daemon surfaces this as
`OFFICE_PANE_ERROR`, giving the model the failing code + context to self-correct
in one retry.
**Basis:** user (writeup: "Return STRUCTURED ERRORS … so the model self-corrects
in ONE retry") + codebase (the existing `.catch → replyErr` pattern).
