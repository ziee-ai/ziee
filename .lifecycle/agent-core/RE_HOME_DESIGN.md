# Chat → agent-core FULL extension re-home — design spec

Human decision (2026-07-17): **full extension re-home** — agent-core natively
runs the whole chat pipeline; every chat concern is expressed either as an
agent-core `AgentExtension` (domain-neutral, in the crate's extension model) or
as one of the six ports. This is the largest/riskiest of the three options,
chosen deliberately. This doc is the multi-session anchor.

## Ground truth (measured, not assumed)

18 `ChatExtension` impls exist. Hook footprint (measured `2026-07-17`):

| Extension | Hooks overridden | Re-home target |
|---|---|---|
| js_tool | before_llm_call | `AgentExtension::before_model` (attach flag + tool guidance) |
| bio_mcp | before_llm_call | `before_model` |
| citations | before_llm_call | `before_model` |
| lit_search | before_llm_call | `before_model` |
| knowledge_base | before_llm_call | `before_model` |
| web_search | before_llm_call | `before_model` |
| skill | before_llm_call | `before_model` |
| control_mcp | before_llm_call | `before_model` |
| code_sandbox | before_llm_call + after_llm_call | `before_model` + `after_round` |
| assistant | before + after_llm_call + after_user_message_created + register_routes | `contribute` (system prompt) + `after_round` + host + host-routes |
| project | before + after_llm_call + register_routes | `contribute` + `after_round` + host-routes |
| memory | before + after_llm_call + register_routes | `before_model` (retrieval) + `after_round` (bg extraction) + host-routes |
| summarization | before + after_llm_call + register_routes | agent-core **CompactionExtension** (already a core ext) + host-routes |
| title | after_llm_call | `after_round` (bg title gen) |
| text | process_delta, accumulate_delta, get_accumulated_content, convert_from_content_block, process_content_for_llm, process_content_from_db, provide_user_message_content, handled_content_types | `AgentExtension::on_delta` + **ChatTranscriptStore** (block reconstruct) |
| file | before + provide_user_message_content + process_content_for_llm + process_content_from_db + should_skip_in_assistant_forwarding + handled_content_types + register_routes | **ChatTranscriptStore** + host request-assembly + host-routes |
| mcp | before + after_llm_call + provide_assistant_message + should_create_user_message + after_user_message_created + accumulate_delta + process_delta + get_accumulated_content + convert_extension_content + convert_from_content_block + register_routes | **ToolProvider** + **HumanGate** + **ApprovalPolicy** + native loop + host + host-routes |

## The re-home rule (how a ChatExtension hook maps)

- **before_llm_call** (mutate `ChatRequest`, inject system/tools, set attach
  flags) → `AgentExtension::before_model(&mut ChatRequest)` (+ `contribute` for
  pure system-block/tool-scope contribution). Short-circuit
  (`BeforeLlmAction::Complete`) → `Flow::ShortCircuit`.
- **after_llm_call** — SPLITS by responsibility:
  - continuation-driving (ONLY mcp returns `Continue`) → agent-core's **native
    tool-detection loop** (not an extension hook). Verified: assistant/project/
    memory/etc. never drive continuation.
  - side-effects (title gen, memory extraction, sandbox cleanup) →
    `AgentExtension::after_round(&ChatMessage)` (spawn-and-forget as today).
- **process_delta / accumulate_delta / get_accumulated_content** →
  `AgentExtension::on_delta` for the streaming half; the **persist** half
  (turning accumulated blocks into rows) is the **ChatTranscriptStore**'s job on
  `append`, using the same `finalize` MAX+1 insert + reconstruction.
- **convert_from_content_block / convert_extension_content / process_content_for_llm
  / process_content_from_db / provide_user_message_content /
  should_skip_in_assistant_forwarding** → these are TRANSCRIPT concerns (block
  ↔ `MessageContentData` ↔ provider `ContentBlock`, DB enrichment). They live in
  **ChatTranscriptStore** (load/reconstruct + append/persist) + the chat host's
  pre-loop request assembly, NOT in the crate's `AgentExtension`.
- **should_create_user_message / provide_assistant_message / after_user_message_created**
  → chat-MESSAGE-LIFECYCLE, owned by the **chat host** (the `send_message`
  handler wiring) BEFORE it calls `core.run`. These stay host-side (resume
  semantics for cross-request approval).
- **register_routes** → the chat module keeps registering extension routes
  exactly as today; unaffected by the loop re-home.

## What the crate must grow (domain-neutral)

1. **A per-turn input carrier.** Context-injectors read
   `SendMessageRequest.extensions` (attach flags, tool_approvals, file_ids). Add
   `TurnContext.metadata` is already present (HashMap<String,Value>); thread the
   request's extension payload into `AgentTurnRequest` as an opaque
   `inputs: serde_json::Value` (or `HashMap`), surfaced to `contribute`/
   `before_model` via `TurnContext`. Domain-neutral (crate never names a chat
   field). **DEC needed.**
2. **An SSE-emit capability for extensions.** Some `before_llm_call` impls emit
   SSE (e.g. titleUpdated, approval-required) via `tx`. In agent-core the
   `EventSink` is the only emit path; extensions get it by the host capturing the
   sink in the concrete extension struct (the trait stays sink-free). No trait
   change — a wiring convention. Confirm each porting extension can reach the
   sink via its own field.
3. **on_delta already exists**; confirm it covers text's thinking-signature +
   redacted-thinking interception (the "not streamed to client" deltas).

Everything else is host/port work, not a crate-trait change.

## Build order (each step compile-verified; behavior-verified vs chat suite + bridge at the end of each port wave)

1. **DEC pass** — resolve the per-turn input carrier + sink-capture convention +
   ordering (chat uses METADATA.order; agent-core uses `order()`), record in
   DECISIONS.md.
2. **Ports** — ChatModelResolver (`create_provider_from_model_id` + params +
   thinking), ChatToolProvider (list = mcp tool-gather; call = `execute_tool` via
   the now-generalized `call_mcp_tool`), ChatApprovalPolicy + ChatHumanGate
   (cross-request Suspend), **ChatTranscriptStore** (the big one — load +
   `group_assistant_blocks` reconstruct; append = `finalize` MAX+1 + text/file
   block conversion), ChatEventSink (ContentDelta→`Content` frame,
   Message→terminal frame + `sync:conversation`).
3. **Context-injector AgentExtensions** — port the 9 before-only + assistant/
   project/memory/title/sandbox onto `AgentExtension`. Mechanical; each is
   small.
4. **Host** — ChatAgentDispatcher: keep the fire-and-forget contract
   (`send_message` persists ids + returns; a detached task runs `core.run`); keep
   the per-`assistant_message_id` `CANCELLATION_TRACKER` token as the
   `CancelSignal`/`CancelToken`; keep the single-flight `begin_generation` slot.
   Replace `StreamingService::send_message`'s loop body with `core.run`.
5. **Cutover** — switch the `send_message` handler to the dispatcher behind the
   same return type; keep the old path deletable in one commit.
6. **Verify** — chat integration suite (`tests/chat/*`, `tests/mcp/*_workflow_*`)
   + real-LLM against the bridge (a genuine multi-tool + approval + resume run);
   fix to green.

## Non-negotiable behaviors to preserve (from the map)

- Fire-and-forget: POST returns `{user_message_id, assistant_message_id}`
  immediately; tokens stream over the separate `/api/chat/stream` SSE.
- Loop is tool-driven continuation only; empty-completion guard; audience-only
  tool output → `CompleteWithContent` (final answer, no re-loop).
- Cross-request approval: turn ends on pending approval; resume on new POST
  appends to the SAME assistant message.
- Block persistence: one `message_contents` row per block, monotonic
  `MAX(sequence_order)+1`, UNIQUE(message_id, sequence_order); thinking
  `token_count` inside the block JSON.
- Cancellation keyed by `assistant_message_id`; partial persist on cancel via
  idempotent finalize.
