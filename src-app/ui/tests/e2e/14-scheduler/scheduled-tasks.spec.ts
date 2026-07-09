import { expect, test } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Scheduled Tasks page: empty state → create a task via the drawer →
 * the task appears in the list. Mocks the scheduler REST endpoints at the HTTP
 * boundary and asserts the UI drives the real create flow (drawer form →
 * POST /api/scheduled-tasks → optimistic list insert).
 */

const MODEL_ID = '11111111-1111-1111-1111-111111111111'

function taskRow(name: string) {
  return {
    id: '22222222-2222-2222-2222-222222222222',
    user_id: '00000000-0000-0000-0000-000000000001',
    name,
    enabled: true,
    paused_reason: null,
    target_kind: 'prompt',
    workflow_id: null,
    inputs_json: {},
    assistant_id: null,
    prompt: 'Say hello.',
    model_id: MODEL_ID,
    schedule_kind: 'recurring',
    run_at: null,
    cron_expr: '0 9 * * 1',
    timezone: 'UTC',
    next_run_at: '2026-07-13T09:00:00Z',
    last_run_at: null,
    last_status: null,
    consecutive_failures: 0,
    notify_mode: 'always',
    notify_on: 'always',
    last_result_fingerprint: null,
    last_result_signature_json: null,
    bound_conversation_id: null,
    created_at: '2026-07-09T00:00:00Z',
    updated_at: '2026-07-09T00:00:00Z',
  }
}

test('create a scheduled task from the drawer and see it listed', async ({ page, baseURL }) => {
  let created: ReturnType<typeof taskRow> | null = null

  await page.route(/\/api\/scheduled-tasks$/, async (route, req) => {
    if (req.method() === 'POST') {
      const body = req.postDataJSON() as { name: string }
      created = taskRow(body.name)
      await route.fulfill({ status: 201, json: created })
      return
    }
    // GET list
    await route.fulfill({ status: 200, json: created ? [created] : [] })
  })

  await loginAsAdmin(page, baseURL as string)
  await page.goto(`${baseURL}/scheduled-tasks`)

  // Empty state first.
  await expect(byTestId(page, 'scheduled-tasks-empty')).toBeVisible({ timeout: 10000 })

  // Open the create drawer + fill the form.
  await byTestId(page, 'scheduled-tasks-new').click()
  await byTestId(page, 'task-form-name').fill('Weekly digest')
  await byTestId(page, 'task-form-prompt').fill('Summarize the week.')
  await byTestId(page, 'task-form-model').fill(MODEL_ID)
  await byTestId(page, 'task-form-save').click()

  // The created task now appears in the list.
  await expect(page.getByText('Weekly digest')).toBeVisible({ timeout: 10000 })
})
