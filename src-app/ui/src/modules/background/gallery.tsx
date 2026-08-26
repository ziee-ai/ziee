/**
 * Dev-gallery seed for the `background` module — the sub-agent / sandbox-exec
 * runs (status badges, cancel, steer, result view) that the IN-CONVERSATION
 * surfaces render: the right-panel "Tasks" tab and the end-of-conversation
 * footer affordance. There is no global `/background-tasks` page any more, so
 * this module contributes no page surface of its own; the two states live on the
 * chat module's deep states (`deep-chat-right-panel-background` /
 * `deep-chat-background-footer`) and consume this cassette.
 *
 * Auto-discovered by the gallery's runtime registry (`@/dev/gallery/support`);
 * never imported by `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { BackgroundRunDetail, BackgroundRunSummary } from '@/api-client/types'
import type { ModuleGallery } from '@/dev/gallery/support'
import { PANEL_PAGE_SIZE } from './stores/BackgroundRuns.store'

const RUNNING = 'b0000000-0000-0000-0000-000000000001'
const SUBAGENT_DONE = 'b0000000-0000-0000-0000-000000000002'
const SANDBOX_FAILED = 'b0000000-0000-0000-0000-000000000003'
const SUBAGENT_CANCELLED = 'b0000000-0000-0000-0000-000000000004'
const SANDBOX_DONE = 'b0000000-0000-0000-0000-000000000005'

// Full run details keyed by run id — served by the `Background.getRun` resolver
// when a card's result view is expanded, so a completed sub-agent AND a completed
// sandbox-exec run render their `final_output_json` populated for review.
const RUN_DETAILS: Record<string, BackgroundRunDetail> = {
  [SUBAGENT_DONE]: {
    id: SUBAGENT_DONE,
    job_kind: 'subagent',
    label: 'Summarize the last 40 support tickets',
    status: 'completed',
    has_result: true,
    total_tokens: 52210,
    conversation_id: 'c0000000-0000-0000-0000-000000000002',
    created_at: '2026-01-03T09:10:00.000Z',
    updated_at: '2026-01-03T09:18:00.000Z',
    // The durable agent-loop transcript, rendered under the card's Transcript
    // tab by the shared `AgentActivityTimeline`. Seeded here so the populated
    // transcript state is a real, browsable gallery cell (reviewed at desktop +
    // 390px), not just empty/error.
    activity: [
      {
        type: 'agent_activity',
        seq: 0,
        kind: 'thinking',
        title: 'Grouping the 40 tickets by theme',
        status: 'ok',
      },
      {
        type: 'agent_activity',
        seq: 1,
        kind: 'tool_call',
        tool: 'search_knowledge',
        title: 'Searching prior ticket resolutions',
        detail: 'query=proration upgrade',
        status: 'ok',
      },
      {
        type: 'agent_activity',
        seq: 2,
        kind: 'message',
        title: 'Wrote the themed summary + recommendation',
        status: 'ok',
      },
    ],
    final_output_json: {
      executor: 'agent-core',
      status: 'completed',
      final_text:
        'Support ticket summary (last 40)\n\n' +
        'Top themes:\n' +
        '- Billing & invoices: 12 tickets — mostly proration confusion on mid-cycle upgrades.\n' +
        '- Onboarding / SSO setup: 9 tickets — SAML metadata URL step is the common blocker.\n' +
        '- Data export: 7 tickets — users expect CSV, we return JSON.\n' +
        '- Misc bugs: 12 tickets — no single cluster.\n\n' +
        'Recommendation: add a proration explainer to the upgrade dialog and a CSV export toggle.',
      tokens_used: 52210,
      spec: {
        system: 'You are a support-analytics sub-agent.',
        task: 'Summarize the last 40 support tickets',
      },
    },
  },
  [SANDBOX_DONE]: {
    id: SANDBOX_DONE,
    job_kind: 'sandbox_exec',
    label: 'Count rows across the uploaded datasets',
    status: 'completed',
    has_result: true,
    total_tokens: 3100,
    created_at: '2026-01-03T07:30:00.000Z',
    updated_at: '2026-01-03T07:31:00.000Z',
    final_output_json: {
      executor: 'code-sandbox',
      kind: 'sandbox_exec',
      status: 'completed',
      command: 'python count_rows.py data/*.csv',
      flavor: 'full',
      exit_code: 0,
      timed_out: false,
      stdout: 'orders.csv: 18042 rows\ncustomers.csv: 5120 rows\nrefunds.csv: 311 rows\n',
      stderr: '',
      duration_ms: 842,
      stdout_truncated: false,
      stderr_truncated: false,
    },
  },
  [SANDBOX_FAILED]: {
    id: SANDBOX_FAILED,
    job_kind: 'sandbox_exec',
    label: 'Run the regression benchmark suite',
    status: 'failed',
    has_result: false,
    total_tokens: 4100,
    error_message: 'command exited 137 (out of memory) after 512 MB cap',
    created_at: '2026-01-03T08:40:00.000Z',
    updated_at: '2026-01-03T08:52:00.000Z',
  },
  [SUBAGENT_CANCELLED]: {
    id: SUBAGENT_CANCELLED,
    job_kind: 'subagent',
    label: 'Draft the quarterly board deck',
    status: 'cancelled',
    has_result: false,
    total_tokens: 9300,
    conversation_id: 'c0000000-0000-0000-0000-000000000004',
    created_at: '2026-01-02T16:20:00.000Z',
    updated_at: '2026-01-02T16:25:00.000Z',
  },
}

/**
 * A conversation id the cassette answers with ZERO runs, so the Tasks panel's
 * EMPTY branch has a real gallery delivery (the `deep-chat-background-empty`
 * deep state opens the panel on this id). Every other id gets the five runs.
 */
export const GALLERY_EMPTY_TASKS_CONVERSATION_ID =
  'c0000000-0000-0000-0000-0000000000e0'

const ALL_RUNS: BackgroundRunSummary[] = [
  {
    id: RUNNING,
    job_kind: 'subagent',
    label: 'Competitor landscape scan',
    status: 'running',
    has_result: false,
    total_tokens: 18400,
    conversation_id: 'c0000000-0000-0000-0000-000000000001',
    model_id: 'm0000000-0000-0000-0000-000000000001',
    created_at: '2026-01-03T10:02:00.000Z',
    updated_at: '2026-01-03T10:05:00.000Z',
  },
  {
    id: SUBAGENT_DONE,
    job_kind: 'subagent',
    label: 'Summarize the last 40 support tickets',
    status: 'completed',
    has_result: true,
    total_tokens: 52210,
    conversation_id: 'c0000000-0000-0000-0000-000000000002',
    created_at: '2026-01-03T09:10:00.000Z',
    updated_at: '2026-01-03T09:18:00.000Z',
  },
  {
    id: SANDBOX_DONE,
    job_kind: 'sandbox_exec',
    label: 'Count rows across the uploaded datasets',
    status: 'completed',
    has_result: true,
    total_tokens: 3100,
    created_at: '2026-01-03T07:30:00.000Z',
    updated_at: '2026-01-03T07:31:00.000Z',
  },
  {
    id: SANDBOX_FAILED,
    job_kind: 'sandbox_exec',
    label: 'Run the regression benchmark suite',
    status: 'failed',
    has_result: false,
    total_tokens: 4100,
    error_message: 'command exited 137 (out of memory) after 512 MB cap',
    created_at: '2026-01-03T08:40:00.000Z',
    updated_at: '2026-01-03T08:52:00.000Z',
  },
  {
    id: SUBAGENT_CANCELLED,
    job_kind: 'subagent',
    label: 'Draft the quarterly board deck',
    status: 'cancelled',
    has_result: false,
    total_tokens: 9300,
    conversation_id: 'c0000000-0000-0000-0000-000000000004',
    created_at: '2026-01-02T16:20:00.000Z',
    updated_at: '2026-01-02T16:25:00.000Z',
  },
]

export const gallery: ModuleGallery = {
  cassette: {
    // Conversation-aware: the panel + footer always request a `conversation_id`,
    // and the designated EMPTY id resolves to a zero-run page so the panel's
    // empty branch is a real, browsable gallery state rather than an allow-listed
    // gap. Every other id gets the full five-run spread.
    'Background.listRuns': ctx => {
      // `conversation_id` is a QUERY param, so it arrives on `ctx.query` — NOT
      // `ctx.params`, which holds PATH captures only (that mistake made the
      // empty-state delivery silently return the populated list).
      const requested = ctx.query.conversation_id
      const empty = requested === GALLERY_EMPTY_TASKS_CONVERSATION_ID
      // Stamp the REQUESTED conversation onto every run, so the seeded panel is
      // faithful to the endpoint it stands in for: the disjoint server scope can
      // only ever return runs belonging to the conversation asked for. Leaving the
      // fixtures' own ids in place produced a gallery state that cannot occur.
      const runs = empty
        ? []
        : ALL_RUNS.map(r => ({ ...r, conversation_id: requested ?? r.conversation_id }))
      return {
        page: 1,
        per_page: PANEL_PAGE_SIZE,
        total: runs.length,
        total_pages: empty ? 0 : 1,
        runs,
      }
    },
    // Full run detail (incl. `final_output_json`) fetched when a card's result
    // view is expanded — keyed by run id; a completed sub-agent + completed
    // sandbox run both resolve to a populated body.
    'Background.getRun': ctx =>
      RUN_DETAILS[ctx.params.run_id ?? SUBAGENT_DONE] ??
      RUN_DETAILS[SUBAGENT_DONE],
    // Pending steering notes for the running run (loaded when its composer opens).
    'Background.listRunNotes': ctx => [
      {
        id: 'n0000000-0000-0000-0000-000000000001',
        run_id: ctx.params.run_id ?? RUNNING,
        note: 'Focus on EU competitors only; skip the US market.',
        created_at: '2026-01-03T10:04:00.000Z',
      },
    ],
  },
}
