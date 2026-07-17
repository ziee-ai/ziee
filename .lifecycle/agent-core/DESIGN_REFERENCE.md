# Ziee Shared Agent Core Architecture (a ziee crate, built on the SDK)

**Status:** design (no code). Deliverable of the deeper-research agenda in
[`OSS_AGENT_LANDSCAPE.md`](../workflow-research-wt/OSS_AGENT_LANDSCAPE.md) §7, building on
[`AGENT_ARCHITECTURE_RESEARCH.md`](../workflow-research-wt/AGENT_ARCHITECTURE_RESEARCH.md) and
[`WORKFLOW_SDK_RESEARCH.md`](../workflow-research-wt/WORKFLOW_SDK_RESEARCH.md).
**Method:** every non-obvious decision is justified against a **primary source** (LangGraph
checkpointer source, Mastra `agentic-loop` source, Block Goose `crates/goose`, OpenAI Codex
`codex-rs`, Letta docs, DBOS/Temporal docs) read first-hand this session, and mapped onto the
**exact ziee seams** (`chat/core/services/streaming.rs`, `workflow/{runner,dispatch,elicit,startup_sweep,types}.rs`,
`ai-providers`, `mcp/client/session.rs`). Base: `origin/main @ 6da3d2aef`.

---

## 0. Thesis (read this first)

**One loop, three hosts — not three loops, and not "the agent is a workflow."**

The agent loop is universal and tiny (every serious agent — Claude Code, Codex, Cursor, Goose,
Mastra — runs the same *gather context → call model → run tools → feed results back → repeat until
no tool call or a cap* cycle). All the engineering value is in the **machinery around the loop**:
context compaction, subagents, the tool interface, the approval/sandbox/reviewer matrix, and
durability. So the right unit to extract is **the loop plus that machinery, behind a small set of
ports** — a self-contained `AgentCore` primitive — and then **host** it three ways:

1. **Chat** constructs an `AgentCore` with chat-flavored ports (SSE sink, chat repos, the
   pause-and-resume approval gate) and streams it. `streaming.rs` becomes a ~thin adapter.
2. **The workflow `kind: agent` step** constructs the *same* `AgentCore` with workflow-flavored
   ports (the `ProgressEmitter`, the run journal, the **durable `elicit` gate**) and runs it as
   **one DAG step**.
3. **Parallel fan-out** spawns N `AgentCore` instances as **subagents**, each with an isolated
   transcript, each returning **a summary, not its transcript**.

The crucial architectural choice — and where we deliberately **diverge from Mastra** — is that
**durability is supplied *through the ports*, per host, not baked into the loop.** Mastra achieved
unification by re-expressing every chat turn *as a workflow run* so one durable substrate serves
both [Mastra `packages/core/src/loop/workflows/agentic-loop/index.ts`]. Ziee does **not** need
that: ziee already owns a durable substrate for workflows (`step_outputs_json` journal + `waiting`
gate + `resume_run` + `startup_sweep`), and its ports let **each host pick its own durability
tier** — chat gets cheap coding-agent-grade *transcript resume*, the workflow step gets
Temporal/DBOS-grade *journaled step resume*. That is strictly better than Mastra for ziee's shape:
chat never pays workflow-snapshot overhead per token, yet an autonomous `kind: agent` run survives
a server restart.

**Commit three, design the ports for ~seven.** The three targets are the load-bearing, committed
scope — but every mature agent hosts *one* core loop from **5–8** distinct surfaces, and the two
same-stack Rust twins are the highest (Codex 8, Goose 7). Ziee already owns the substrate for ~4
more hosts (desktop, scheduled/unattended, MCP-exposed agent, standalone-headless) that fall out of
the same core + ports for near-free. So the ports are sized for ~7, not 3 — see §7.1. This is the
decisive evidence for the ports investment over a one-off `AgentDispatcher` (the landscape doc's
counter-argument was "core-plus-ports only pays off with a 2nd/3rd consumer"; the leaders overshoot
that bar by 2–3×).

Everything below is the detail of that thesis.

---

## 0.5 SDK re-framing (2026-07-16 — the SDK has landed; the agent core stays in ziee)

Since §0 was written, the **ziee SDK extraction landed on main** (`46f605dc5`): ziee's platform layer
is now a standalone submodule (`sdk/`) of **domain-neutral crates** — `ziee-core` (`AppError`/
`ApiResult`/macros), `ziee-identity` (identity *traits*), `ziee-framework` (module system + sync +
`app_builder` + codegen), `ziee-auth`, `ziee-control-mcp`, + infra — consumed by ziee (and app #2,
**CytoAnalyst**) via cross-workspace path-deps.

**Decision (human): the agent core and `ai-providers` STAY IN ZIEE — they are ziee-only, NOT SDK
crates.** Rationale: the SDK is the *shared platform*; the agent is *ziee's own feature*. No other app
needs the agent loop — CytoAnalyst is **purely companion-driven** (no agent of its own; ziee drives
it, see §7.2) — so promoting the core to a shared SDK primitive would be premature generalization, and
forcing SDK domain-neutrality (N9) on a single-consumer, deeply-ziee-domain crate would be cost with
no benefit. **Nothing in the loop/compaction/durability/safety design (§1–§6) changes.** The SDK
landing changes *less* than this section's first draft implied — here is the corrected framing:

- **The agent core is a ziee-app crate** — `src-app/agent-core` in the **ziee** workspace (the
  DEC-15/DEC-18 crate, unchanged in shape), a sibling of `ai-providers`. **Not** `sdk/crates/ziee-agent`;
  **not** domain-neutral — it may name ziee domain (`citations`/`lit_search`/…) freely. The crate wall
  still buys the compiler-enforced port boundary; it's just a boundary *within ziee*, not an SDK edge.
- **`ai-providers` stays app-side** (`src-app/server/ai-providers`) — **no relocation.** This drops the
  one prerequisite the SDK framing had added.
- **N9 does NOT apply** to the core. Drop the domain-neutrality gate, the templated-app-name, and the
  "no domain nouns" grep — those are for *SDK* crates; this is a ziee crate. (`INV-8`/`TEST-36` reverts
  to the plain crate-dependency-boundary test — deps are `ai-providers` + `ziee-core` + `ziee-identity`,
  not the whole server.)
- **The agent core is BUILT ON the SDK** — it deps the SDK crates the rest of the ziee app already
  deps: `ziee-core` (`AppError`/`ApiResult`/macros — the whole ziee app now uses these) and
  `ziee-identity` (`Principal`/`PermissionCheck`, to gate tools by permission). So the earlier bespoke
  `AgentError` **becomes `ziee_core::AppError`** for the driver (now app-wide-consistent); pure ports
  keep associated `Error` types (à la `TokenVerifier::Error`).
- **The ports remain the right design** — and it's a good sign they *mirror* the SDK's own
  pluggable-seam pattern (`AgentCore` generic over injected `Arc<P>` ≈ `RequirePermissions<R:
  IdentityResolver>`). But that's a *pattern the core borrows*, not a constraint the SDK imposes: the
  ports exist for the three-host reuse **within ziee** (§7), not for cross-app genericity.
- **The three hosts stay app-side** (chat, workflow, mcp, code_sandbox, memory, summarization, the
  science tools — verified none moved to the SDK) and construct `AgentCore` from the ziee crate. The
  `AgentExtension` registry stays app-side (server owns the `distributed_slice`, DEC-18).
- **The one genuinely SDK-enabled dimension survives — driving *other* apps (§7.2).** ziee's single
  agent, via **`control_mcp`** (an SDK crate) in its `ToolProvider`, operates companion apps like
  CytoAnalyst. This is *why* the agent is ziee-only: ziee is the one agent; every other SDK app is
  companion-driven (exposes `control_mcp`), not agent-having. The "OS of apps / companion-AI" model
  holds — the agent just lives in ziee, not the SDK.

**Net delta from the SDK landing:** (1) keep the agent core + `ai-providers` in ziee; (2) drop the
`ai-providers`→SDK relocation and the N9 constraint; (3) adopt `ziee_core::AppError` (the app-wide
error now); (4) the agent gains a real cross-app job via `control_mcp` (§7.2). The three-host design
(§1–§7) is otherwise intact.

---

## 1. State of the art (one paragraph + the evidence)

Across LangGraph, Mastra, Goose, Codex, Letta, DBOS and Temporal the picture is remarkably
consistent. **The loop is a solved, universal shape** returned as an async **event stream** (Goose:
`Agent::reply(...) -> BoxStream<Result<AgentEvent>>`, events coarse — `Message | Usage |
McpNotification | HistoryReplaced` — with tool requests riding *inside* message content blocks, not
as their own variants [Goose `crates/goose/src/agents/agent.rs`]). **Durability universally lives at
the completed-step boundary and never mid-step**: DBOS journals one row per *completed* step
(`dbos.operation_outputs.output`) and re-runs the workflow function from the top, serving completed
steps from the checkpoint [DBOS docs/architecture]; Temporal replays deterministic workflow code
against an Event History and runs each side-effecting Activity exactly once, recording only its
*result* [Temporal docs/workflows]; LangGraph's `PostgresSaver` snapshots full state per super-step
into `checkpoints`/`checkpoint_blobs` with a **separate `checkpoint_writes` table for uncommitted
per-task writes**, and on resume **re-runs the whole node from the top** — "it does not resume from
the exact line where `interrupt` was called" — replaying already-committed task writes so completed
tasks don't re-fire [LangGraph `checkpoint-postgres/base.py`, interrupts docs]. **Context compaction
is a core-loop responsibility, not a tool** — Letta states it outright: compaction is "fully
automatic — not a tool the agent invokes… built into the core loop," a sliding window that
summarizes the oldest ~30% and keeps ~70%, escalating in ~10% steps, with the summary written back
into the buffer and raw messages retained in recall storage; *deliberate* knowledge curation is
separate (agent tool calls editing memory blocks) [Letta compaction + memory-blocks docs]. **Safety
is a two-axis matrix fronted by a reviewer**: Codex composes `SandboxPolicy` (`read-only` /
`workspace-write` / `danger-full-access`) × `AskForApproval` (`untrusted` / `on-request` / `never` /
granular), pauses on a oneshot keyed by `call_id` with `ReviewDecision::{Approved,
ApprovedForSession, Denied, Abort}`, and optionally interposes an `auto_review` **reviewer agent**
that risk-classifies each escalation (Low→proceed, High→human, Critical→deny, **fail-closed**) —
crucially only vetting actions that *already* require approval [Codex `protocol/src/protocol.rs`,
`approvals.rs`, learn.chatgpt.com config]. **Subagents are bounded** (`[agents] max_depth = 1`,
`max_threads = 6`, per-agent model/effort/sandbox) and return **a summary, not the full transcript**
[Codex `[agents]`]. And **Mastra proves one engine can serve both chat and durable workflows** by
making the agent turn *be* a workflow, reconciling token-streaming with durable snapshots via one
discipline — "don't persist `running` snapshots," stream out-of-band [Mastra source]. Net: ziee's
plan is validated on every axis; the one place ziee should *not* follow the leader is Mastra's
"turn = workflow run" — ziee's ports make that unnecessary.

---

## 2. Design principles (each extracted from a primary source)

| # | Principle | Primary-source basis |
|---|---|---|
| P1 | **The loop is universal; extract it once.** A single `AgentCore::run` returning an event stream serves every consumer. | Goose `Agent::reply -> BoxStream<AgentEvent>`; the four-agent convergence in `AGENT_ARCHITECTURE_RESEARCH.md §2`. |
| P2 | **Events are coarse; tool requests ride inside messages.** Don't invent a per-tool-lifecycle event zoo in the core. | Goose `AgentEvent { Message, Usage, McpNotification, HistoryReplaced }`. |
| P3 | **Compaction lives *in* the core loop, not as a tool.** Sliding-window summarize-oldest, keep-newest, escalate on overflow. | Letta: "built into the core loop"; Goose `HistoryReplaced` event + `replace_conversation`. |
| P4 | **Deliberate memory curation stays a tool the agent calls.** Distinct from (P3) compaction. | Letta `core_memory_append`/`archival_memory_*` are ordinary tool calls. |
| P5 | **Journal at the *completed-tool-call* boundary — the finest sane unit. Never mid-call.** | DBOS `operation_outputs` (per completed step); Temporal Activity result; LangGraph `checkpoint_writes` per task. |
| P6 | **Resume re-runs the interrupted turn from the top, serving *completed* tool calls from the journal.** Pre-completion code re-executes → make it idempotent / re-runnable. | LangGraph "restarts the entire node from the beginning"; DBOS "checks before each step if checkpointed." |
| P7 | **Durability is a *port*, supplied by the host — not baked into the loop.** Chat tier ≠ workflow tier. | Mastra unified by baking it in (we reject that); Goose leaves persistence concrete (an anti-pattern — make it a trait). |
| P8 | **Safety = SandboxPolicy × ApprovalPolicy × Reviewer, escalating to a durable human gate.** | Codex `SandboxPolicy`/`AskForApproval`/`auto_review`; ziee `elicit` durable gate. |
| P9 | **Subagents are bounded and return a summary, not a transcript.** | Codex `[agents] max_depth=1, max_threads=6`; Anthropic subagent isolation. |
| P10 | **Stream tokens out-of-band; snapshot only at durable boundaries.** Don't let the token stream fight the journal. | Mastra "don't persist `running` snapshots"; ziee already streams deltas over SSE separate from DB writes. |

---

## 3. The shared agent core

### 3.1 Where it comes from — ziee is *already* 80% here

The single most important codebase finding: **ziee's chat loop is not a monolith — it is a generic
loop with pluggable `ChatExtension`s, and the tool-calling agent behaviour is itself an extension**
(`mcp/chat_extension`, order 30). `core/services/streaming.rs` already runs
`loop { before_llm_call(exts) → provider.chat_stream → drain deltas → finalize → after_llm_call(exts)
→ branch on ExtensionAction::{Continue, CompleteWithContent, Complete} }` with a `SAFETY_MAX_ITERATIONS
= 1000` failsafe and the real cap enforced by the MCP extension's `loop_settings.max_iteration`
(`streaming.rs:184,217-744`; `mcp.rs:2007`). **So "extract the agent core" is not a rewrite — it is
lifting the generic loop skeleton out of `streaming.rs`, formalizing the seams that already exist as
ports, and letting the workflow step re-host it.** This is the "build core by *extraction*, not
abstraction" advice validated (`AGENT_ARCHITECTURE_RESEARCH.md §6`).

### 3.2 The core loop signature

```rust
/// The shared agent core. Constructed by a HOST (chat / workflow-step / subagent-orchestrator)
/// with host-flavored ports, then driven. It owns NO I/O of its own — every side effect goes
/// through a port. This is the single primitive the SDK extraction exposes.
pub struct AgentCore {
    provider:  Arc<Provider>,          // ai_providers::Provider — already a port (streaming-first)
    tools:     Arc<dyn ToolProvider>,  // §4.3  — unifies built-in + external MCP
    transcript: Arc<dyn TranscriptStore>, // §4.1 — append/replace/load turns (Postgres-agnostic)
    sink:      Arc<dyn EventSink>,     // §4.2  — push events out (SSE / ProgressEmitter)
    gate:      Arc<dyn HumanGate>,     // §4.4  — request approval / elicit input; MAY be durable
    policy:    Arc<dyn ApprovalPolicy>,// §4.5  — decide auto-approve / prompt / deny per tool call
    compactor: Compactor,             // §3.4  — IN-CORE (not a port); reuses conversation_summaries
    budget:    Budget,                // token / wall-clock / iteration / byte caps
    limits:    SubagentLimits,        // §3.5  — max_depth, max_threads
}

pub struct AgentTurnRequest {
    pub run_id:         Uuid,          // conversation-turn id OR workflow run_id — the journal key
    pub user_id:        Uuid,
    pub seed:           TurnSeed,      // new user message, OR a resumed/continued transcript cursor
    pub system:         Vec<ContentBlock>, // assistant/project/skill/core-memory system blocks
    pub tool_scope:     ToolScope,     // which servers/tools this turn may call (RBAC-resolved)
    pub start_iteration: u32,          // 1 for fresh; N for a resumed turn (P6)
}

impl AgentCore {
    /// Drive one agent turn to a stop condition. Returns a pull-stream of coarse events (P2);
    /// the host forwards them to its EventSink surface and persists via the TranscriptStore.
    /// This is Goose's `reply` shape, made port-driven.
    pub fn run(
        &self,
        req: AgentTurnRequest,
        cancel: CancelToken,
    ) -> impl Stream<Item = Result<AgentEvent, AgentError>> + Send;
}

/// Coarse, à la Goose — tool requests ride INSIDE Message content blocks (P2).
pub enum AgentEvent {
    Message(Message),                  // assistant text / thinking / tool_use / tool_result blocks
    Usage(Usage),                      // token accounting per model call
    ToolNotification { server: String, note: ServerNotification }, // progress from a running tool
    HistoryReplaced { summary_upto: usize }, // compaction happened (P3) — host re-syncs its cache
    GateOpened(GateTicket),            // core is suspending on a human decision (§4.4)
    Stopped(StopReason),               // NoToolCall | IterationCap | TokenCap | WallClock | Halted
}
```

**The loop body** (pseudocode — this is what lifts out of `streaming.rs`, unchanged in spirit):

```
turn(req):
  iteration = req.start_iteration
  history   = transcript.load(req.run_id)           # port
  loop:
    ctx      = assemble(req.system, history, tool_scope)   # REBUILD every turn (the key fact)
    ctx      = compactor.fit(ctx, budget)           # §3.4 IN-CORE; may emit HistoryReplaced
    chat_req = ChatRequest { messages: ctx, tools: tools.list(req.tool_scope), .. }
    stream   = provider.chat_stream(chat_req)        # ai_providers — already a port
    (assistant_msg, usage) = drain(stream) → yield Message / Usage    # tokens stream out-of-band (P10)
    transcript.append(req.run_id, assistant_msg); yield Usage(usage); budget.add(usage)
    tool_uses = assistant_msg.tool_uses()
    if tool_uses.is_empty(): yield Stopped(NoToolCall); return          # final answer
    if budget.iteration_capped(iteration): synthesize_pending_results(); yield Stopped(IterationCap); return
    for tu in tool_uses:
        decision = policy.decide(tu, sandbox_mode)   # §4.5 — auto | prompt | deny | review
        if decision == Prompt|Review:
            ticket = gate.request(tu, decision)      # §4.4 — MAY suspend durably; yield GateOpened
            decision = gate.await(ticket)            # resumes here on approve/deny (durable or live)
        result = if decision.approved:
                    tools.call(req.run_id, tu, idempotency_key(req.run_id, tu, iteration))  # §4.3, P5
                 else: synth_denied(tu)
        transcript.append(req.run_id, result); yield Message(result)   # JOURNAL at completion (P5)
    iteration += 1
```

Every numbered concern below is one line in this body.

### 3.3 The event/message model — reuse, don't reinvent

`AgentEvent::Message` carries a `Message` whose content blocks are the *provider's* own
`ContentBlock::{Text, Thinking, ToolUse, ToolResult}` (already in `ai-providers/models/chat.rs`, and
already the wire format chat persists into `message_contents`). Tool requests are `ToolUse` blocks
(P2). This means:

- **Chat host** maps `AgentEvent` → its existing `SSEChatStreamEvent::{Started, Content, Complete,
  Error}` + the extension raw events (`mcpToolStart`, `mcpApprovalRequired`, `mcpElicitationRequired`)
  — a near-identity mapping, because those events already exist.
- **Workflow host** maps `AgentEvent` → `SSEWorkflowRunEvent::{StepStarted, StepProgress,
  StepCompleted, ElicitationRequired, ...}` — the agent step reports its inner tool calls as
  `StepProgress` tracks (the same channel `llm_map` / sandbox already use).

### 3.4 Context compaction — IN the core (P3)

Compaction is a **core method, `Compactor::fit`, not a port and not a tool** — Letta is explicit and
the coding agents unanimous that this is the highest-leverage core feature. Design, borrowing
Letta's sliding window and reusing ziee's existing `conversation_summaries` +
`summarization/engine/summarizer.rs`:

- **Trigger:** when `estimate_tokens(ctx) > budget.context_soft_limit` (a fraction of the model
  window). Not a fixed constant — window-relative, like Letta.
- **Mechanism:** keep the newest ~70% of turn messages verbatim; summarize the oldest ~30% via the
  provider into a single system block; if still over, escalate the summarized fraction in ~10% steps
  (Letta's exact algorithm). The core-memory tier (`assistant_core_memory`) is injected **verbatim,
  never summarized** (Letta: blocks are always in-context).
- **Where the summary goes:** written to `conversation_summaries` (the chat host's existing rolling
  per-branch summary) via the `TranscriptStore` port's `replace_head(run_id, summary, upto)`; the
  evicted raw messages **stay in the DB** (ziee's "recall storage" — retrievable via
  `tool_result_mcp` / `get_message_window`). Emits `AgentEvent::HistoryReplaced` so the host re-syncs
  its cache (Goose's `HistoryReplaced` / `replace_conversation` pattern).
- **Why in-core, not a tool:** "the agent should never spend a turn deciding to compact" (Letta). The
  existing `summarization` chat extension (order 24) already does the chat-side of this; the core
  absorbs it so the *workflow* agent step gets compaction for free (today a long `kind: llm_map`
  fan-out has none).
- **Durability-layer analog:** compaction *is* ziee's Temporal `continue-as-new` — it bounds the
  replayable transcript so a long autonomous run never outgrows its window or its journal
  [Temporal continue-as-new].

**Retrieval (P4) stays a tool.** `memory` (`user_memories` vector recall) and `knowledge_base`
remain MCP tools the agent *chooses* to call — Letta's block-editing/archival tools. The core does
**not** own retrieval; it owns compaction. (The chat memory extension's `before_llm_call` *injection*
of top-K memories becomes a system-block contributor to `AgentTurnRequest.system`, not core logic.)

### 3.5 Subagents / parallel fan-out — a core primitive (P9)

A subagent is "a fresh agent loop with its own context window that returns *only* a summary"
(Anthropic; Codex). Model it as the core calling **itself** through a bounded orchestrator:

```rust
pub struct SubagentLimits { pub max_depth: u8 /*=1*/, pub max_threads: u8 /*=6*/ }

impl AgentCore {
    /// Spawn N isolated child AgentCores concurrently; each gets a FRESH transcript
    /// (own run_id, own compaction budget) and returns ONLY a summary string/struct.
    /// Bounded by SubagentLimits + the parent's remaining Budget. Depth is decremented;
    /// at depth 0 the tool is not offered (Codex max_depth=1 = no grandchildren).
    async fn fan_out(
        &self,
        children: Vec<SubagentSpec>,   // per-child: system, tool_scope, model, effort, sandbox_mode
        cancel: CancelToken,
    ) -> Vec<SubagentSummary>;         // summary, NOT transcript (P9)
}
```

- **Defaults mirror Codex verbatim:** `max_depth = 1`, `max_threads = 6`, per-child `model` /
  `reasoning_effort` / `sandbox_mode` overrides inheriting from the parent when omitted.
- **Maps onto ziee's existing fan-out machinery:** the concurrency bound is exactly `llm_map`'s
  `Semaphore::new(max_parallel)` with `MAX_PARALLEL_HARD_CAP` (`dispatch.rs:345`); the per-child
  token accounting folds into the parent `Budget` the same way `LlmMapDispatcher` aggregates
  `total_tokens` and self-aborts at `PER_STEP_TOKEN_CAP` (`dispatch.rs:510`). "Each subagent re-pays
  for its own context" — the token cost is real and the caps already exist.
- **Exposure:** fan-out is offered to the model as a built-in tool (`delegate`/`research_each`), so an
  orchestrator agent can say "research each of these 20 hits in parallel, summarize" — the headline
  life-science JTBD. It is also directly callable by the workflow host (a `kind: agent` step whose
  spec declares parallel children).

---

## 4. The ports (SDK seams)

> **SDK framing (see §0.5).** The ports *borrow* the SDK's pluggable-seam pattern — traits in the ziee
> **`agent-core` crate** (a ziee crate, not an SDK crate), `AgentCore` generic over an injected `Arc<P>`,
> just as `ziee-framework`'s `RequirePermissions<R: IdentityResolver>` is generic over the app's injected
> resolver. The host adapters supply the impls; the ports exist for **three-host reuse within ziee**,
> not cross-app genericity. Per the crate-boundary audit the count is **six** — the five below **plus
> `ModelResolver`** (`async resolve(model_id, user_id) -> Result<Arc<Provider>>`, the seam that lets
> `fan_out`/reviewer mint a per-child/reviewer provider without the crate touching the DB/RBAC; the
> direct analog of `IdentityResolver`). Pure ports use associated `Error` types (à la
> `TokenVerifier::Error`); the driver returns `ziee_core::AppError`.


Goose validates that the **Provider** and the **tool interface (`McpClientTrait`)** deserve to be
traits, but leaves **transcript store, event sink, human gate, and approval policy concrete**
(SQLite `SessionStorage`, the `BoxStream`, `ToolConfirmationRouter`, a `goose_mode` string). That is
the anti-pattern to fix: ziee is Postgres-backed and multi-host, so **all five are traits**. Each
port has a **chat impl**, a **workflow impl**, and (where relevant) a **subagent impl**.

### 4.1 `TranscriptStore` — turn history (Goose left concrete; make it a trait)

```rust
#[async_trait]
pub trait TranscriptStore: Send + Sync {
    async fn load(&self, run_id: Uuid) -> Result<Vec<Message>>;
    async fn append(&self, run_id: Uuid, msg: Message) -> Result<()>;           // one txn, like finalize()
    async fn replace_head(&self, run_id: Uuid, summary: Message, upto: usize) -> Result<()>; // compaction sink (P3)
    async fn journal_tool_call(&self, run_id: Uuid, rec: ToolCallRecord) -> Result<()>;      // P5 checkpoint
    async fn completed_tool_calls(&self, run_id: Uuid) -> Result<Vec<ToolCallRecord>>;        // P6 replay set
}
```

- **Chat impl:** `Repos.chat.core` — `append_content` (atomic MAX+1 seq), `get_message_with_content`,
  `conversation_summaries` for `replace_head`, and **`mcp_tool_calls` (migration 105) is already the
  `journal_tool_call` table** (owner-scoped, `result_json` capped+redacted, fire-and-forget insert on
  completion, links to run via `set_workflow_run`). Nothing new for the journal.
- **Workflow impl:** the agent step's transcript is held in `RunContext` and periodically flushed to a
  dedicated `agent_transcript_json` on `workflow_runs` (or, minimally, to `step_progress_json`);
  `journal_tool_call` reuses the **same `mcp_tool_calls`** rows the `ToolDispatcher` already writes
  (`guard.set_workflow_run(ctx.run_id)`, `dispatch.rs:1218`). This is the seam that makes crash-resume
  correct (§5).
- **Why a trait:** the core must be DB-shape-agnostic so chat (messages/message_contents) and workflow
  (`workflow_runs` JSONB) supply different storage, and unit tests supply an in-memory fake.

### 4.2 `EventSink` — push events out

```rust
#[async_trait]
pub trait EventSink: Send + Sync { async fn emit(&self, ev: AgentEvent); }
```

- **Chat impl:** wraps `stream/registry.rs::{publish_frame, publish_raw_event}` — maps `AgentEvent`
  onto `SSEChatStreamEvent` + the existing extension raw events. (The per-user SSE stream,
  `ChatStreamRegistry`, is unchanged.)
- **Workflow impl:** wraps the existing `Arc<dyn ProgressEmitter>` (`events.rs:309`) — maps
  `AgentEvent` onto `SSEWorkflowRunEvent`. **This port and the workflow `ProgressEmitter` are the
  same shape**, which is why the agent step forwards live progress for free (Idea 3 in the workflow
  research — "surface the tokens you already throw away").
- **Note (P10):** the sink is *out-of-band from durability*. Tokens stream through it continuously;
  the journal writes only at completion boundaries. This is Mastra's "don't persist running
  snapshots" discipline, and ziee already honors it (deltas stream over SSE; DB writes happen in
  `finalize()`).

### 4.3 `ToolProvider` — unify built-in + external MCP (Goose `McpClientTrait`)

```rust
#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn list(&self, scope: &ToolScope) -> Result<Vec<Tool>>;              // RBAC-resolved tool set
    async fn call(&self, run_id: Uuid, call: ToolUse, idem: IdempotencyKey) -> Result<ToolResult>;
    fn is_trusted(&self, server_id: Uuid) -> bool;                            // read-only built-in? (auto-approve)
}
```

- **Single impl, both hosts:** an `McpToolProvider` that wraps
  `mcp::client::manager::get_or_create_with_context(...)` + `session.call_tool(...)`. **The workflow
  `ToolDispatcher` already contains every load-bearing piece** and should be refactored so both the
  chat path and the agent-step path call one function: `resolve_tool_server` (RBAC + built-in
  resolution, `dispatch.rs:1033`), the **fail-closed disabled-server gates** (conversation-scoped and
  default, `dispatch.rs:1107-1173`), `render_tool_arguments` (type-preserving, `dispatch.rs:963`), and
  **`resource_link::persist_links`** (turns tool-produced files into durable artifacts,
  `dispatch.rs:1315`). The chat MCP extension (`helpers::execute_tool` → `session.call_tool`) is the
  other half; they converge here.
- `is_trusted` = the existing `is_builtin_server_id` / `is_trusted_resource_emitter` allow-lists —
  the input to the approval policy (§4.5), and Codex's `is_safe_command()` analog.
- `idem` (§5) threads into the tool-call context (the `ZIEE_STEP_KEY` idea generalized to
  `<run_id>:<turn>:<tool_ordinal>`).

### 4.4 `HumanGate` — approval / elicitation, **durability lives here** (P7)

```rust
#[async_trait]
pub trait HumanGate: Send + Sync {
    /// Ask a human to approve a tool call or supply input. Returns a ticket.
    /// The impl decides DURABILITY: chat = live pause; workflow = durable `waiting` gate.
    async fn request(&self, run_id: Uuid, ask: GateAsk) -> Result<GateOutcome>;
}

pub enum GateOutcome {
    Decided(ReviewDecision),        // resolved live (chat) or on a hot resume
    Suspended(GateTicket),          // the CORE must yield Stopped and let the host park the run (durable)
}
pub enum ReviewDecision { Approved, ApprovedForSession, Denied, Abort } // Codex verbatim
```

This is the port that carries the biggest design payload, because it is where the **two durability
tiers** live:

- **Chat impl (live pause, coding-agent-grade):** exactly today's behaviour — emit
  `mcpApprovalRequired` SSE, persist a pending-approval row, **truncate the turn**, and resume when
  the user re-sends the message with `tool_approvals` (`mcp.rs:1224-1330`). Non-durable across a
  restart, and that's fine for chat (the coding agents don't survive an interrupted turn either —
  `AGENT_ARCHITECTURE_RESEARCH.md §5`). Mechanically this is Goose's `ToolConfirmationRouter`
  register/deliver oneshot, made a trait.
- **Workflow impl (durable, Temporal/DBOS-grade):** the **existing `elicit` gate**. `request` →
  `persist_pending` (`pending_elicitation_json`) + `mark_status(Waiting)` + returns
  `Suspended(ticket)`; the core yields `Stopped(GateOpened)`; the `AgentDispatcher` returns
  `StepResult::Suspended`; the runner calls `set_no_runner` and keeps the workspace (`runner.rs:528`).
  On human submit, `submit_elicit` → `set_elicit_response` + `resume_run` → the agent step re-enters,
  reads `elicit_response_json`, consumes it exactly once, and continues (`dispatch.rs:1445-1474`,
  `elicit.rs:141-155`). **This is strictly stronger than Codex's in-memory oneshot map** — it
  survives a full restart (LangGraph's `interrupt()`/`Command(resume=)` across a process restart,
  keyed by the run journal).
- **Design invariant (from LangGraph):** resume matches the response to the gate **by identity**
  (`elicitation_id`) and the core **re-runs the interrupted turn from the top**, serving already-
  journaled tool calls from `completed_tool_calls()` (P6). So the human gate must be reached
  *deterministically* on replay — same rule as LangGraph's "don't conditionally skip/reorder
  interrupts."

### 4.5 `ApprovalPolicy` + the Reviewer — the safety matrix (P8, Goose `goose_mode` → a trait)

```rust
pub enum SandboxMode { ReadOnly{net:bool}, WorkspaceWrite{roots:Vec<PathBuf>, net:bool}, DangerFullAccess } // Codex
pub enum ApprovalMode { UnlessTrusted, OnRequest, Granular(GranularFlags), Never }                          // Codex

#[async_trait]
pub trait ApprovalPolicy: Send + Sync {
    /// Decide what happens to a tool call BEFORE it runs.
    async fn decide(&self, call: &ToolUse, tools: &dyn ToolProvider, sandbox: SandboxMode)
        -> Decision; // Auto | Prompt | Review | Deny
}
```

- **Composition (Codex):** `SandboxMode` is the *technical* boundary (what a `code_sandbox` tool may
  touch), `ApprovalMode` is the *gate* (what must ask). Effective default = `WorkspaceWrite` +
  `OnRequest` (Codex's default): read-only built-ins (`recall`, `search_knowledge`, `web_search`)
  auto-approve; mutating/external calls prompt. `ReadOnly + Never` = the unattended/CI mode ziee
  already has (the `"unattended"` metadata flag in `mcp.rs`).
- **The Reviewer (`auto_review`) — the piece ziee lacks and most needs.** Before a `Prompt` decision
  escalates to a human, run a **cheap LLM reviewer subagent** (an `AgentCore` fan-out child on a
  small/local model — the self-hosted Qwen engine at ~zero cost) that risk-classifies the call for
  **exfiltration / credential-probe / destructive / persistence**, **fail-closed** on any error
  (Codex): `Low → Auto`, `High → Prompt` (escalate to the durable `HumanGate`), `Critical → Deny`.
  Store the classification alongside the `mcp_tool_calls.result_json` for audit. **This is what makes
  an autonomous `kind: agent` run deployable for a life scientist who will not click "approve" 40
  times** (`OSS_AGENT_LANDSCAPE.md §7 P2-7`). The reviewer only vets calls that already need approval
  (Codex) — in-sandbox read-only calls skip it, so it costs nothing on the common path.
- **Escalation (Codex `with_escalated_permissions`):** a `code_sandbox` tool call denied by
  `SandboxMode` is **not** a hard failure — it emits a durable `elicit` carrying the *proposed*
  widened `roots`/`net`, and on approve re-runs with an amended per-conversation policy (an
  `ApprovedForSession` allow-rule row — Codex `blocking_append_allow_prefix_rule`). Never a silent
  full-access fallback.

### 4.6 Provider — already a port

`ai_providers::Provider::chat_stream(ChatRequest) -> Stream<StreamChatChunk>` with
`ContentBlockDelta::ToolUseDelta` and `FinishReason::ToolCalls` is **already** exactly Goose's
streaming-first `Provider::stream(...) -> MessageStream` shape (usage as a trailing frame). The only
change agentic use needs: pass `ChatRequest.tools` (the current workflow `LlmDispatcher` sends
`..Default::default()` — no tools; that's the whole reason "there is no LLM in a loop choosing tools"
in workflows today). No new abstraction.

---

## 5. The durability model

### 5.1 The rule: journal at the completed-tool-call boundary (P5)

DBOS, Temporal, and LangGraph agree unanimously: **the finest durable unit is a completed
side-effecting step; nothing journals mid-step.** This **corrects the prior framing** in
`OSS_AGENT_LANDSCAPE.md §7 P0-3` ("turn-level journaling is novel"): it is *not* novel, and trying to
checkpoint *inside* a tool call would diverge from every proven engine for no benefit. The right unit
is:

> **An agent turn is a sequence of durable steps; each step is one completed tool call, journaled to
> Postgres at completion. A model call has no external side effect, so it is safe to re-run (it just
> re-spends tokens) — it need not be journaled for correctness, only for cost accounting.**

Ziee already has the journal table: **`mcp_tool_calls`** (migration 105) records every tool
invocation at completion, fire-and-forget, capped+redacted `result_json`, linkable to a run. That is
DBOS's `operation_outputs` and Temporal's Activity-result record, already built.

### 5.2 The journal schema (what each host writes)

Per agent turn (`run_id`), the durable state is:

| Datum | Chat host | Workflow host |
|---|---|---|
| Transcript (messages so far) | `message_contents` (already) | `agent_transcript_json` on `workflow_runs` (new column) or `step_progress_json` |
| Completed tool calls (the replay set, P6) | `mcp_tool_calls` rows (already) | `mcp_tool_calls` rows w/ `workflow_run_id` (already) |
| Compaction summary | `conversation_summaries` (already) | `conversation_summaries` (agent step may set a `conversation_id`) |
| Pending human gate | pending-approval rows (already) | `pending_elicitation_json` (already) |
| Submitted gate response | re-sent message `tool_approvals` (already) | `elicit_response_json` (already, migration 110) |
| Run status | conversation generation slot | `workflow_runs.status ∈ {running, waiting, ...}` (already) |

**Almost nothing new** — the workflow host needs at most one JSONB column for the in-turn transcript;
everything else exists.

### 5.3 Crash-resume, mapped to `startup_sweep.rs` / `resume_run`

Two cases, both reusing the existing durable-resume plumbing:

- **Suspended at a human gate (already works).** `startup_sweep::fail_orphaned_runs`
  (`repository.rs:807`) flips only `pending`/`running` → `failed` and **spares `waiting`**; the staged
  dir is kept for `waiting` runs (`startup_sweep.rs:59`). The agent step, when it opens a durable gate
  (§4.4), returns `StepResult::Suspended` → the run is `waiting` → survives the restart → `resume_run`
  re-enters on submit. **Zero new mechanism.**
- **Crashed mid-loop, not at a gate (the new capability, Idea 1 of the workflow research).** Today an
  agent turn that crashes while `running` is flipped to `failed`. The durable-agent design (LangGraph
  `interrupt`-resume generalized to crash):
  1. `startup_sweep` marks a crashed agent run **`resumable`** instead of `failed` (a new terminalless
     status, reusing the `waiting` machinery: spared, dir kept) and re-arms it.
  2. `resume_run` re-enters the `AgentDispatcher`; the core loads the transcript, reads
     `completed_tool_calls(run_id)` (P6), and **re-runs the interrupted turn from the top**, serving
     journaled tool results from the replay set and re-issuing only the interrupted model call / the
     one un-journaled tool call. This is DBOS's "check before each step if checkpointed" and
     LangGraph's "restart the node from the beginning; replay committed task writes" — verbatim.
  - **Scope decision:** ship gate-suspend durability first (free), crash-mid-loop resume second (Idea
    1 + Idea 5). Chat opts *out* of crash-mid-loop resume (coding-agent tier — a dropped chat turn is
    just re-sent); only the workflow `kind: agent` host opts in.

### 5.4 Idempotency (P6) — the ZIEE_STEP_KEY generalization

Because resume re-runs pre-completion code (LangGraph/DBOS), an interrupted tool call may re-fire.
Model calls are safe (no side effect). Side-effecting tool calls (a `code_sandbox` write, an external
MCP write) get a stable **idempotency key** `<run_id>:<turn>:<tool_ordinal>` threaded through the
tool-call context (generalizing the workflow research's `ZIEE_STEP_KEY` / Idea 5, `Vercel
getStepMetadata().stepId`) so a re-executed call can dedupe. For built-in read-only tools this is a
no-op; only external/mutating tools consult it. This is the one genuinely new plumbing bit, and it is
small.

### 5.5 Why NOT Mastra's "agent turn = workflow run" (the load-bearing divergence)

Mastra makes `agent.generate()` build and run a `createWorkflow({ id: 'agentic-loop' })`, so one
durable substrate serves chat and workflows [Mastra source]. Adopting that literally in ziee would
mean **every chat token turn spawns a `workflow_runs` row and a DAG execution** — heavy, and a poor
fit because ziee's workflow runner is a *static-DAG topo executor* (Kahn's algorithm over
`depends_on`), not a dynamic tool-loop engine. Instead, ziee inverts the nesting: **the agent loop is
an activity, and the workflow step is the durable boundary** — which is exactly Temporal's model (a
complex Activity inside a durable Workflow) and matches ziee's runner precisely: `kind: agent` is one
`StepDispatcher` whose durable boundary is the step, and *within* it each tool call is a journaled
sub-step (DBOS). Chat, meanwhile, hosts the same loop with a *lighter* durability port and pays no
DAG/snapshot tax. The ports are what let ziee have Mastra's unification (one loop, both surfaces)
*without* Mastra's cost (every turn is a workflow). This is the single most important design decision
in the document.

---

## 6. Safety, approval & reviewer model (consolidated)

The full matrix an autonomous ziee agent runs under (Codex-analog, escalating to ziee's durable
gate):

```
                 SandboxMode (technical boundary — code_sandbox)
                 ReadOnly            WorkspaceWrite         DangerFullAccess
ApprovalMode  ┌───────────────────────────────────────────────────────────────
 UnlessTrusted│ read-only built-ins auto; everything else → Reviewer → gate
 OnRequest ★  │ default: mutating/external → Reviewer → gate; read-only auto
 Granular     │ per-category allow/deny flags; unflagged → Reviewer → gate
 Never (CI)   │ no prompts; denied calls fail back to the model (unattended runs)
```

- **Reviewer stage** (`auto_review`): a fan-out child on a cheap model classifies each
  approval-needing call → `Low`=proceed, `High`=escalate to `HumanGate` (durable elicit in the
  workflow host), `Critical`=hard-deny; **fail-closed**. Only runs for calls that already need
  approval (cost-free on the common read-only path).
- **Escalation:** sandbox-denied `code_sandbox` op → durable elicit carrying proposed widened perms →
  approve re-runs with an `ApprovedForSession` per-conversation allow-rule. Never silent escalation.
- **Ziee already owns 2 of 3 halves** (the hardened bwrap sandbox; per-tool MCP approval +
  disabled-server gates). The net-new work is (a) the explicit `SandboxMode × ApprovalMode` matrix
  and (b) the reviewer subagent. Both are thin given the ports.
- **Life-science ground truth as verification** (the "linters" analog): the `citations` verifier
  ("never invent a citation") and `knowledge_base` grounded-answer ("answer only from results; say
  not found") are wired as tools the agent self-corrects against — the factual-grounding equivalent
  of a coding agent's test suite (`AGENT_ARCHITECTURE_RESEARCH.md §8.4`).

---

## 7. The three-target integration (the payoff)

**One `AgentCore::run`, three hosts.** Each host is a small adapter that supplies ports and forwards
events.

| Concern | Target 1 — **Chat** | Target 2 — **Workflow `kind: agent`** | Target 3 — **Parallel fan-out** |
|---|---|---|---|
| Host code | `streaming.rs` becomes a thin `AgentCore` adapter | new `AgentDispatcher: StepDispatcher` (`dispatch.rs`) | `AgentCore::fan_out` orchestrator |
| Entry | `POST /conversations/{id}/messages` | `StepConfig::Agent { system, tools, max_steps, sandbox_mode, output_format }` (`validate.rs`) | a built-in `delegate` tool / a `kind: agent` step w/ children |
| `TranscriptStore` | `Repos.chat.core` + `conversation_summaries` | `workflow_runs` JSONB + `mcp_tool_calls` | fresh per-child `run_id` |
| `EventSink` | `SSEChatStreamEvent` via `ChatStreamRegistry` | `SSEWorkflowRunEvent` via `ProgressEmitter` | parent's sink, prefixed by child |
| `HumanGate` | live pause / re-sent-message resume (non-durable) | **durable `elicit` `waiting` gate** | inherits parent's gate |
| `ApprovalPolicy` | interactive (`OnRequest`) | interactive OR headless+Reviewer | per-child `sandbox_mode` |
| Durability tier | transcript resume (coding-agent grade) | **journaled step resume (DBOS/Temporal grade)** | child failure ⇒ degraded summary (like `llm_map on_error`) |
| Caps | `loop_settings.max_iteration`, model `max_tokens` | `PER_RUN_TOKEN_CAP` 5M / `PER_STEP` 2M / wall-clock 30m | `Semaphore(max_threads=6)`, `max_depth=1` |
| Cancellation | generation slot / stop-token | `RunHandle::await_cancel` | parent cancel propagates |

**Target 1 (chat replaces its loop with the core).** The generic loop skeleton lifts out of
`streaming.rs`; the current chat extensions split cleanly into (a) *system-block contributors*
(assistant, project, skill, file, memory-injection → `AgentTurnRequest.system`), (b) the *tool host*
(the MCP extension → the `ToolProvider` + `ApprovalPolicy` impls), and (c) *compaction* (the
summarization extension → `Compactor`, now in-core). Behaviour is preserved; the win is that this
same core now has a second and third consumer.

**Target 2 (workflow `kind: agent`).** A new `StepConfig::Agent` variant (the DB already anticipates
it — `workflow_runs.invocation_source` CHECK already permits `'agent'`) and a new `AgentDispatcher`
that constructs an `AgentCore` with workflow ports and calls `run(...)`, returning `StepResult::
Completed` (folding agent tokens into `ctx.total_tokens`, honoring `PER_STEP_TOKEN_CAP`) or
`StepResult::Suspended` when it opens a durable gate. This **legitimizes the dead `tools:` field**
(`WORKFLOW_DEAD_TOOLS_FIELD`, `validate.rs:589`) under the new kind. It reuses `resolve_tool_server`,
the disabled-server gates, `render_tool_arguments`, and `resource_link::persist_links` verbatim
(§4.3). The exhaustive `match &step.config` sites that must gain an `Agent` arm are enumerated:
`runner.rs:736` (dispatch), `runner.rs:1221/1345` (`require_model`), `validate.rs:224` (`kind_str`),
`cost.rs:78/96` (estimate/dry-run), `types.rs:151` (`StepKindTag`).

**Target 3 (parallel fan-out).** `AgentCore::fan_out` (§3.5) with Codex defaults, mapped onto
`llm_map`'s semaphore + caps. A `kind: agent` step whose spec declares children *is* an
orchestrator-workers workflow step; the chat orchestrator gets the same via a `delegate` tool. The
return contract is enforced structurally: `fan_out` returns `Vec<SubagentSummary>`, never child
transcripts (P9).

### 7.1 Host surfaces — committed (3) + latent (~4). Size the ports for ~7.

The three targets above are the *committed* scope. But "one core, N hosts" is not ziee's invention —
it is what every mature agent does, and the count is the strongest argument for the ports. **Confirmed
from primary sources this session**, each project runs ONE core loop from many surfaces, none forking
the loop:

| Project | The one core | Distinct host surfaces (confirmed) | N |
|---|---|---|---|
| **Codex** | `codex-core` `CodexThread` (every crate depends on it) | TUI · `codex exec` (headless/CI) · `codex cloud` (delegated) · `app-server` → IDE/desktop/web · `@openai/codex-sdk` · `codex mcp-server` · subagents `[agents]` · `codex review` (GitHub Action) | **8** |
| **Goose** | `goose::agents::Agent::reply` (no second loop) | `goose session` · `goose run` (headless) · `goose serve` → Desktop/Web (ACP) · `goose acp` (stdio/IDE) · scheduler (`Agent::new()…reply` per job) · subagents/subrecipes · `goose-sdk` · (+`goose gateway`) | **7–8** |
| **Mastra** | `agentic-loop`; durable agents "run **the same loop** as `Agent.stream()`" (docs verbatim) | chat agent · agent-as-workflow-step · durable workflow · evented/Inngest agent · agent/workflow-as-tool + networks | **5** |
| **LangGraph** | one `compile()`d Pregel runtime | local `invoke`/`stream` · Platform/Server + assistants · Studio · embedded subgraph · agent-as-tool | **5** |
| **Letta** | one stateful agent server (`:8283`) | REST · SDK · ADE GUI · multi-agent · sleep-time agents | **5** |

The two **same-stack Rust twins are the highest** (8 and 7) — direct evidence that the core+ports
boundary scales to that many hosts *in Rust* without forking the loop. Ziee's realistic count is
**~7**, because the same substrate the leaders count as separate hosts, ziee already owns:

| Ziee host | Status | Nearest analog | Durability tier / port wiring |
|---|---|---|---|
| Chat | **Target 1** | Goose `session`, Codex TUI | live gate (transcript resume) |
| Workflow `kind: agent` | **Target 2** | Mastra durable step, LangGraph subgraph | **durable `elicit` gate** (journaled resume) |
| Parallel fan-out | **Target 3** | Codex `[agents]`, Goose subagents | inherits parent gate |
| Desktop (Tauri) | latent, ~free | Goose `serve` → Desktop | live gate — chat-on-core ships to desktop automatically (desktop embeds the server) |
| Scheduled / unattended | latent | Goose scheduler, Letta sleep-time | **durable gate + `ApprovalMode::Never` + Reviewer** stands in for the human |
| MCP-exposed agent (A2A-lite) | latent | Codex `mcp-server`, Letta multi-agent | a `kind: agent` workflow is already surfaced as a `wf_<slug>` MCP tool → an agent callable by another agent, **no new plumbing** |
| Standalone / headless run | present | Codex `exec`, Goose `run` | the no-conversation workflow run the `AgentDispatcher` already supports |

**This is *why* durability is a port (§4.4), not a baked-in property.** The scheduled/unattended and
MCP-exposed hosts need the *durable* gate + Reviewer; chat/desktop need the *live* one — same loop,
different port wiring. Codex and Goose prove the pattern reaches 7–8 hosts precisely because they kept
these seams as one core rather than forking per surface. **Design rule:** don't hard-code the three;
make every port impl selectable at host-construction time so a 4th–7th host is an adapter, not a
core change.

### 7.2 The cross-app dimension (SDK) — one agent, many apps via `control_mcp`

The SDK adds a target the pre-SDK design couldn't: **ziee's single agent operating *other apps*.**
CytoAnalyst (app #2 on the SDK) is **purely companion-driven** — it runs *no* agent of its own; it
exposes its REST API as MCP tools via **`control_mcp`** and ziee's agent drives it. This needs **no
change to the agent core**: `control_mcp`'s three-tool catalog (`list_capabilities` /
`describe_capability` / `invoke_capability`, permission-filtered, mutation-approval-gated,
forwarded-JWT loopback re-auth — the DB-free dispatch core is the SDK crate `ziee-control-mcp`) is
surfaced as **just another `ToolProvider`**. So a fourth integration story sits atop the three:

| Target | What the agent core sees | Where it lives |
|---|---|---|
| 1 Chat / 2 Workflow / 3 Fan-out | its own ports (this app's tools, transcript, gate) | ziee-app hosts |
| **4 Companion-app control** | a `control_mcp` `ToolProvider` pointed at another app's OpenAPI | the driven app exposes `ziee-control-mcp`; ziee holds the MCP client |

The safety model composes cleanly: `control_mcp` already classifies each capability
(`policy::is_denied`, mutation→approval), so a companion-app mutation flows through the *same*
`ApprovalPolicy` → Reviewer → durable gate path as any other tool (§6). The result is the
**"OS of apps / companion-AI" model** — **ziee is the one agent** (its `agent-core` crate is ziee-only);
every *other* SDK app is companion-driven (exposes `control_mcp`), not agent-having. ziee's agent is the
AI operator for the whole family, reaching each app through its `control_mcp` surface.

---

## 8. The agent core as a ziee crate (built on the SDK, not in it)

The agent core is a **ziee-app crate** — `src-app/agent-core` in the *ziee* workspace (the DEC-15/DEC-18
crate, unchanged in shape), a sibling of `ai-providers`. It is **not** an SDK crate and **not**
domain-neutral; it is a first-class ziee feature that *consumes* the SDK the same way the rest of the
ziee app does.

**Crate shape:**
- **Lives at `src-app/agent-core`** (ziee workspace member). Build-DB-free, no `sqlx` compile-time
  macros. Deps: **`ai-providers`** (app-side, unchanged) for `Provider` + message/tool types; **`ziee-core`**
  for `AppError`/`ApiResult`/macros; **`ziee-identity`** for `Principal`/`PermissionCheck` (to gate tools
  by permission). May name ziee domain freely — **no N9**.
- **Ports = leaf traits in the crate** — the pattern *borrowed* from `ziee-identity`: sync ports mirror
  `Principal`/`TokenVerifier`; async ports (`ToolProvider`, `HumanGate`, `ModelResolver`) are
  `#[async_trait]`. `AgentCore` is generic over the injected `Arc<P>` set. The wall buys a
  compiler-enforced port boundary *within ziee*.
- **Returns `ziee_core::AppError`** on the driver (app-wide-consistent now); pure ports keep associated
  `Error` types. (This supersedes the bespoke `AgentError`.)
- **Domain tools/grounding/reviewer-policy/nudges are supplied by ziee** through `ToolProvider` +
  `AgentExtension` — for reuse across ziee's three hosts, not for neutrality. `INV-8`/`TEST-36` is the
  plain crate-dependency-boundary test (deps = `ai-providers` + `ziee-core` + `ziee-identity`, not the
  whole server).
- **`AgentExtension` registry + the three hosts stay app-side** (chat/workflow/mcp construct `AgentCore`;
  server owns the `distributed_slice`, DEC-18).

**Two SDK touch-points that DO matter** (the core is built *on* the SDK):
1. **It deps SDK crates** (`ziee-core`, `ziee-identity`) — the same platform the rest of ziee stands on;
   so it inherits the app-wide `AppError`, permission model, etc.
2. **It drives *other* apps via an SDK crate** — the `ToolProvider` surfaces `control_mcp`
   (`ziee-control-mcp`) tools, so ziee's one agent operates companion apps (CytoAnalyst) with no core
   change (§7.2).

`kind: agent` remains the **bridge** where the agent primitive plugs into the (app-side) workflow engine;
the workflow's durability supplies the `HumanGate`/journal ports. Goose proved the loop+provider+tool
split is a sound Rust framework boundary; ziee's `agent-core` advance is that
**transcript/gate/policy/model-resolution are traits too** (Goose left them concrete) and it adds a
*durable* gate Goose never had. It is **ziee's** agent engine, standing on the shared SDK platform and
— via `control_mcp` — the AI operator for the whole companion-app family.

---

## 9. Sequencing (extraction order — build, don't boil the ocean)

Ordered so each step ships value and de-risks the next (matches `AGENT_ARCHITECTURE_RESEARCH.md §9`):

0. **Scaffold the ziee `agent-core` crate** — `src-app/agent-core` (ziee workspace member; deps
   `ai-providers` app-side + `ziee-core` + `ziee-identity`). No `ai-providers` relocation, no SDK crate.
1. **Define the six port traits + `AgentCore::run`** in `agent-core`, backed by in-memory fakes.
   Unit-test the loop, compaction, and the gate state machine with fakes (Goose/Mastra both testable
   this way).
2. **Land `kind: agent` on it first (greenfield, no migration risk).** Workflow ports = existing
   `ProgressEmitter` + `ToolDispatcher` seams + the durable `elicit` gate. Proves the durability port
   against the hardest tier. This is Idea 2 of the workflow research, now on a real core.
3. **Add the Reviewer + the SandboxMode×ApprovalMode matrix** (§6) — makes the autonomous step
   deployable for non-developers.
4. **Add crash-mid-loop resume** (§5.3, Idea 1 + idempotency keys Idea 5) for the workflow host.
5. **Migrate chat onto the core** (Target 1) — the biggest diff, done last when the core is proven;
   behaviour-preserving, gated by the existing chat e2e + the gallery runtime-health pass.
6. **Expose `fan_out`** as a built-in `delegate` tool + a `kind: agent` children spec (Target 3).

---

## 10. Open decisions / risks (for the discussion)

- **D1 — transcript storage for the workflow host.** One new `agent_transcript_json` JSONB column vs.
  reusing `step_progress_json` vs. a child `agent_turns` table. Recommendation: a dedicated column,
  keeping the journal (`mcp_tool_calls`) as the source of truth for replay.
- **D2 — how much of chat's extension behaviour is "system-block contributor" vs. must stay an
  extension.** The clean split (§7 Target 1) needs validation against `control_mcp`, `js_tool`,
  `knowledge_base` extensions, which do more than inject system blocks.
- **D3 — does chat want crash-mid-loop resume at all?** Recommendation: no (coding-agent tier); only
  the workflow host opts in. Revisit if users report lost long chat turns.
- **D4 — reviewer model + cost.** The self-hosted Qwen engine at ~zero cost is the intended reviewer;
  confirm latency is acceptable on the approval path (it only runs for approval-needing calls).
- **D5 — `max_depth > 1`.** Codex defaults to 1 (no grandchildren) for predictability + token cost;
  keep 1 unless a real recursive-research JTBD emerges.
- **R1 — streaming vs. journal interleaving (Mastra's sharp edge).** Enforce P10 (snapshot only at
  boundaries, stream out-of-band) as a core invariant; ziee already honors it, but the agent step's
  transcript flush must not race the token stream.
- **D6 — RESOLVED (human): agent core + `ai-providers` stay in ziee, ziee-only.** Not SDK crates. So:
  no `ai-providers` relocation; the `agent-core` crate is `src-app/agent-core` (ziee workspace); no N9
  domain-neutrality constraint. This removes the largest prerequisite the SDK framing had introduced.
- **D7 — SDK alignment confirmations.** (a) Error: driver → `ziee_core::AppError`, pure ports →
  associated `Error` (supersedes the bespoke `AgentError`, now app-wide-consistent). (b) The crate deps
  `ai-providers` (app-side) + `ziee-core` + `ziee-identity`; `ziee-framework` only in the app-side host
  adapters that mount routes. (c) `INV-8`/`TEST-36` reverts to the plain crate-dependency-boundary test
  (no whole-server dep), NOT an N9 grep.
- **D8 — companion-app control (§7.2) scope.** Is "ziee's agent drives CytoAnalyst via `control_mcp`"
  in the first agent-core milestone, or a fast-follow once CytoAnalyst boots? It needs *no* core change
  (a `control_mcp` `ToolProvider`), so it can land whenever CytoAnalyst is ready.

---

## Appendix — decisions ↔ primary sources

| Decision | Source (read first-hand this session) |
|---|---|
| Loop returns a coarse event stream; tools inside messages | Goose `crates/goose/src/agents/agent.rs` (`AgentEvent`) |
| Provider is streaming-first, `complete` derived | Goose `goose-provider-types/base.rs`; ziee `ai-providers` already matches |
| Compaction in-core, sliding 30/70 escalate; summary→buffer, raw→recall | Letta compaction + memory-blocks docs |
| Retrieval/curation stays a tool | Letta `core_memory_*`/`archival_memory_*` |
| Journal at completed-tool-call boundary; never mid-call | DBOS `operation_outputs`; Temporal Activity; LangGraph `checkpoint_writes` |
| Resume re-runs turn from top, serves completed calls from journal | LangGraph interrupts ("restarts the node from the beginning"); DBOS checkpoint-check |
| Idempotency key for side-effecting tool calls | LangGraph `@task`; Vercel `getStepMetadata().stepId` |
| Durability as a per-host port, not baked in (reject Mastra's turn=workflow) | Mastra `agentic-loop/index.ts` (what they did) vs. Temporal activity-in-workflow nesting (what we do) |
| Durable human gate stronger than in-memory oneshot | Codex `approvals.rs` oneshot vs. ziee `elicit` `waiting`+`resume_run` |
| SandboxMode × ApprovalMode × Reviewer, escalate to gate, fail-closed | Codex `SandboxPolicy`/`AskForApproval`/`auto_review` |
| Subagents bounded (`max_depth=1`, `max_threads=6`), summary not transcript | Codex `[agents]`; Anthropic subagent isolation |
| continue-as-new ≈ compaction bounds unbounded history | Temporal continue-as-new |
| Stream out-of-band; snapshot only at boundaries | Mastra "don't persist running snapshots" |

---
*End of design. No code written; no push. Next step: discuss, then a `feature-lifecycle` plan for
sequence step 1–2.*
