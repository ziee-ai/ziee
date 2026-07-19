# RESEARCH — State-of-the-art agent orchestration (2026) vs the ziee menu

> Deep, primary-source research pass requested by the human before locking scope:
> "is the 5-group menu enough / are we on the right track, and how do others do the
> background-job + spawn-sub-agent backbone?" Five parallel research agents worked
> from official docs + repos (Anthropic/OpenAI/Cursor/Goose/DBOS/LangGraph/Mastra/
> Cloudflare/CrewAI/AutoGen/Temporal/Restate/Inngest). Confidence is flagged per
> finding; primary-fetched claims are quoted, secondary/community claims marked.
> This is Phase-1 research — nothing implemented.

## Headline verdicts (the two answers the human asked for)

1. **Are we on the right track?** **Yes, strongly — and on several axes ziee's design is byte-for-byte the industry consensus.** But the research surfaced (a) **~5 completeness gaps** worth adding (led by *goal-seeking/verification loops* and a *unified inbox*), and (b) a set of **durability + hardening corrections** where ziee's current sketch is behind SOTA (persisted state machine + boot orphan-reclaim; output backpressure; idle timeouts; a "needs-input" reply state).

2. **The backbone (A/B/C) — REVISED recommendation:** the prevailing SOTA is **ONE durable-run primitive with a `kind` discriminator + decentralized kind-registration, where "spawn sub-agent" is just a run-kind, not a second subsystem.** So the answer is a **hybrid: Option A as the substrate (generalize `workflow_runs` with a `kind`), wrapped in Option B's ergonomic facade (`spawn_background/check_status/collect_result` + a `JobKind` trait registry), with Option C folded in as one kind (`JobKind::SubAgent`).** Do NOT build a separate `background_jobs` table — that is the exact split the field is actively *unwinding*. Closest prior art: **DBOS** (Postgres, checkpoint-camp, one workflow primitive) + **Goose** (`execute(RecipeSource, ExecutionMode{Interactive,Background,SubTask})`).

---

## 1. Sub-agents (Group A) — ziee's design IS the consensus

**Every serious system converges on ziee's exact contract:** an isolated child context → **only the final summary returns** → shallow depth → bounded concurrency → per-child model/tool restriction.

| System | Spawn interface | Return contract | Depth cap | Concurrency | Source |
|---|---|---|---|---|---|
| **ziee** | `delegate` tool → `fan_out` | **merged summaries** | **1** | **6** (`max_threads`) | (built, dormant) |
| **Codex `[agents]`** | `.codex/agents` TOML | **consolidated summaries** (barrier) | **1** (default) | **6** (default) | learn.chatgpt.com/docs/agent-configuration/subagents |
| Claude Code | `Agent` tool (was `Task`) | final message only | 5 (fixed) | unbounded; **200/session** cap | code.claude.com/docs/en/sub-agents |
| Claude Agent SDK | `AgentDefinition` / `agents=` | final message only | 5 | `Workflow` tool for 100s | code.claude.com/docs/en/agent-sdk/subagents |
| OpenAI Agents SDK | `agent.as_tool()` | final output → tool result | none | user code (`asyncio.gather`) | openai.github.io/openai-agents-python |
| CrewAI hierarchical | manager + "Delegate to coworker" | output→context | ~1 practical | async | docs.crewai.com |
| AutoGen | GroupChatManager / nested | `last_msg` \| `reflection_with_llm` | nesting | conversation | microsoft.github.io/autogen |
| LangGraph | `Send()` / `create_handoff_tool` | shared-state reducers | subgraphs | Send fan-out (unbounded) | langgraph-supervisor |

**ziee's `max_depth=1, max_threads=6` is IDENTICAL to Codex's shipped defaults**, and the summary-only contract matches Claude Code / OpenAI as-tool / CrewAI / AutoGen. The three most important decisions (summaries not transcripts; shallow depth; per-child model+tool scope) were all made correctly.

**Additive gaps worth building (ranked, evidence-based):**
- **A-gap-1 — Untrusted-output scanning of child summaries (HIGH, cheap).** Claude Code scans every sub-agent's final message for injection (`<system-reminder>`, `Human:`/`Assistant:`, permission-string imitation) *before the parent reads it*. ziee children run `bio_mcp`/`web_search`/`lit_search` (untrusted third-party content) → **the merged summary is a prompt-injection vector into the parent.** Highest-value add.
- **A-gap-2 — Named, reusable agent definitions.** Everyone has them (`.claude/agents`, `.codex/agents` TOML, CrewAI agents, ADK `sub_agents`) — description-driven, reusable, enabling auto-delegation + a spawn allowlist. ziee has only ad-hoc per-call `tool_scope`+`model`. Ergonomics/governance gap.
- **A-gap-3 — Cumulative spawn budget.** `max_threads=6` bounds *concurrency* but not *total* spawns. Claude adds a separate **200/session** cap. Add a cumulative cap.
- **A-gap-4 — Streaming child progress.** Barrier-then-merge yields no live progress; Claude/Cursor show per-child panels. Emit per-child heartbeat/progress even though the *result* merges at the barrier.
- **A-gap-5 — Per-child sandbox/approval mode.** ziee has per-child `tool_scope` (good); Codex adds per-agent `sandbox_mode`, Claude per-sub-agent `permissionMode`. Pin an explicit per-child sandbox/approval rather than pure inheritance for write-capable children.

**Correctly deferred:** agent-to-agent messaging / **teams** (Anthropic ships `agent-teams` **experimental, off by default, "significantly more tokens," no nesting** — wrong altitude for non-technical scientists; ziee's delegate→merged-summaries is the right default); recursion (`max_depth>1`); handoff/control-transfer; worktree isolation (until children can write).

---

## 2. Background sub-agents (Group B) — right model, two durability corrections

Every SOTA system (Claude Code Agent View + supervisor daemon; Codex cloud tasks; OpenAI Responses `background=true`; Cursor Cloud Agents; Anthropic Managed Agents work-queue) converges on: **fire-and-forget create → returns an opaque handle → task enters a durable queue decoupled from the caller → poll status OR subscribe to a resumable stream → completion delivered via a thin webhook/notification the client refetches → survives disconnect.**

- **ziee is already architecturally aligned + ahead in one way:** ziee's **notify-and-refetch `sync` bus** (`{entity,action,id}` → client refetches the permission-checked REST endpoint) is *exactly* OpenAI's deliberately-thin webhook (`data:{id}` only, "call back to fetch, to avoid stale-on-retry") and Cursor's thin+summary+link shape. **Reuse it — a background task = a `SyncEntity` + a `notification` inbox row; the right-panel is the "task dashboard." Do not invent a new channel.**

**Corrections (where ziee's sketch is behind):**
- **B-gap-1 — Persisted state machine + boot orphan-reclaim (REQUIRED).** ziee's chat turn runs on `tokio::spawn` + an in-memory `is_generating` flag — that **does not survive a server restart**. Claude Code's supervisor persists `roster.json`/`state.json` and, after a hard stop, marks sessions failed and **restarts them from where they left off**; cloud systems persist server-side. "Results land when done" only holds across a deploy with a **durable task row + a startup sweep** (which `workflow_runs`/`startup_sweep` already provide — this is the backbone argument, §5).
- **B-gap-2 — Explicit state machine incl. `needs_input` (HIGH value).** Replace the boolean flag with `queued→running→{completed|failed|cancelled}` **plus a `needs_input` state with a reply affordance.** Every leader bubbles "needs input / requires_action / ready for review" to the top of the dashboard; a background task that hits tool-approval or ask-user otherwise silently stalls.
- **Run OUTSIDE the per-conversation single-flight lock** (the whole point is "spawn and keep chatting"); give background tasks their own per-user concurrency domain + a cap + retention/prune. Split cheap `check_status` from heavy, **idempotent, paged** `collect_result` (mirror `tool_result_mcp` paging). Make the handle an **opaque owner-scoped id** (cross-user → 404). Cancel is first-class.

---

## 3. Background sandbox exec (Group C) — right core insight, four concrete traps

The core decision — **move the `Child` + its RAII guards (cgroup/seccomp/progress-FIFO) out of the request stack frame into a registry that outlives the call** — is exactly what Claude Code (`run_in_background`/`BashOutput`/`KillShell`), Codex (`UnifiedExecProcessManager` / `ProcessStore`, LRU-capped at 64), and reference impls (`agent-exec`) all do. Adopt the **three-verb shape**: `execute_command(run_in_background:true)→{run_id}`, `get_command_output(run_id, offset?, max_bytes?)`, `kill_command(run_id)`.

**Traps to design around (each traces to a filed incumbent bug):**
- **C-gap-1 — Output backpressure (biggest correctness trap).** In background you cannot stop draining the child's pipes or the OS buffer fills and **the child blocks on write and stalls**. A dedicated task must drain both pipes continuously into a **ring / head+tail buffer** or **spill to a per-run file** in the workspace. ziee's current **1 MiB hard drop-after cap is wrong for background** (it loses the *recent* tail); use head+tail (Codex `HeadTailBuffer` precedent) or on-disk byte-range-paged logs (`agent-exec`).
- **C-gap-2 — Output paging by BYTE-RANGE, not a consuming cursor.** `get_command_output(offset, max_bytes)` returning `total_bytes`+`next_offset` is idempotent (safe re-reads after a dropped turn); Claude Code's consuming cursor is not.
- **C-gap-3 — Timeout policy (Codex #5948 cautionary tale: "unified exec never auto-detaches" from `tail -f`/`npm run dev`).** Do NOT reuse the synchronous 600s verbatim and do NOT go timeout-less: use an **absolute max lifetime + an idle/no-new-output reaper + bind the run's lifetime to the conversation/sandbox teardown**; report `timed_out` distinctly from `killed`/`exited`.
- **C-gap-4 — Registry reaping (Claude Code #11190: finished shells never reaped → immortal "still running" reminders burning context).** On exit → transition to a **terminal state, record exit_code, STOP advertising running**, keep briefly for one final read, then **prune on every path** (kill / natural exit / idle TTL / conversation-sandbox teardown / **server shutdown** — the registry must OWN the RAII guards so `kill -9` of the server cascades). **Kill the cgroup**, not just the pid (reaps grandchildren — beats both incumbents). **Re-apply all hardening** on the new path (Codex #14367: a new exec path bypassed the sandbox on Windows). Prefer **notify-on-exit over the SSE seam** (beat Claude Code/Codex poll-only).

---

## 4. Schedule / loop (Group E) — dead-on, plus a distinct missing axis

**The merged Once/Recurring/Self-paced dialog is exactly SOTA.** Claude Code's `/loop` is literally *one command whose behavior depends on what you provide*: interval+prompt → cron; **prompt only → self-paced (model picks the next delay 1min–1hr and PRINTS the reason)**; nothing → maintenance prompt. Self-paced **self-stops** (`ScheduleWakeup(stop:true)`) with a **7-day auto-expiry backstop**. One-time reminders are pure NL ("remind me at 3pm"). Everything is **NL-first with cron/RRULE as an advanced escape hatch** (ChatGPT Scheduled Tasks, Codex Automations both agree). Delivery = **inbox + notification + optional continue-in-chat** (Codex "chat-embedded" = ziee's `continue_chat`). Autonomy needs an explicit permission story (Routines "no approval prompts," Codex `approval_policy=never`) = ziee's **unattended allow-list**. ziee reuses its whole existing `scheduler` module (`src-app/server/src/modules/scheduler/`) — the correct call.

**Self-paced mechanics to mirror:** after each fired turn compute next delay; **show the delay + the reason**; let it **self-stop** when done; keep a **hard max-horizon backstop**. Implement as a *self-rescheduling one-off* on the existing cron+once backbone.

---

## 5. The backbone (Group D) — REVISED recommendation with prior art

**The "one abstraction or two?" question, answered by the field:**

| System | Seam shape | New kind without a central switch? | job vs sub-agent |
|---|---|---|---|
| **DBOS** (Postgres, checkpoint-camp) | **status table + step-output table + decorator registry** | **Yes** (decorate a fn) | **ONE** — sub-agent = child workflow of the same primitive |
| **Goose** (Rust) | MCP/extension registry + recipe-as-unit; SQLite session | **Yes** | **converging TWO→ONE**: Discussion #4389 → `execute(RecipeSource, ExecutionMode{Interactive,Background,SubTask})` |
| Restate (Rust) | durable-step journal + virtual object | Yes | **ONE** (`call` vs `send`) |
| Inngest/Trigger | durable-step journal + fn-id registry | Yes | **~ONE** (`step.invoke`/`triggerAndWait`) |
| Mastra (TS) | two engines on one store | semi-central | **TWO→converging** (2026 "durable agents" moves the loop into the workflow engine) |
| Temporal | event-history journal (replay/exactly-once) | Yes | **TWO** (Activity ≠ Child Workflow) — determinism-camp reason |
| Cloudflare | actor / durable-object-per-task | binding | **THREE** — actor *is* the substrate |
| **LangGraph** | graph node + checkpointer | **NO — central `StateGraph` edit** | ONE, but centralized (anti-pattern for ziee) |

**Conclusion:** the majority + the *trend* is ONE durable-run primitive with a `kind`/mode discriminator + **decentralized** kind registration; sub-agent-spawn is a *variant* of the run/step primitive. Goose and Mastra are spending engineering *right now* to collapse a previously-split design into one. The two-primitive camp (Temporal, Cloudflare) splits for substrate-specific reasons that **don't apply to ziee** (ziee is checkpoint-camp, Postgres, not actor/replay). LangGraph's central-graph unification is the **anti-pattern** for ziee's decentralized culture (built-in-MCP id registry, `auto_attach` lists, chat-extension order slots).

**ziee's camp = checkpoint / at-least-once = DBOS/LangGraph/Cloudflare-Workflows** (not Temporal/Restate exactly-once). **Closest prior art = DBOS: Postgres-native, checkpoint-camp, ONE workflow primitive, status table + step-output table, decorator/kind registry — and DBOS's `workflow_status` table is architecturally the same shape as ziee's existing `workflow_runs`.**

### REVISED recommendation: A-substrate + B-facade + C-as-a-kind
- **A is the skeleton:** generalize `workflow_runs` with a `kind` discriminator + a compact typed per-kind jsonb payload. A background job = a 1-step run; a sub-agent = a run whose single step *is* the agent loop (literally DBOS "a workflow can be one step" + Goose `ExecutionMode::SubTask`). Reuse the runner/heartbeat/`RunHandle` SSE/snapshot-on-connect/`startup_sweep`/`SyncEntity::WorkflowRun`/notification you already paid for.
- **B is the skin (KEEP its API + registry):** the uniform `spawn_background/check_status/collect_result` surface + a **`JobKind` trait registry** (decentralized — matches Temporal Worker registration / DBOS decorators / Inngest fn-ids / ziee's own built-in-MCP registry). **But back it by `workflow_runs`, not a new `background_jobs` table.**
- **C is one bone:** the agent-core "detached turn" is `JobKind::SubAgent`, not a standalone seam (standalone C re-forks the exact split everyone is collapsing, and doesn't cover non-agent work like a batch reindex or a scheduled digest).

**Why NOT a separate `background_jobs` table (my initial Option-B instinct):** two durable substrates = two orphan-sweeps, two status models, two SSE/sync/notification/retention pipelines, and a permanent "is this a job or a workflow?" ambiguity — precisely the four-fragmented-paths mess Goose is spending a design cycle to *delete*.

**Biggest risk: semantic overload of the `workflow_runs` schema.** Mitigation: a `kind` discriminator + compact typed jsonb payload (never kind-conditional nullable-column sprawl); make the step/journal **optional**; and give each `JobKind` its **own** orphan-sweep / flap-cap / concurrency / retention policy (a token-heavy LLM sub-agent needs different limits than a fire-and-forget export — copy Goose's concrete sub-agent caps: **≤25 turns / 5 min / ~10 concurrent / no recursion**, rather than reusing the generic workflow limits).

---

## 6. Completeness sweep — 5 gaps beyond the original 5 groups (ranked for a non-technical life-science audience)

1. **Goal-seeking / verification loop (`/goal` analog) — HIGHEST.** A **different axis** from scheduling: keep working across turns until an *independent* (cheap Haiku-class) evaluator confirms a completion condition ("done when the QC figure passes / no missing values"). Directly answers Routines' own "**a green status does not mean the task succeeded**" warning — grounds trust for users who can't read a transcript to judge success. Cheap on ziee's existing evaluator-model + workflow + memory; folds naturally into the Group-E dialog as an optional "done when…" condition.
2. **Steer a running agent (HIGH).** Nudge / redirect / queue a note to a background sub-agent or long sandbox run **without killing it** (SOTA: agent-view peek+reply, type-while-working, Esc-interrupts-a-turn-not-the-session). Avoids restart-from-scratch on long analyses. Applies to Groups B/C.
3. **Unified background-agent inbox/dashboard (HIGH-MEDIUM).** Every leader converged on ONE consolidated surface (Claude `claude agents`, ChatGPT + Codex "Scheduled" inbox) with state + peek + unread + results across all background/scheduled work. **This is the connective tissue that makes Groups B/C/D/E feel like one system rather than five features.** ziee has per-surface status + an activity timeline but not the one inbox.
4. **Event-driven "monitor & notify" triggers (MEDIUM-HIGH).** A time-scheduler (cron+once) can't express "notify me when the sequencing run finishes / this dataset changes / this file appears" — a top real JTBD for scientists. Both ChatGPT and Codex ship monitor+notify; Claude offers Channels/Monitor (stream, not poll). Add an event/completion trigger alongside cron in Group D/E; prefer event-push over Group-C polling where possible.
5. **Live agent TODO checklist (MEDIUM).** A live "plan → steps checking off" surface complements the existing plan-preview + timeline; reassuring for non-technical users. Cheap; lower urgency.

**Correctly OUT of scope for this audience (research-confirmed):** agent teams / agent-to-agent messaging / mailboxes / split-panes (experimental even in Claude Code, token-heavy, coordination overhead the docs warn against); user-facing hooks config, git worktrees, tmux panes (dev primitives — but surface the *lifecycle events* TaskCreated/TaskCompleted/Stop **internally** on the Group-D backbone); GitHub/API/webhook triggers as a user menu (keep as an *internal* integration capability for instrument/pipeline callbacks).

---

## 7. Net direction verdict

- **The 5 groups are the right spine and each is on-SOTA.** Groups A and E are essentially "surface an engine that already exists, the way the leaders surface it." Groups B/C/D are the genuine build, and the research pins down exactly how to do them right.
- **Add (in priority order):** goal-seeking/verification loop (#1 completeness), the durability corrections (persisted state machine + orphan-reclaim + needs-input), the sandbox hardening set (backpressure/ring-buffer, idle+absolute timeouts, terminal-state reaping, cgroup-kill), untrusted-output scanning of child summaries, a unified inbox/dashboard, event-driven triggers, then named agent defs / cumulative spawn budget / streaming child progress / live TODO checklist.
- **Backbone: A-substrate + B-facade + C-as-a-kind** (DBOS + Goose prior art), NOT a separate table, NOT standalone C.

## 8. Agent self-task-management tool (Group G) — dedicated pass (done AFTER Group G was first drafted)

Group G was initially wired from general `TodoWrite` knowledge *without* a dedicated
research pass (the human caught this). A focused primary-source pass corrected it:

- **Claude Code REPLACED `TodoWrite` with structured `Task` tools** (`TaskCreate`/`TaskUpdate`/`TaskGet`/`TaskList`) as of **CC v2.1.142 / SDK 0.3.142** (official: code.claude.com/docs/en/agent-sdk/todo-tracking). The Task model is **per-item create + patch-by-id + a first-class read-back**, adds **dependencies** (`addBlocks`/`addBlockedBy`), an **`owner`** field, hierarchy, and **disk persistence** (cross-session, survives compaction). Legacy `TodoWrite` (single `todos` array rewrite; `{content, activeForm, status}`) still reachable via `CLAUDE_CODE_ENABLE_TASKS=0`. → **Group G now models the current Task tools, not legacy TodoWrite.**
- **The re-injection is TWO mechanisms, not one** (this was my ITEM-35 error): (a) an **in-session, change-triggered** out-of-band `<system-reminder>` re-emitting the full list ("Your todo list has changed… DO NOT mention this to the user"), attached to the user turn — **not** before every LLM call; and (b) a **separate compaction-restoration** step that explicitly re-emits the list after summarizing. The enabler: the current Task tools keep the list in a **durable store the model re-reads**, so "fresh" + "survives compaction" both fall out. → ITEM-35 corrected: durable source of truth + out-of-band change-gated re-render + explicit CompactionExtension re-emit.
- **Behavioral rules are the substance** (must be verbatim in the tool description): "use VERY frequently / you may forget important tasks," **exactly one `in_progress`** + keep **≥1 in_progress until all done** (LangChain's anti-idle rule), **mark complete IMMEDIATELY, don't batch**, never complete on failure/partial, **use for 3+ steps / skip trivial**.
- **Render rule** (CC): show the `in_progress` item by its `active_form` ("Running tests"), everything else by `content` ("Run tests"). → ITEM-36.
- **Sub-agent semantics** (this was my ITEM-37 error): CC sub-agents each get their **OWN isolated** list; **parent and child never see each other's** — the parent gets only the child's **final summary**, never its todos; **there is NO automatic rollup**. Cross-agent coordination is opt-in via a **shared list-id + `owner`**, not an auto-merge. → ITEM-37 corrected: drop the bespoke rollup.
- **Cross-system:** Codex has `update_plan(explanation?, plan:[{step,status:pending|in_progress|done}])` — single `step` field, terminal `done`, at-most-one in_progress, whole-plan rewrite; kept STRICTLY separate from Codex "Plan Mode." LangChain `write_todos` = `{content,status}` (no `activeForm`), enforces one `write_todos`/turn, and **deliberately does NOT re-inject** the list each turn. OpenAI Agents SDK: no built-in planning primitive. LangGraph: planning is a graph pattern, not a tool.
- **Building it into the shared `AgentCore` loop is faithful** — CC/SDK ship it as a built-in harness tool every agent + sub-agent gets its own instance of; the `AgentExtension`/`CompactionExtension` seams are the right layer.

Sources: code.claude.com/docs/en/agent-sdk/todo-tracking (official, Task-tools migration) · code.claude.com/docs/en/sub-agents · openai/codex PR #24794 (`update_plan`) · LangChain `write_todos` middleware source · minusx "Decoding Claude Code" + decodeclaude compaction deep-dive (system-reminder / compaction restoration; community mirrors of the system prompt). Secondary/flagged: the `~/.claude/tasks/` disk path + `CLAUDE_CODE_TASK_LIST_ID` env var (aibuilderclub); exact `<system-reminder>` wordings (community system-prompt mirrors); Cursor's internal todo schema (unpublished).

## 9. Auto-approval / tool-approval reviewer (Group H) — dedicated pass + as-built reconciliation

Human question: "auto-approve tool use — CC uses a model to classify and only ask if it's not sure; how should we do this?" A SOTA pass + an as-built map of ziee's current approval stack:

**As-built (ziee today):** `is_trusted(server)→Auto` → `ApprovalPolicy.decide(trusted × ApprovalMode)→{Auto|Prompt|Review|Deny}` → (`Review`→`Reviewer`→`ModelRiskClassifier` one-word `Risk{Low|High|Critical}`→`map_risk` Low→Auto/High→Prompt/Critical→Deny, **fail-closed**) → `HumanGate` (chat: pending `tool_use_approvals` row + `mcpApprovalRequired` SSE, suspend; workflow: durable `elicit`; unattended: allow-listed→Auto else Deny). Config: `agent_admin_settings.{unattended_approval_policy, reviewer_enabled, reviewer_model_id, reviewer_policy, reviewer_risk_thresholds}`.

**SOTA (primary sources):**
- **Field converged on BOTH, rules-first, model-for-the-ambiguous-middle** — nobody is model-first. Claude: allow/deny rules → read-only/in-dir Auto → classifier only for the rest → sandbox underneath; protected-path + `rm -rf /` bypass rules sit ABOVE everything (a matching allow-rule does NOT grant a protected-path write). Codex: sandbox (technical boundary) + `approval_policy` presets + the `auto_review` "guardian" reviewer at escalation boundaries. Cursor: allowlist → sandbox → LLM classifier.
- **"Ask only if unsure":** only **Cursor's** classifier literally emits *ask-the-human* as a third outcome. **Codex** encodes uncertainty as a **`user_authorization ∈ {high,medium,low,unknown}`** dimension that gates the HIGH band (`high→allow` only if `authorization ≥ medium`, else deny) + "missing context → lean conservative" — but has **no ask outcome** (open issue #21975 requests human-fallback). **Claude** = fail-toward-block + "deny-and-continue" (feed the reason, agent finds a safer path), prompting a human only after **3 consecutive / 20 total** blocks; headless = abort. Codex's `guardian/policy_template.md` (fetched from `openai/codex`) is the only fully-published rubric: emit `{risk_level(low/med/high/critical), user_authorization, outcome, rationale}`; **post-denial explicit user re-approval has highest precedence.**
- **Categories, not a scalar:** both name **exfiltration-to-untrusted-destination (the #1), destructive/irreversible, credential/secret, persistence, protected-path** as explicit categories.
- **Injection resistance = the crown jewel:** the classifier must be un-steerable — Claude is **"reasoning-blind"** (strips tool RESULTS + assistant text; a separate server-side probe scans tool results first); Codex "treat all artifacts as untrusted evidence, ignore embedded instructions" (99.3% injection recall). Fail-closed on error/timeout + a circuit-breaker are universal.
- **Unattended:** everyone degrades an "ask" toward **deny/abort/wait, never allow** (Claude `dontAsk`=auto-deny; Codex reviewer-stands-in or `never`+sandbox; LangGraph waits indefinitely). ziee's scheduler allow-list matches this.
- **NOT the security boundary:** all three insist the classifier/allowlist is governance/UX; **the SANDBOX is the boundary** (Cursor's denylist was bypassed via base64/`&&`).

**Reconciliation → the gaps (become Group H, ITEM-38–46):** (1) `reviewer_risk_thresholds` is **dead config** — stored/validated but never consumed (`map_risk` hardcoded) → ITEM-38. (2) No confidence/authorization dimension, no "unsure→ask" (only 3 hard bands) → ITEM-39. (3) No rule-layer above the model for protected-path/destructive/exfiltration → ITEM-40. (4) Risk is a scalar with no category/rationale → ITEM-41. (5) Classifier injection-resistance not explicit (guard prompt + result-strip + probe + red-team test) → ITEM-42. (6) Unattended fail-closed not specified/tested → ITEM-43. (7) No per-tool `requires_user_interaction` flag; `ApprovedForSession` not durable + not exact-scope → ITEM-44. (8) `Granular`/`on-failure`/`SandboxMode` are stubs; two parallel chat classifiers (drift) → dedupe ITEM-46. ziee is AHEAD of Codex on the ask-outcome + durable gate, and can add Claude's optional deny-and-continue → ITEM-45.

Sources: code.claude.com/docs/en/permission-modes + anthropic.com/engineering/claude-code-auto-mode (two-stage classifier, deny-and-continue, reasoning-blind) · learn.chatgpt.com/docs/sandboxing + /agent-approvals-security + /sandboxing/auto-review + `openai/codex` `guardian/policy_template.md` (rubric) + issue #21975 · cursor.com/docs/agent/security/run-modes · developers.openai.com Responses MCP `require_approval` + Agents SDK guardrails/`needs_approval` · LangGraph interrupts. Flagged secondary: Claude 17%FN/0.4%FP (self-reported), Cursor denylist-bypass (The Register/Backslash).

## 10. External MCP servers — auto-approval with NO enforcement boundary (Group H cont.)

Human question: "for external MCP it's harder — we can custom our sandbox, but not that." Correct, and it's the security crux. A dedicated primary-source pass (MCP spec + threat research + host UX):

- **MCP spec (2025-06-18) is explicit:** "Tools represent arbitrary code execution… **Hosts MUST obtain explicit user consent before invoking any tool**," and **"descriptions of tool behavior such as annotations should be considered untrusted, unless obtained from a trusted server."** The official annotations blog: hints "aren't enforcement… keep your actual safety guarantees in deterministic controls (network/sandbox), not a boolean hint"; a tool with no annotations is assumed **non-read-only, destructive, non-idempotent, open-world**. → **`readOnlyHint` can NEVER be the sufficient condition to auto-approve an external tool.**
- **Threats (primary):** tool-poisoning (malicious instructions in the *description* — Invariant PoC exfiltrated an SSH key **through tool arguments**); ATPA (injection in tool *results* — CyberArk); **rug-pull** = approve-benign-then-swap, realized as **CVE-2025-54136 "MCPoison"** in Cursor (fix: any change → re-approve); cross-server tool shadowing; result-injection (MS/GitHub MCP private-repo exfil). Simon Willison's **lethal trifecta** (private data + untrusted content + external comms) — an external MCP call supplies **all three legs at once**.
- **How hosts gate 3rd-party MCP:** all converge on **config-time human server-trust (TOFU) as the root**, with call-time relaxation opt-in on top: Claude Code (`.mcp.json` "trust this server?" pending-approval; `allow/ask/deny` rules, deny>allow; `anthropic/requiresUserInteraction` forces per-call ask even in bypass); VS Code/Copilot (server trust dialog + per-tool approve — but the **workspace-trust `autoApprove` bypass** shows config-file-driven trust is dangerous); Cursor (server-enable → per-tool allowlist → sandbox → classifier; CVE fixed by re-approve-on-change); OpenAI Responses (`require_approval` default **`"always"`**, per-tool `never` only "once you trust the server"). **Nobody reliably auto-classifies external read-vs-write without trusting the hint.**
- **Generalizable models:** TOFU + per-server tiers; **schema/description hash pinning** to detect rug-pulls (Invariant + CVE fix); **ETDI** (arXiv 2506.01333 — signed/versioned tool defs, re-approve on any change) as the north star; **CaMeL/dual-LLM** (arXiv 2503.18813 — capability-tagged data flow keeps untrusted results out of control-flow) as the structural fix for result-injection; MCP Registry = provenance only (does NOT scan code).

**Decision model for ziee (no boundary):** config-time HUMAN server-trust is the authorization ROOT; per-`(server,tool)` human allowlist; the classifier is **veto-only** (can tighten, never grant); the tool's metadata is inert for security; external tools get a **stricter default** than built-in/sandboxed (which DO have a boundary). → ITEM-47–53. ziee is already ahead (never auto-trusts external; per-`(server,tool)` allowlist; SSRF `url_validator`; `env_clear`+secret-denylist) — the gaps are hash-pinning (biggest), full-disclosure prompt + destination-host naming, veto-only invariant, result-cannot-escalate audit, data-egress interlock, hints-tighten-only.

Sources: modelcontextprotocol.io/specification/2025-06-18 + /docs/tutorials/security/security_best_practices + blog.modelcontextprotocol.io "Tool Annotations as Risk Vocabulary" + /registry/about · Invariant Labs tool-poisoning · CyberArk ATPA · Check Point CVE-2025-54136 · Willison lethal-trifecta + CaMeL · arXiv 2503.18813 (CaMeL) + 2506.01333 (ETDI) · code.claude.com/docs/en/mcp · code.visualstudio.com/docs/agents/{approvals,security} + Repello workspace-trust bypass · cursor.com/docs/mcp · developers.openai.com Responses MCP `require_approval`. Flagged secondary: Cursor/Codex internals (host writeups); vendor defaults evolve — re-check live.

## Confidence / caveats
- Highest confidence (full primary-doc fetches, quoted): Claude Code sub-agents / agent-view / scheduled-tasks / goal / routines / agent-teams; Claude Agent SDK; Codex `[agents]` + cloud + automations; OpenAI Responses background mode + webhooks; Managed Agents; DBOS / LangGraph / Mastra / Cloudflare / Goose seam shapes.
- Lower confidence (own-search / secondary, flagged in-thread): OpenAI Agents SDK / CrewAI / AutoGen / LangGraph exact tool signatures; Cursor "8 concurrent"; Temporal/Restate/Inngest fine details (rows marked "report pending" — they reinforce, not change, the verdict); some Codex/Claude internal constants are community/deepwiki, not official.
- One agent mis-stated ziee's scheduler path as `modules/jobs/scheduler.rs`; the real path is **`src-app/server/src/modules/scheduler/`** (verified in the codebase sweep).
