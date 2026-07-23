/**
 * Dev-gallery seed for the `scheduler` module — the `/scheduled-tasks` list, the
 * `/settings/scheduler` admin page, per-task run history, and the create/edit
 * task drawer overlay. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyNamed } from '@/dev/gallery/support'
import type { ScheduledTask } from '@/api-client/types'
import { SchedulerDrawer } from '@/modules/scheduler/stores/schedulerDrawer'

const USER = 'aaaa0000-0000-0000-0000-000000000001'

const tasks: ScheduledTask[] = [
  {
    id: 's0000000-0000-0000-0000-000000000001',
    name: 'Weekly literature scan',
    enabled: true,
    schedule_kind: 'recurring',
    cron_expr: '0 9 * * 1',
    timezone: 'America/New_York',
    target_kind: 'prompt',
    prompt: 'Search for new papers on CRISPR base editing published this week.',
    notify_mode: 'always',
    notify_on: 'on_change',
    consecutive_failures: 0,
    allowed_unattended_tools: [],
    inputs_json: {},
    last_run_at: '2026-01-06T09:00:00.000Z',
    next_run_at: '2026-01-13T09:00:00.000Z',
    last_status: 'success',
    created_at: '2025-12-01T00:00:00.000Z',
    updated_at: '2026-01-06T09:00:05.000Z',
    user_id: USER,
  },
  {
    id: 's0000000-0000-0000-0000-000000000002',
    name: 'Nightly grant-summary workflow',
    enabled: true,
    schedule_kind: 'recurring',
    cron_expr: '0 2 * * *',
    timezone: 'UTC',
    target_kind: 'workflow',
    workflow_id: 'wf000000-0000-0000-0000-000000000001',
    notify_mode: 'silent',
    notify_on: 'every_run',
    consecutive_failures: 1,
    allowed_unattended_tools: [],
    inputs_json: {},
    last_run_at: '2026-01-07T02:00:00.000Z',
    next_run_at: '2026-01-08T02:00:00.000Z',
    last_status: 'failed',
    created_at: '2025-11-15T00:00:00.000Z',
    updated_at: '2026-01-07T02:00:10.000Z',
    user_id: USER,
  },
  {
    id: 's0000000-0000-0000-0000-000000000003',
    name: 'One-off reminder',
    enabled: false,
    // `once` is the valid schedule_kind (the backend CHECK admits only
    // once|recurring|self_paced) — was seeded as the non-existent `one_off`,
    // which rendered a broken "Cron:" summary in the gallery.
    schedule_kind: 'once',
    run_at: '2026-02-01T15:00:00.000Z',
    timezone: 'Europe/London',
    target_kind: 'prompt',
    prompt: 'Remind me to renew the ethics approval.',
    notify_mode: 'always',
    notify_on: 'every_run',
    consecutive_failures: 0,
    allowed_unattended_tools: [],
    inputs_json: {},
    paused_reason: undefined,
    created_at: '2026-01-05T00:00:00.000Z',
    updated_at: '2026-01-05T00:00:00.000Z',
    user_id: USER,
  },
  {
    // Self-paced /loop task, still running (goal-seeking): a "Loop" badge, a
    // "Stops when: …" completion condition, an in-context conversation link, and
    // a model-chosen "Next check" instead of a cron next-run.
    id: 's0000000-0000-0000-0000-000000000004',
    name: 'Watch for the sequencing run to finish',
    enabled: true,
    schedule_kind: 'self_paced',
    timezone: 'America/New_York',
    target_kind: 'prompt',
    prompt: 'Check whether the sequencing run has completed and summarize the QC.',
    completion_condition:
      'the sequencing run reports COMPLETE and all QC metrics pass',
    bound_conversation_id: '11111111-1111-1111-1111-111111111111',
    notify_mode: 'always',
    notify_on: 'on_change',
    consecutive_failures: 0,
    allowed_unattended_tools: [],
    inputs_json: {},
    last_run_at: '2026-01-07T14:00:00.000Z',
    next_run_at: '2026-01-07T15:00:00.000Z',
    last_status: 'success',
    created_at: '2026-01-07T12:00:00.000Z',
    updated_at: '2026-01-07T14:00:05.000Z',
    user_id: USER,
  },
  {
    // Self-paced /loop task that self-stopped: `paused_reason: 'completed'`
    // renders as a green "Completed" badge + "Loop finished", NOT a generic
    // "Paused/Disabled" (tranche-2 convention, DEC-44 drift).
    id: 's0000000-0000-0000-0000-000000000005',
    name: 'Keep polling until the grant portal opens',
    enabled: false,
    schedule_kind: 'self_paced',
    timezone: 'UTC',
    target_kind: 'prompt',
    prompt: 'Check whether the NIH grant submission portal is open yet.',
    completion_condition:
      'the submission portal is open and accepting applications',
    bound_conversation_id: '11111111-1111-1111-1111-111111111111',
    paused_reason: 'completed',
    notify_mode: 'always',
    notify_on: 'every_run',
    consecutive_failures: 0,
    allowed_unattended_tools: [],
    inputs_json: {},
    last_run_at: '2026-01-06T18:00:00.000Z',
    last_status: 'success',
    created_at: '2026-01-05T09:00:00.000Z',
    updated_at: '2026-01-06T18:00:00.000Z',
    user_id: USER,
  },
]

export const gallery: ModuleGallery = {
  cassette: {
    'ScheduledTask.list': tasks,
    'SchedulerAdminSettings.get': {
      max_active_tasks_per_user: 20,
      max_consecutive_failures: 5,
      min_interval_seconds: 300,
      max_horizon_days: 7,
      notification_retention_days: 30,
      updated_at: '2026-01-01T00:00:00.000Z',
    },
    'ScheduledTask.listRuns': ctx => ({
      page: 1,
      per_page: 20,
      total: 2,
      runs: [
        {
          id: 'r0000000-0000-0000-0000-000000000001',
          scheduled_task_id: ctx.params.id ?? tasks[0].id,
          fired_at: '2026-01-06T09:00:00.000Z',
          finished_at: '2026-01-06T09:00:04.000Z',
          status: 'success',
          trigger: 'schedule',
          skipped_tools: [],
          result_preview: '3 new papers found; added to your reading list.',
          user_id: USER,
        },
        {
          id: 'r0000000-0000-0000-0000-000000000002',
          scheduled_task_id: ctx.params.id ?? tasks[0].id,
          fired_at: '2025-12-30T09:00:00.000Z',
          finished_at: '2025-12-30T09:00:03.000Z',
          status: 'success',
          trigger: 'schedule',
          skipped_tools: [],
          result_preview: 'No new papers this week.',
          user_id: USER,
        },
      ],
    }),
  },
  overlays: [
    {
      slug: 'overlay-scheduled-task-form-drawer',
      surface: 'modules/scheduler/components/ScheduledTaskFormDrawer',
      title: 'Scheduled task — create/edit (drawer)',
      component: lazyNamed(
        () => import('@/modules/scheduler/components/ScheduledTaskFormDrawer'),
        'ScheduledTaskFormDrawer',
      ),
      open: () => SchedulerDrawer.openCreate(),
    },
  ],
}
