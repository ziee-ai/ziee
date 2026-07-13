import { loginAsAdmin } from '../../common/auth-helpers'
import { expect, test } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { MODEL_ID, mockPickerEndpoints, pickSelect } from './helpers'

/**
 * E2E — Scheduled-task drawer form idiom (FB-9 / ITEM-56/57/58). Proves the
 * controls sit in labelled Field/FormField rows and that zodResolver blocks an
 * invalid submit with a visible error (not a silent no-op).
 */

function baseRow(overrides: Record<string, unknown>) {
  return {
    id: '22222222-2222-2222-2222-222222222222',
    user_id: '00000000-0000-0000-0000-000000000001',
    name: 'task',
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
    allowed_unattended_tools: [],
    created_at: '2026-07-09T00:00:00Z',
    updated_at: '2026-07-09T00:00:00Z',
    ...overrides,
  }
}

async function mockTasksEndpoint(page: import('@playwright/test').Page) {
  const captured: { posted: boolean } = { posted: false }
  let created: ReturnType<typeof baseRow> | null = null
  await page.route(/\/api\/scheduled-tasks$/, async (route, req) => {
    if (req.method() === 'POST') {
      captured.posted = true
      created = baseRow(req.postDataJSON() as Record<string, unknown>)
      await route.fulfill({ status: 201, json: created })
      return
    }
    await route.fulfill({ status: 200, json: created ? [created] : [] })
  })
  return captured
}

test('TEST-62: labelled Field/FormField controls + zodResolver blocks an invalid submit', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page)
  const captured = await mockTasksEndpoint(page)

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)
  await byTestId(page, 'scheduled-tasks-new').click()
  await expect(byTestId(page, 'task-form')).toBeVisible({ timeout: 10000 })

  // Controls sit in labelled Field/FormField rows.
  await expect(byTestId(page, 'task-form-target-kind')).toBeVisible()
  await expect(page.getByText('Type', { exact: true })).toBeVisible()
  await expect(page.getByText('Schedule', { exact: true })).toBeVisible()
  await expect(page.getByText('Show a toast when it runs')).toBeVisible()
  await expect(page.getByText('Only notify when results change')).toBeVisible()

  // Invalid submit (empty name/prompt/model) is BLOCKED by zodResolver: a
  // visible field error appears, the drawer stays open, and nothing is POSTed.
  await byTestId(page, 'task-form-save').click()
  await expect(byTestId(page, 'field-error-name')).toBeVisible({ timeout: 10000 })
  await expect(byTestId(page, 'task-form')).toBeVisible()
  expect(captured.posted).toBe(false)

  // A valid submit then creates the task.
  await byTestId(page, 'task-form-name').fill('Valid task')
  await byTestId(page, 'task-form-prompt').fill('Do the thing.')
  await pickSelect(page, 'task-form-model', MODEL_ID)
  await byTestId(page, 'task-form-save').click()

  await expect(page.getByText('Valid task')).toBeVisible({ timeout: 10000 })
  expect(captured.posted).toBe(true)
})
