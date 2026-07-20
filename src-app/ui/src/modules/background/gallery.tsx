/**
 * Dev-gallery seed for the `background` module — the `/background-tasks` list of
 * detached sub-agent / sandbox-exec runs (status badges, cancel, steer, go-to-
 * conversation). Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'

const RUNNING = 'b0000000-0000-0000-0000-000000000001'

export const gallery: ModuleGallery = {
  cassette: {
    'Background.listRuns': {
      page: 1,
      per_page: 10,
      total: 4,
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
          id: 'b0000000-0000-0000-0000-000000000002',
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
          id: 'b0000000-0000-0000-0000-000000000003',
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
          id: 'b0000000-0000-0000-0000-000000000004',
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
