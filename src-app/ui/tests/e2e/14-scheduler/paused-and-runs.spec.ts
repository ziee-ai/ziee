import { expect, test } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Paused-state surfacing + run history (ITEM-33): a task auto-paused after
 * repeated failures shows a "Paused" badge with its reason, and its "Runs"
 * section lists past firings with statuses. Mocks the list + runs endpoints.
 */

const TASK_ID = '44444444-4444-4444-4444-444444444444'

const pausedTask = {
  id: TASK_ID,
  user_id: '00000000-0000-0000-0000-000000000001',
  name: 'Flaky sweep',
  enabled: false,
  paused_reason: 'max_failures',
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
  next_run_at: null,
  last_run_at: '2026-07-09T09:00:00Z',
  last_status: 'failed',
  consecutive_failures: 5,
  notify_mode: 'always',
  notify_on: 'always',
  last_result_fingerprint: null,
  last_result_signature_json: null,
  bound_conversation_id: null,
  created_at: '2026-07-01T00:00:00Z',
  updated_at: '2026-07-09T09:00:00Z',
}

test('paused task shows its reason and lists run history', async ({ page, testInfra }) => {
  const { baseURL } = testInfra

  await page.route(/\/api\/scheduled-tasks$/, async route =>
    route.fulfill({ status: 200, json: [pausedTask] }),
  )
  await page.route(/\/api\/scheduled-tasks\/[^/]+\/runs$/, async route =>
    route.fulfill({
      status: 200,
      json: [
        {
          id: 'a1',
          scheduled_task_id: TASK_ID,
          fired_at: '2026-07-09T09:00:00Z',
          status: 'failed',
          error_class: 'provider_error',
          trigger: 'schedule',
          notification_id: null,
          workflow_run_id: null,
          conversation_id: null,
        },
      ],
    }),
  )

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  // The paused badge with its reason renders.
  await expect(byTestId(page, `task-paused-${TASK_ID}`)).toContainText('max_failures', {
    timeout: 10000,
  })

  // Expand the runs section → the past firing is listed with its status +
  // error class (unique run-row text avoids matching the card's "Last: failed").
  await byTestId(page, `task-runs-toggle-${TASK_ID}`).click()
  await expect(
    page.getByText(/failed \(provider_error\)/),
  ).toBeVisible({ timeout: 10000 })
})
