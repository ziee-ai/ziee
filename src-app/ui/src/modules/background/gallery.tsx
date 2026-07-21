/**
 * Dev-gallery seed for the `background` module — the `/background-tasks` list of
 * detached sub-agent / sandbox-exec runs (status badges, cancel, steer, go-to-
 * conversation). Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { BackgroundRunDetail } from '@/api-client/types'
import type { ModuleGallery } from '@/dev/gallery/support'

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

export const gallery: ModuleGallery = {
  cassette: {
    'Background.listRuns': {
      page: 1,
      per_page: 10,
      total: 5,
      total_pages: 1,
      runs: [
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
      ],
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
