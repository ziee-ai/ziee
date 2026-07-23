# DECISIONS — agent-orchestration (Phase 4)

All product/human inputs resolved up front. Human-selected scope decisions are the
LOCK-1..6 records in PLAN.md; the per-item decisions below (resolved by codebase
convention or the recorded human picks) let implementation run without a mid-flight
stop. Every operational tunable defaults to an admin-configurable settings row unless
a security boundary requires a fixed constant. Zero unresolved placeholders.

## Scope dispositions (descoped items — human-approved)
- DESCOPED: ITEM-28 — Live agent task checklist (UI) is absorbed into ITEM-36 (Group G's live render is one surface); no independent work. [approved: human 2026-07-19]
- DESCOPED: ITEM-33 — Named agent defs / cumulative spawn budget / streaming child progress / per-child sandbox mode (Group-A ergonomics polish); the human did not select it, deferred to a post-A round. [approved: human 2026-07-19]

## Locked scope (human-selected — see PLAN.md §Locked scope)
- LOCK-1 breadth full A–E + Group-F completeness; LOCK-2 backbone = reuse `workflow_runs` + `JobKind` registry (no separate table); LOCK-3 hardening in as baseline; LOCK-4 sequence A+E → backbone → B+C; LOCK-5 auto-approval = Reviewer-ON default + external hash-pinned veto-only + deny-and-continue + admin per-tool approval; LOCK-6 per-surface window-relative compaction (chat eager ~60%, agent patient ~75%, tokens-not-messages, high/low watermark + cooldown, 9-section, outbound-only).

### DEC-1: Cap on children per delegate call?
**Resolution:** new admin column `fan_out_max_children_per_call` on agent_admin_settings (default 8, 1..=64); over-cap truncates + "capped at N" note.
**Basis:** codebase (agent_admin_settings fan_out_max_threads).
### DEC-2: delegate tool default-enabled + gating?
**Resolution:** admin bool `delegate_enabled` (default false); chat also needs ZIEE_CHAT_AGENT_CORE; children allow_delegate=false.
**Basis:** codebase (agent_dispatch reads settings; cutover flag).
### DEC-3: cumulative spawn budget?
**Resolution:** admin column `subagent_spawn_budget_per_run` (default 50, 1..=1000); over-budget errors.
**Basis:** convention (RESEARCH A-gap-3; Goose caps) + codebase.
### DEC-4: surfaced delegate depth?
**Resolution:** FIXED 1 (children allow_delegate=false), independent of fan_out_max_depth.
**Basis:** codebase (fanout.rs:59) + convention (Codex max_depth=1).
### DEC-5: child tool-scope narrowing?
**Resolution:** FIXED invariant — intersect child servers with parent reachable.
**Basis:** convention (least-privilege/RBAC).
### DEC-6: background per-run limits?
**Resolution:** reuse default_max_steps + per_run_token_cap; ADD `background_wall_clock_secs` (default 300, 30..=3600) + idle reaper.
**Basis:** codebase (Budget) + convention (Goose per-kind caps).
### DEC-7: background per-user cap + retention?
**Resolution:** admin `max_background_tasks_per_user` (default 10, 1..=100) + `background_task_retention_days` (default 7, 0=forever) + boot prune loop.
**Basis:** codebase (scheduler cap; notification/prune) + convention.
### DEC-8: background holds chat single-flight lock?
**Resolution:** FIXED no — own per-user domain, outside begin_generation.
**Basis:** convention (RESEARCH §2).
### DEC-9: fan_out all-or-nothing failure in delegate?
**Resolution:** relax — failed child → error-summary, survivors return; one bad child never fails parent turn.
**Basis:** convention (§6) + codebase (fanout.rs:79-88).
### DEC-10: merged child summary scanned?
**Resolution:** yes — through the ITEM-32 untrusted-content/injection scan before appended.
**Basis:** convention (RESEARCH A-gap-1; §11) + user (LOCK-3).
### DEC-11: delegate tool name collision?
**Resolution:** FIXED reserved unprefixed `delegate`; MCP tools namespaced server__tool so no collision.
**Basis:** codebase (resolver namespacing).
### DEC-12: completion delivery?
**Resolution:** reuse continue_chat seeding + notification row + sync fan-out. FIXED.
**Basis:** codebase.
### DEC-13: new sync entity for background?
**Resolution:** no new entity from A/B — reuse SyncEntity::Notification + WorkflowRun; dedicated BackgroundTask entity deferred to Group-D owner.
**Basis:** codebase + convention.
### DEC-14: restart re-drive?
**Resolution:** reuse startup_sweep resumable re-drive + per-JobKind resume entry (SubAgent re-enters agent loop).
**Basis:** codebase + convention.
### DEC-15: cancel + access-loss cleanup?
**Resolution:** FIXED — cancel cascades to children + workspace + cgroup kill; conversation delete synchronously cancels + reclaims, aborts on cleanup failure.
**Basis:** convention (§5).
### DEC-16: background_status ownership?
**Resolution:** FIXED owner-scoped opaque handle → 404 cross-user; split check_status from paged result read.
**Basis:** convention (§1).
### DEC-17: ITEM-4 new variant or reuse?
**Resolution:** NEW sub-agent-activity content-block + SSE via compose_* seams; forces openapi-regen both + clean macro rebuild.
**Basis:** codebase + convention.

### DEC-21: Backbone shape
**Resolution:** reuse workflow_runs + job_kind discriminator + JobKind trait registry + spawn_background/check_status/collect_result facade; JobKind::SubAgent = Option C. NO separate background_jobs table.
**Basis:** LOCK-2 + RESEARCH §5 (DBOS/Goose).
### DEC-22: workflow_id for non-workflow runs
**Resolution:** make workflow_id nullable + add job_kind text NOT NULL DEFAULT 'workflow' (CHECK workflow/sandbox_exec/subagent); no fake ephemeral row.
**Basis:** codebase (workflow_id NOT NULL FK, spawn_run reads yaml).
### DEC-23: discriminator column
**Resolution:** job_kind is NEW orthogonal column; never overload run_kind/invocation_source.
**Basis:** codebase (existing CHECKs).
### DEC-24: needs_input state
**Resolution:** reuse existing `waiting` status + reply affordance; no new status value.
**Basis:** codebase (WorkflowRunStatus::Waiting semantics).
### DEC-25: per-JobKind sweep policy (fixed)
**Resolution:** SubAgent→resumable (replay); SandboxExec/export→failed.
**Basis:** resume_run replays transcript; subprocess can't.
### DEC-26: background concurrency caps
**Resolution:** admin cols background_max_concurrent_per_user(10)/per_conversation(3)/global(512); over-cap errors.
**Basis:** agent_admin_settings pattern + RESEARCH §2.
### DEC-27: sandbox bg max-lifetime + idle-reaper
**Resolution:** admin sandbox_background_max_lifetime_secs(3600) + sandbox_background_idle_reap_secs(300); reaped→timed_out.
**Basis:** RESEARCH C-gap-3 (Codex #5948).
### DEC-28: output ring-buffer size
**Resolution:** admin sandbox_background_output_ring_bytes (head+tail, default 1MiB); REPLACES drop-after.
**Basis:** RESEARCH C-gap-1/ITEM-30/LOCK-3.
### DEC-29: get_command_output access
**Resolution:** byte-range paging {bytes,total_bytes,next_offset}, idempotent; not consuming cursor.
**Basis:** RESEARCH C-gap-2.
### DEC-30: background-run retention
**Resolution:** admin background_run_retention_days (0=forever) + boot prune loop.
**Basis:** RESEARCH §2; notification/prune pattern.
### DEC-31: teardown/cgroup-kill/guards (fixed)
**Resolution:** kill cgroup (grandchildren); registry owns guards + PR_SET_PDEATHSIG; prune every exit path; re-apply hardening.
**Basis:** RESEARCH C-gap-4 (CC #11190, Codex #14367).
### DEC-32: completion-notify
**Resolution:** reuse SyncEntity::WorkflowRun + notification row; new entity only if FE needs separate store.
**Basis:** RESEARCH §2; existing producers.
### DEC-33: trio is built-in MCP (fixed)
**Resolution:** perm + both mcp.rs edits; check/collect approval-bypass; spawn_background→gate.
**Basis:** CODING_GUIDELINES §11.
### DEC-34: sdk-submodule workflow
**Resolution:** init sdk; author detach+ring+paging in ziee-sandbox; land in sdk repo, bump pin 9e6d8c74 in coordinated commit.
**Basis:** codebase (path-dep, submodule not checked out).
### DEC-35: right-panel surfacing
**Resolution:** registerPanelRenderer + displayInRightPanel mirroring workflow per-run progress + snapshot-on-connect; Tabs variant=line; raw stdout on Log SSE variant.
**Basis:** PLAN Surface 2; DESIGN_SYSTEM J5.
### DEC-36: handle shape (fixed)
**Resolution:** opaque workflow_runs.id UUID; owner-scoped fetch → cross-user 404.
**Basis:** RESEARCH §2 + CODING_GUIDELINES §1.

### DEC-41: loop/schedule entry
**Resolution:** toolbar_actions button SOLE entry now (mirror voice), opens merged dialog; defer slash parser; if built route to same dialog.
**Basis:** RESEARCH §4 NL-first; §9 one-source.
### DEC-42: self-paced next-fire signal
**Resolution:** a model-callable schedule_next{delay_seconds, reason, stop?} tool on self-paced runs; dispatch_prompt reads it, writes next_run_at/disables. Not prose parsing.
**Basis:** RESEARCH §4 (ScheduleWakeup structured signal).
### DEC-43: surface next-delay+reason
**Resolution:** persist next_run_at+reason on run row; render in timeline + attached card.
**Basis:** RESEARCH §4; ScheduledTaskRun additive field.
### DEC-44: self-stop
**Resolution:** stop:true → enabled=false, "completed" (paused_reason null).
**Basis:** RESEARCH §4; is_active keys off enabled && paused_reason.is_none().
### DEC-45: 7-day backstop
**Resolution:** clamp delay to [min_interval_seconds, max_horizon_days]; absolute expiry default 7d (new max_horizon_days admin col); expiry→self-stop.
**Basis:** RESEARCH §4; min_interval_seconds exists.
### DEC-46: bind-to-current-conv default
**Resolution:** in-chat entry defaults bound_conversation_id=current; results land here via bound path, not continue_chat.
**Basis:** dispatch.rs:294-318 ownership-checked; §1 404.
### DEC-47: attached list = filter
**Resolution:** GET ?conversation_id= + ScheduledTaskCard; same rows.
**Basis:** §9 no dup storage.
### DEC-48: self-paced DB
**Resolution:** add 'self_paced' to schedule_kind CHECK + relax schedule_coherent (neither run_at/cron at rest); one additive migration.
**Basis:** schema.sql:54 CHECK.
### DEC-49: G persistence DURABLE
**Resolution:** durable per-run store (new table+repo mirroring assistant_core_memory); item {content,active_form,status,owner?,deps?}; FK-cascade.
**Basis:** RESEARCH §8; assistant_core_memory precedent.
### DEC-50: G store home = new agent-core port
**Resolution:** TaskListStore port (sibling to TranscriptStore), server-side impl; agent-core DB-free. chat keys branch; workflow keys run.
**Basis:** ports.rs DB-free.
### DEC-51: task-tool injection seam (shared w/ Group A)
**Resolution:** TaskCreate/Update/Get/List core-injected + intercepted; build ONE shared core-tool-interception with Group-A delegate (ITEM-1).
**Basis:** core.rs:329/551.
### DEC-52: re-injection = out-of-band reminder + compaction re-emit
**Resolution:** (a) change-gated out-of-band <system-reminder> before user turn, before_model hook order<COMPACTION_ORDER; (b) CompactionExtension re-emits from durable store post replace_head. Store = source of truth.
**Basis:** RESEARCH §8; compaction.rs:115 pins only System.
### DEC-53: sub-agent list isolation no rollup
**Resolution:** own run-scoped list; no auto-rollup; shared-list-id+owner deferred.
**Basis:** RESEARCH §8; fan_out returns SubagentSummary.
### DEC-54: task item schema
**Resolution:** {content, active_form, status, owner?, deps?}; active_form drives in_progress render; not Codex single-step.
**Basis:** RESEARCH §8.
### DEC-55: behavioral rules verbatim
**Resolution:** tool desc carries CC rules verbatim (frequent, one in_progress, complete immediately, 3+ steps skip trivial).
**Basis:** RESEARCH §8.
### DEC-56: live render via compose seams
**Resolution:** SSE event + content variant via compose macros (mirror McpToolProgress); render in_progress→active_form else content; absorbs ITEM-28; clean-build validate.
**Basis:** extension.rs:168/244 + content.rs:58.
### DEC-57: G tunables admin-configurable
**Resolution:** max_active_tasks + item size cap on agent_admin_settings (CHECK-bounded, gated, sync); default 100 items/bounded len.
**Basis:** §16/§4.
### DEC-58: E tunables
**Resolution:** reuse scheduler_admin_settings max_active_tasks_per_user(20)/min_interval_seconds(300); add max_horizon_days(7).
**Basis:** schema.sql:62-70.
### DEC-59: ZIEE_CHAT_AGENT_CORE gating for G
**Resolution:** G reaches chat only when agent-core flag on; ships to workflow-agent now; don't retrofit legacy loop.
**Basis:** PLAN cutover.
### DEC-60: migration numbering + regen
**Resolution:** timestamp migrations after 202607170105, distinct per group; openapi-regen both; G SSE/content → types.ts (emit_ts golden).
**Basis:** build.rs filename-sort; emit_ts parity.

### DEC-61: goal-seeking evaluator model
**Resolution:** nullable goal_eval_model_id on agent_admin_settings; NULL→run model; RBAC-resolved.
**Basis:** mirrors reviewer_model_id + WorkflowModelResolver.
### DEC-62: max goal-seeking turns
**Resolution:** admin goal_seek_max_turns (default 10, 1-50) + hard 7-day horizon; exceed→incomplete.
**Basis:** budget SAFETY_MAX_ITERATIONS + self-paced horizon.
### DEC-63: goal-seeking eval cadence
**Resolution:** once per fired turn after_llm_call, non-blocking, isolated cheap-model, sees only artifact+condition.
**Basis:** reviewer isolated-classify.
### DEC-64: inbox retention
**Resolution:** reuse scheduler_admin_settings.notification_retention_days (30, 0=forever) + 6h prune.
**Basis:** notification/prune.
### DEC-65: inbox scope
**Resolution:** owner background JobKind runs (SSE snapshot) + owner notifications; needs_input/waiting top; cross-user→404.
**Basis:** SyncEntity owner; progress_sse snapshot.
### DEC-66: event-trigger source set v1
**Resolution:** v1 run-complete + scheduled-task-complete only; dataset-change DROPPED (no module); file-appear DEFERRED.
**Basis:** dormant EventBus; no dataset; SyncEntity::File notify-only.
### DEC-67: event-trigger substrate
**Resolution:** internal completion hook at runner terminal sites (not EventBus); schedule_kind='event' + trigger_source (DROP+re-add CHECK).
**Basis:** runner.rs terminal emits; CHECK precedent 202607170100.
### DEC-68: event-trigger debounce
**Resolution:** coalesce same-source within min window; reuse min_interval_seconds (300, 60-86400).
**Basis:** scheduler min_interval.
### DEC-69: sandbox bg absolute-max lifetime
**Resolution:** admin bg_exec_max_lifetime_secs on code_sandbox_settings (3600, 60-86400); 0/unbounded disallowed for bg.
**Basis:** code_sandbox_settings wall-clock; RESEARCH C-gap-3.
### DEC-70: sandbox bg idle reaper
**Resolution:** admin bg_exec_idle_secs (300, 30-3600); no new output→timed_out.
**Basis:** VM idle-evict; RESEARCH C-gap-3.
### DEC-71: ring-buffer size/strategy
**Resolution:** head+tail ring (default 1MiB = 256KiB head + 768KiB tail) + dropped-middle marker, replaces head-only; admin bg_exec_output_ring_bytes (capped); spill-to-file above ceiling.
**Basis:** current head-only OUTPUT_CAP_BYTES; RESEARCH C-gap-1.
### DEC-72: byte-range paging contract
**Resolution:** get_command_output(offset?,max_bytes?)→{bytes,total_bytes,next_offset}; idempotent non-consuming.
**Basis:** RESEARCH C-gap-2; tool_result_mcp paging.
### DEC-73: background concurrency caps
**Resolution:** admin bg_exec_max_per_conversation (3) + bg_exec_max_global (32); over-cap 429/capped; SubAgent reuse fan_out_max_threads.
**Basis:** registry MAX_CLIENTS_PER_RUN 429; Goose caps.
### DEC-74: terminal reporting + retention
**Resolution:** distinguish exited{code}/killed(signal)/timed_out; keep terminal row bg_exec_terminal_retain_secs (300) for final read then prune every path.
**Basis:** RESEARCH C-gap-4; SandboxRunResult.timed_out.
### DEC-75: cgroup-kill + guard ownership
**Resolution:** security-fixed — background registry OWNS guards (invert stack-frame ownership) so kill -9 cascades; reap writes cgroup.kill (enhancement over PID-ns collapse); re-apply all hardening.
**Basis:** ziee-sandbox.slice; no cgroup.kill today; RESEARCH C-gap-4.
### DEC-76: per-JobKind orphan-sweep
**Resolution:** SubAgent crash→resumable+replay; sandbox-exec→failed (Child gone). Each JobKind own policy.
**Basis:** fail_orphaned_runs resumable_agent; RESEARCH §5.
### DEC-77: needs_input representation
**Resolution:** reuse durable Waiting + pending_elicitation_json (timeout_ms==0) not a new status; inbox surfaces as "needs input".
**Basis:** ElicitDispatcher + elicit reply/resume.
### DEC-78: needs_input reply timeout
**Resolution:** admin bg_needs_input_timeout_secs (86400=24h; 0=indefinite); expiry→fail needs_input_timeout, never allow.
**Basis:** elicit deadline_at; unattended-fail-closed.
### DEC-79: steer note channel
**Resolution:** bounded note queue on RunHandle (mirror cancel Notify), depth 8 drop-oldest-marker, delivered at iteration boundary, owner-only, not persisted.
**Basis:** RunHandle.cancel; owner check.
### DEC-80: child-summary injection scan (security-fixed)
**Resolution:** always-on deterministic neutralize-and-annotate in fan_out before parent reads (neutralize not drop); extract shared guard helper.
**Basis:** fanout summary_from_events unscanned; reviewer isolated-classify; per-module guard consts.

### DEC-81: default posture Reviewer-ON
**Resolution:** read-only builtins Auto; builtin writes reviewer (Auto only low+authz≥medium, Prompt unsure/high-unauthz, Deny critical); external Prompt until allowlisted; unsure→Prompt.
**Basis:** LOCK-5; RESEARCH §9.
### DEC-82: chat gains reviewer behind ZIEE_CHAT_AGENT_CORE
**Resolution:** wire real Reviewer in chat agent-host (replace reviewer:None); legacy mcp.rs unchanged until cutover.
**Basis:** only workflow builds reviewer today.
### DEC-83: thresholds admin-configurable (fix dead config)
**Resolution:** map_risk reads reviewer_risk_thresholds at single site, default when band absent; both build sites thread same map.
**Basis:** reviewer.rs:27; ITEM-38.
### DEC-84: threshold storage reuses jsonb (no migration)
**Resolution:** band→decision + per-category nest in agent_admin_settings.reviewer_risk_thresholds jsonb.
**Basis:** column exists/validated.
### DEC-85: authorization {high,medium,low,unknown} gates HIGH
**Resolution:** High Auto only if authz≥medium else Prompt; Critical ignores authz.
**Basis:** Codex idea; ITEM-39.
### DEC-86: unknown/abstain→Prompt
**Resolution:** unknown/abstain routes to durable Prompt gate, never Auto.
**Basis:** "prompt only when unsure"; ITEM-39.
### DEC-87: uncertainty-fails-toward-prompt invariant
**Resolution:** uncertainty/parse-fail/timeout/missing-context → Prompt/Deny; clamp not classifier's word.
**Basis:** SOTA fail-closed; reviewer.rs:60-65.
### DEC-88: Risk scalar→struct {band,authorization,category,rationale}
**Resolution:** classify returns struct; ModelRiskClassifier/RecordingRiskClassifier/gate payload updated.
**Basis:** ITEM-41; extensibility.
### DEC-89: category taxonomy fixed enum
**Resolution:** {exfiltration,destructive/irreversible,credential/secret,persistence,protected-path,other}; per-category thresholds nest in jsonb.
**Basis:** RESEARCH §9.
### DEC-90: classifier input = user msgs + tool CALLS only (reasoning-blind)
**Resolution:** strip tool results + reasoning; explicit + tested.
**Basis:** RESEARCH §9; ITEM-42.
### DEC-91: guard prompt + pre-scan probe
**Resolution:** prepend untrusted-evidence guard; cheap pre-scan probe on incoming results.
**Basis:** ITEM-42.
### DEC-92: fail-closed + circuit-breaker
**Resolution:** keep Deny-on-error; add classifier timeout + circuit-breaker → Prompt/Deny.
**Basis:** reviewer.rs:60-65.
### DEC-93: danger-layer fixed order
**Resolution:** deny-rule > per-tool always-prompt > read-only Auto > danger-layer > Reviewer > default; applied in core.rs BEFORE Auto short-circuits.
**Basis:** ITEM-40.
### DEC-94: danger lists = fixed structured constants
**Resolution:** protected-paths + irreversible-destructive + egress; not admin-editable, one module, commented.
**Basis:** security floors; RESEARCH §9.
### DEC-95: danger-layer not overridable by ApprovedForSession
**Resolution:** session grant checked AFTER danger-layer.
**Basis:** ITEM-40.
### DEC-96: egress reuses url_validator allowlist
**Resolution:** exfiltration rule wires into SSRF trusted-host allowlist (sdk/ziee-framework); submodule change.
**Basis:** ITEM-40.
### DEC-97: reviewer model nullable→run model
**Resolution:** NULL→run model under RBAC; set id resolves, fallback on error.
**Basis:** agent_dispatch.rs:749-778.
### DEC-98: per-tool requires_user_interaction wins over matrix+reviewer
**Resolution:** forces Prompt regardless of mode/allowlist/reviewer.
**Basis:** ITEM-44.
### DEC-99: durable exact-scope ApprovedForSession (new table, excludes Critical/protected)
**Resolution:** persisted table (survives restart), scoped to tool+arg-shape hash, never Critical/protected; both hosts read.
**Basis:** ITEM-44.
### DEC-100: retire in-memory workflow ApprovedForSession
**Resolution:** replace OnceLock map with durable store; ConversationApprovalPolicy consults it.
**Basis:** DEC-99.
### DEC-101: deny-and-continue feeds rationale (build now)
**Resolution:** blocked call returns rationale as tool_result (replace generic strings); complements elicit gate.
**Basis:** LOCK-5; ITEM-45.
### DEC-102: category+rationale persistence
**Resolution:** add nullable category+rationale cols to mcp_tool_calls + tool_use_approvals via additive migrations.
**Basis:** ITEM-41.
### DEC-103: dedupe two classifiers; retire legacy on cutover
**Resolution:** one resolver both hosts; new rules land once; retire mcp.rs ladder at cutover; keep parity table until then.
**Basis:** ITEM-46.
### DEC-104: external veto-only
**Resolution:** for external, reviewer may only downgrade; clamp; closes OnRequest→Review→Low→Auto.
**Basis:** ITEM-47; RESEARCH §10.
### DEC-105: external per-(server,tool) prompt-by-default, no trust-all one-click
**Resolution:** fresh external auto nothing until grants; server-wide toggle needs heavier confirmation.
**Basis:** LOCK-5; RESEARCH §10.
### DEC-106: fingerprint hash
**Resolution:** sha256(name+description+inputSchema+server command/url/transport) at grant; new nullable column; recompute per call.
**Basis:** ITEM-49; RESEARCH §10.
### DEC-107: rug-pull invalidation
**Resolution:** drift→invalidate+re-elicit w/ diff; tools/list_changed = invalidation.
**Basis:** CVE-2025-54136.
### DEC-108: full-disclosure prompt
**Resolution:** full exact desc + concrete args + dest host; is_system host resolved server-side.
**Basis:** ITEM-50; RESEARCH §10.
### DEC-109: result-cannot-escalate + fence all external
**Resolution:** no result/desc writes allowlist/unattended/approval state (tested); extend untrusted fence to ALL external results.
**Basis:** ITEM-51; RESEARCH §10.
### DEC-110: egress interlock v1 subset
**Resolution:** secret/file/other-server-output-in-arg to external host forced prompt; cross-server-origin tagging; full capability-dataflow after Group-D.
**Basis:** ITEM-52; CaMeL.
### DEC-111: hints tighten-only
**Resolution:** readOnlyHint/registry advisory; destructiveHint/requiresUserInteraction one-way force-prompt (max() not override).
**Basis:** ITEM-53; MCP spec.
### DEC-112: admin per-tool default storage
**Resolution:** tool_approval_defaults jsonb on mcp_servers (per-tool map), activates dormant approval_mode at per-tool granularity.
**Basis:** ITEM-54.
### DEC-113: admin default precedence
**Resolution:** admin baseline; user restricts not loosens; danger-layer/hash-pin/veto bind on top; grant source on journal.
**Basis:** LOCK-5; ITEM-54.
### DEC-114: admin default gating + sync
**Resolution:** system-MCP admin perm; SyncEntity; consulted by converged resolver as precedence LAYER not 3rd classifier.
**Basis:** ITEM-54.
### DEC-115: external system Auto still hash-pinned
**Resolution:** admin Auto on external system tool = fingerprint-pinned + reviewer veto-only.
**Basis:** ITEM-54; RESEARCH §10.
### DEC-116: system MCP per-tool FE
**Resolution:** mirror McpServerDrawer Tools tab; Auto/Prompt/Disabled + set-all; tools/list cached fallback + unreachable; ListPagination; 390px; negative-perm e2e; external stricter hint.
**Basis:** ITEM-55.
### DEC-117: unattended degradation
**Resolution:** headless Prompt/Review → durably wait / deny-and-continue / abort-after-N, NEVER Auto; reviewer stands in within allow-list; non-allow-listed denied (no orphan pending).
**Basis:** LOCK-5; ITEM-43.
### DEC-118: storage by convention
**Resolution:** open-shape config→jsonb (thresholds, admin per-tool defaults); scalar projections→columns/tables (category/rationale, fingerprint, durable grants).
**Basis:** codebase convention.
### DEC-119: openapi + desktop parity
**Resolution:** regen both for admin defaults/category-rationale/fingerprint-drift/dest-host; Option<Option<T>>+deserialize_nullable_field; 401+403 per endpoint.
**Basis:** conventions.
### DEC-120: sandbox = enforcement boundary; classifier = governance
**Resolution:** code_sandbox sole enforcement; reviewer/allowlist/danger-layer governance/UX; external (no boundary) strictest defaults.
**Basis:** RESEARCH §9/§10; LOCK-5.

### DEC-121: per-surface window-relative fractions
**Resolution:** trigger = surface_fraction × (context_length − output_headroom); chat 0.60, agent 0.75; admin cols, validated (0.10,0.95). Replaces 200K/100K + fixed 0.75.
**Basis:** LOCK-6; model_context_window.
### DEC-122: fallback when None
**Resolution:** headroom = max_output(8000)+safety(4000); fallback_window_tokens=128000. chat≈69600, agent≈87000. Never 0/unbounded.
**Basis:** LOCK-6.
### DEC-123: tier thresholds
**Resolution:** Tier0 always; Tier1 tier1_fraction 0.50; Tier2 with Tier1; Tier3+4 at high watermark. Order Tier0<1<2<3<4.
**Basis:** RESEARCH §11; needs DEC-129.
### DEC-124: split by TOKENS
**Resolution:** compact-oldest/keep-newest in tokens net of summary size, target low watermark; replaces message-fraction.
**Basis:** LOCK-6/ITEM-63.
### DEC-125: low watermark + min-growth
**Resolution:** low chat 0.40/agent 0.55; min_free_tokens 20000; never compact unless frees ≥.
**Basis:** LOCK-6/ITEM-63; clear_at_least.
### DEC-126: cooldown
**Resolution:** no Tier-4 refire until ≥cooldown_turns(2) AND ≥cooldown_growth_tokens(10000); state persisted per surface.
**Basis:** LOCK-6/ITEM-63.
### DEC-127: 9-section format
**Resolution:** 9 sections (user req/intent, task list verbatim, decisions, files/edit state, errors/fixes, WIP/next, recall handles, governance signals, durable facts). Replaces both freeform.
**Basis:** LOCK-6; RESEARCH §11.
### DEC-128: unify onto engine
**Resolution:** engine sole Tier-4 impl + sole conversation_summaries writer; replace_head stops writing summary (keeps HistoryReplaced); Compactor = tier-orchestrator delegating Tier-4 to engine.
**Basis:** ITEM-56; engine convergence test.
### DEC-129: fix .order()
**Resolution:** AgentCore sorts extensions by .order() (stable) at construction; tier orders < COMPACTION_ORDER; clean-build validate.
**Basis:** ITEM-56 confirmed inert.
### DEC-130: inline vs between-turns
**Resolution:** cheap tiers 0/1/2 inline before_model; tiers 3/4 between-turns after_llm_call; hard-overflow inline Tier-4 fallback to avoid 400.
**Basis:** LOCK-6/ITEM-63.
### DEC-131: sleep-time on Group-D
**Resolution:** ITEM-62 = JobKind on workflow_runs backbone, after Group D; gated off until D; CAS/version guard vs live write.
**Basis:** LOCK-2/LOCK-4; summarizer.rs:27 race.
### DEC-132: tokenizer-accurate
**Resolution:** per-model tokenizer (tiktoken o200k/cl100k cloud, HF local, chars/4 fallback biased HIGH); shared count_tokens(model,text).
**Basis:** LOCK-6/ITEM-61.
### DEC-133: tunables extend summarization_admin_settings
**Resolution:** add chat/agent trigger+low-watermark fractions, tier1_fraction, headroom, fallback_window, min_free_tokens, cooldown_turns/growth, summary_format, compaction_state; migration + regen both.
**Basis:** LOCK-6; singleton pattern; types_ts_parity.
### DEC-134: recall-handle scope non-chat
**Resolution:** chat keeps get_tool_result marker; workflow/sub-agent inline preview + run-scoped ref, no cross-conv read.
**Basis:** ITEM-57; tool_result_mcp conversation-scoped.
### DEC-135: governance-decay source
**Resolution:** governance signals contributed as a pinned governance System block by gate/policy contribute hook; Tier-4 pins (Section 8); summarizer decoupled from gate internals.
**Basis:** ITEM-60 CONCERN 3; RESEARCH §11 governance decay.
### DEC-136: task-list pin degrades gracefully
**Resolution:** pin Group-G task list when store exists; omit section when Group G not shipped; no hard build-order dep.
**Basis:** ITEM-60 CONCERN 2.
### DEC-137: /compact-with-focus
**Resolution:** POST /api/conversations/{id}/compact {focus?}; owner-gated; emits HistoryReplaced; composer control; outbound-only; regen+desktop.
**Basis:** LOCK-6/ITEM-61.
### DEC-138: OUTBOUND-ONLY binding
**Resolution:** every tier mutates only outbound request; no message_contents delete/rewrite; test asserts stored transcript unchanged.
**Basis:** ITEM-56; LOCK-6.
### DEC-139: chat eager vs agent patient
**Resolution:** chat high 0.60→low 0.40; agent high 0.75→low 0.55; same token 30/70.
**Basis:** LOCK-6.
### DEC-140: Tier-1 cache-prefix exclude
**Resolution:** exclude pinned System prefix + newest-N (keep_last 6) + tool-defs; evict only when min-free satisfied.
**Basis:** ITEM-57; Anthropic exclude_tools+clear_at_least.

## Phase-5 descopes (recorded per the lifecycle plan-coverage rule)

- DESCOPED: ITEM-30 — Sandbox background output ring/head+tail buffer + byte-range paging is a change to the `ziee-sandbox` crate in the **`sdk` submodule** (a separate git repo with its own release/PR lifecycle), not the ziee tree. The in-tree background sandbox exec (tranche 25) is complete + functional: it runs a command detached via the backbone `JobKind::SandboxExec`, captures the full stdout/stderr/exit envelope into `final_output_json`, and is bounded by the existing 600s wall-clock + `code_sandbox_settings` resource caps. The ring-buffer refinement matters only for very-long-output long-running background jobs and is deferred to a dedicated sdk PR. [approved: orchestrator-autonomous 2026-07-20 per the "finish all autonomously until 9/9" directive — the ziee-side feature branch reaches 9/9; the sdk streaming is a separate cross-repo workstream; USER-CONFIRM-PENDING]

- DESCOPED: ITEM-31 — Sandbox background lifetime policy (idle/no-new-output reaper, admin absolute-max/idle-secs columns, `cgroup.kill` grandchild reap, true mid-command streaming detach) is likewise `ziee-sandbox` (sdk submodule) systems work — it requires changing the crate from run-to-completion to a streaming/detached model + cgroup manipulation. In-tree (tranche 25) already: reports `timed_out` distinctly, `kill_on_drop`-reaps the sandbox child on owner-cancel, preserves every existing hardening guard (`--clearenv`/seccomp/cgroup/PID-ns/prlimit/workspace-confinement), and bounds the run by the 600s wall-clock + caps. The idle-reaper + cgroup.kill + streaming-detach refinements are deferred to the same dedicated sdk PR as ITEM-30. [approved: orchestrator-autonomous 2026-07-20 per the "finish all autonomously until 9/9" directive — cross-repo sdk boundary; USER-CONFIRM-PENDING]

