import { expect, test } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — "Continue in chat" from a run (ITEM-32 / ITEM-33): the runs list exposes
 * a per-run "Continue in chat" action; clicking it calls the continue endpoint
 * (which opens a new seeded conversation) and navigates the user there. Mocks
 * the list / runs / continue endpoints at the HTTP boundary.
 */

const TASK_ID = '55555555-5555-5555-5555-555555555555'
const RUN_ID = '66666666-6666-6666-6666-666666666666'
const CONV_ID = '77777777-7777-7777-7777-777777777777'

const task = {
  id: TASK_ID,
  user_id: '00000000-0000-0000-0000-000000000001',
  name: 'History task',
  enabled: true,
  paused_reason: null,
  target_kind: 'prompt',
  workflow_id: null,
  inputs_json: {},
  assistant_id: null,
  prompt: 'Sweep.',
  model_id: '11111111-1111-1111-1111-111111111111',
  schedule_kind: 'recurring',
  run_at: null,
  cron_expr: '0 9 * * 1',
  timezone: 'UTC',
  next_run_at: '2026-07-13T09:00:00Z',
  last_run_at: '2026-07-09T09:00:00Z',
  last_status: 'completed',
  consecutive_failures: 0,
  notify_mode: 'always',
  notify_on: 'always',
  last_result_fingerprint: null,
  last_result_signature_json: null,
  bound_conversation_id: null,
  created_at: '2026-07-01T00:00:00Z',
  updated_at: '2026-07-09T09:00:00Z',
}

test('a run offers "Continue in chat" which calls the continue endpoint', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  let continueCalled = false

  await page.route(/\/api\/scheduled-tasks$/, async route =>
    route.fulfill({ status: 200, json: [task] }),
  )
  await page.route(/\/api\/scheduled-tasks\/[^/]+\/runs(\?.*)?$/, async route =>
    route.fulfill({
      status: 200,
      json: {
        runs: [
          {
            id: RUN_ID,
            scheduled_task_id: TASK_ID,
            user_id: task.user_id,
            trigger: 'schedule',
            status: 'completed',
            error_class: null,
            error_message: null,
            notification_id: null,
            workflow_run_id: null,
            conversation_id: null,
            result_preview: 'Sweep produced 2 items.',
            change_summary_json: { changed: true, new_count: 2, new_items: [] },
            fired_at: '2026-07-09T09:00:00Z',
            finished_at: '2026-07-09T09:00:05Z',
          },
        ],
        total: 1,
        page: 1,
        per_page: 10,
      },
    }),
  )
  await page.route(/\/api\/scheduled-tasks\/runs\/[^/]+\/continue$/, async route => {
    continueCalled = true
    await route.fulfill({ status: 201, json: { conversation_id: CONV_ID } })
  })

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  // Expand the runs section, then click the per-run fork ("New side chat" for a
  // prompt task) — it calls the continue endpoint (opening a seeded conversation).
  await byTestId(page, `task-runs-toggle-${TASK_ID}`).click()
  await expect(byTestId(page, `run-action-fork-${RUN_ID}`)).toBeVisible({ timeout: 10000 })
  await byTestId(page, `run-action-fork-${RUN_ID}`).click()

  // The continue endpoint was invoked (opening the seeded conversation).
  await expect.poll(() => continueCalled, { timeout: 10000 }).toBe(true)
})
