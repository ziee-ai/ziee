# INFRA_INTEGRATION — scheduled-tasks (per FB-6)

FB-6 mandates, for EVERY item, two walks: (1) the USER-EXPERIENCE walk and (2) the
INFRASTRUCTURE-INTEGRATION walk (enumerate every existing subsystem the item touches and check
each for specific behaviors/constraints that must be handled, not assumed). This is the living
artifact. It is re-checked per item during Phase 5.

## Item × infrastructure matrix

| Item(s) | UX walk (how a real user hits it) | Infrastructure touched → specific concern to handle |
|---|---|---|
| ITEM-1 pickers | User opens New Task → picks Assistant/Workflow/Model by NAME from a dropdown. | **AssistantPicker/Workflow/ModelPicker stores** — each self-gates its fetch on the user's read perm (assistants::read / workflows::read / user_llm_providers::read, all Users-held) → loads for scheduler users. **Empty states** — no models → the ModelSelector empty-CTA (route to /settings/llm-providers); no assistants → "Default assistant" still selectable. Must NOT mutate the GLOBAL picker selection (chat composer) — use controlled wrappers (DEC-14). |
| ITEM-2 tz auto-detect | User never sees a tz field; times shown are "in your timezone". | **schedule.rs** evaluates cron in the stored IANA zone → the auto-detected `browserTz()` must be a valid IANA name (it is, from `Intl`). **Edit path** — an existing task's stored zone may differ from the current browser; show it read-only, don't silently rewrite (data-integrity). |
| ITEM-3 FormField / ITEM-16 tool picker | Standard settings-drawer form. | **Design system** — `FormField` is mandated (DESIGN_SYSTEM.md); the drawer currently violates it. **gallery state-matrix** — new conditional renders (picker empty states, workflow-inputs vs JSON) need gallery cells or the `check:state-matrix` gate fails (phase 8). |
| ITEM-4 typed workflow inputs | User picks a workflow → sees its real input fields, not JSON. | **workflowIr / parseWorkflowIr** — reused as-is; a workflow with no declared inputs → JSON fallback (DEC-13). **Backend** — `inputs_json` is still free JSONB (no server schema validation, Bug 10) → the typed form is the client-side guard. |
| ITEM-5/6 create/update validation | User picks an inaccessible model → clear error at save, not a silent broken task. | **workflow/runner user_has_access_to_provider** (model access), **assistant get_for_user** (ownership), **quota count** (repository) — reuse all three; quota count must EXCLUDE the row being updated (DEC-4, off-by-one). **HTTP contract** — 404/403 not 500 (DEC-2). |
| ITEM-7 referent-deleted pause | A task whose conversation/workflow was deleted stops cleanly, user sees "paused: conversation deleted". | **FK ON DELETE SET NULL** (bound_conversation_id / workflow_id) — the NULL is the signal; discriminate first-run vs deleted via `last_status` (DEC-5). **notifications** — a pre-emptive pause must NOT emit a spurious failure notification (unlike today's reactive path). |
| ITEM-8 run prune / ITEM-10 completed | Old run rows don't accumulate; a one-shot shows "Completed". | **notification/prune.rs boot loop** — extend it (don't fork a new scheduler); reuse `notification_retention_days` (DEC-7). **sync** — a completed once-task flips enabled=false + emits the existing `ScheduledTask` sync entity so other devices refresh. |
| ITEM-9 transient retry | A flaky network blip doesn't auto-pause the user's task. | **failure.rs is_retryable/retry_backoff_ms** (reuse) + **tick.rs spawns fire_task off-loop** → the bounded retry sleep can't stall the claim loop (DEC-9). **consecutive-failure cap** (admin setting) semantics unchanged for non-transient. |
| ITEM-11 log notif errors | (invisible) — diagnosability only. | **tracing** — structured warn; no behavior change. |
| ITEM-12 multi-day | User picks Mon+Wed+Fri in one control. | **schedule.rs cron parser** accepts comma dow already (no backend change); **min-interval floor** samples 24 occurrences → multi-day validated. **humanizeCron** (page) must parse comma lists or degrade to `Cron:`. |
| **ITEM-13/14 unattended approval + server-constrain** | The user's scheduled research task runs its read-only tools; a tool needing approval is cleanly skipped + reported, never silently dropped; nothing side-effecting fires unattended. | **CHAT PIPELINE (shared!)** — `SendMessageRequest` / `StreamContext.metadata` gain an ADDITIVE default-false `unattended` flag; interactive chat MUST be byte-identical (B3). **MCP chat extension `after_llm_call`** (`mcp.rs:2043-2116`, pause block `2705-2727`) — branch to deny-not-pause; still persist approval-exempt built-in results so the turn stays protocol-valid ("tool_use without tool_result" is a hard provider error). **approval-mode resolution chain** (`mcp.rs:1965-1978`) — inject the unattended branch. **is_builtin_server_id / is_side_effect_tool** (reuse for the read-only classification). **mcp_config** (`mcp.rs:1352-1359`) — pass an explicit server set instead of the "None ⇒ all accessible" default. **elicitation/ask_user** (`helpers.rs`) — reachable in a scheduled run, burns ≤300s; under unattended, treat as auto-cancel fast (don't wait the full timeout). |
| ITEM-15 allow-list field | User ticks the specific tools this task may use unattended. | **migration 153** (next free vs 145); **get_all_accessible_config** — validate allow-list ⊆ accessible (no privilege widening — security). **OpenAPI regen** BOTH workspaces. |
| ITEM-17 skipped-tools report | User's notification/Runs tab honestly says "1 tool skipped". | **scheduled_task_runs** new column; **notification body** composition (dispatch finalize); **sync** `McpToolCall`/run refresh. |
| ITEM-18 workflow elicit reject + disabled-servers | A user can't schedule a workflow that would hang on a human prompt; scheduled workflow honors the user's disabled servers. | **workflow IR (parseWorkflowIr)** — detect `elicit` steps at create (reuse ITEM-4 parse). **workflow/dispatch.rs tool-step** — the `disabled_servers` filter is conversation-scoped (`workflow/dispatch.rs:1107`); a scheduled run has no conversation → thread the user's default disabled set via `invocation_source=scheduled`. **workflow runner Waiting/Suspended** status — a durable elicit gate strands the run; create-time rejection avoids it. |

## Cross-cutting integration risks (must-verify in Phase 5/6)
- **Shared chat pipeline (ITEM-13):** highest blast radius. The `unattended` flag must be additive + default-false; the phase-6 blind audit MUST include an "interactive chat unchanged" angle, and TEST-24 is the regression proof.
- **Desktop overrides (R2-3):** the drawer + any permission/gating logic changed in `ui/` must be diffed against `desktop/ui/` overrides — a dropped filter once reached desktop prod.
- **Permissions:** no NEW permission is introduced (allow-list ⊆ existing accessible set), so no A9/A10 obligation — but the allow-list validator (TEST-29) IS the escalation guard and must be a real deny test.
- **OpenAPI (ITEM-15/17):** regen BOTH workspaces or merge-gate C3 fails.

## Round 3 (FB-9 precedent audit) — per-item walks

Pure-frontend layout/precedent delta. UX walk + infra-integration walk per item:

- **ITEM-49/50 (route layout + shell)** — UX: clicking the sidebar "Scheduled Tasks"
  entry now keeps the left sidebar + top header bar (was: a bare, chrome-less page).
  Infra: the router (`RouterComponent.tsx`: `route.layout || null`) applies NO shell
  when `layout` is absent — `AppLayoutDef` is the exact seam chat/projects/knowledge-base
  use; `HeaderBarContainer` is a per-page opt-in (AppLayout renders bare `{children}`),
  so the page must render it itself. `useNativeScroll(true)` + `Stores.AppLayout.nativeScroll`
  drive mobile document-scroll vs desktop inner-scroll — same store other top-level pages use.
- **ITEM-51 (Load-More)** — UX: a user with many tasks sees the first 12 + "Showing N of M"
  + Load More, not an unbounded wall. Infra: `.list()` already returns the full (admin-cap-bounded)
  set; client-side slice mirrors KB/projects (no server-paging change, no store change).
- **ITEM-52/53/54 (card)** — UX: a decluttered card; the on/off Switch stays visible (state),
  action icons reveal on hover/focus and are always-on for touch (`hover-none:opacity-100`).
  Infra: reuses `Card` title/extra + `Confirm`/`Tooltip` (same kit primitives as KnowledgeBaseCard);
  the inline `Confirm` overlay is allow-listed in the gallery overlay registry (mirrors KB).
- **ITEM-55 (empty)** — UX: an actionable empty state (icon + heading + create button) instead of
  a bare sentence. Infra: `Empty` kit component + `<Can permission={SchedulerUse}>` gate (the same
  permission gating the nav entry + the page route + the header create button — no ungated surface).
- **ITEM-56/57/58 (drawer + schedule)** — UX: invalid submits now surface a message (zodResolver +
  onInvalid), and every control sits in a labelled Field/FormField. Infra: `useForm`+`zodResolver`
  (RHF, identical to ProjectFormDrawer); the DYNAMIC "required declared workflow inputs" check stays
  imperative because the workflow IR isn't in the form value (zod can't see it); `Field/FieldTitle`
  from the shadcn field kit (same as SchedulerAdminPage). No backend/API/sync/permission change.
- **ITEM-59 (responsive)** — UX: header + card actions usable at 390px (actions wrap; runs panel
  already has a mobile overflow menu). Infra: the nativeScroll threading + `sm:` breakpoints in the
  page/card; gallery narrow-viewport is enforced by `gate:ui` (phase 8).
