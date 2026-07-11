import { loginAsAdmin } from '../../common/auth-helpers'
import { expect, test } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  ASSISTANT_ID,
  ASSISTANT_NAME,
  MODEL_ID,
  SERVER_ID,
  SERVER_NAME,
  WORKFLOW_ID,
  WORKFLOW_NAME,
  mockPickerEndpoints,
  pickCombobox,
  pickMultiSelect,
  pickSelect,
  workflowWithInputs,
} from './helpers'

/**
 * E2E — Scheduled Tasks page: the create-drawer now drives NAMED pickers
 * (Assistant / Model / allowed-tools) instead of raw-UUID text inputs, an
 * auto-detected read-only timezone (no input), and a weekly multi-day toggle.
 * The scheduler REST endpoints are mocked at the HTTP boundary; the create POST
 * echoes the submitted body so the list reflects the real picked values.
 */

// A base task row; the POST handler merges the submitted body onto it so the
// listed task reflects exactly what the drawer sent.
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

/** Wire the list+create endpoints; capture the POST body. Returns a getter. */
async function mockTasksEndpoint(page: import('@playwright/test').Page) {
  const captured: { body?: Record<string, unknown> } = {}
  let created: ReturnType<typeof baseRow> | null = null
  await page.route(/\/api\/scheduled-tasks$/, async (route, req) => {
    if (req.method() === 'POST') {
      const body = req.postDataJSON() as Record<string, unknown>
      captured.body = body
      created = baseRow(body)
      await route.fulfill({ status: 201, json: created })
      return
    }
    await route.fulfill({ status: 200, json: created ? [created] : [] })
  })
  return captured
}

test('TEST-1: create a task by picking an Assistant + Model (no typed IDs)', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page)
  const captured = await mockTasksEndpoint(page)

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  await expect(byTestId(page, 'scheduled-tasks-empty')).toBeVisible({
    timeout: 10000,
  })

  await byTestId(page, 'scheduled-tasks-new').click()
  await byTestId(page, 'task-form-name').fill('Weekly digest')
  await byTestId(page, 'task-form-prompt').fill('Summarize the week.')

  // Pick — never type — the Assistant and Model.
  await pickCombobox(page, 'task-form-assistant', ASSISTANT_NAME, ASSISTANT_ID)
  await pickSelect(page, 'task-form-model', MODEL_ID)

  await byTestId(page, 'task-form-save').click()

  // The created task appears in the list.
  await expect(page.getByText('Weekly digest')).toBeVisible({ timeout: 10000 })

  // It was created from the PICKED ids (not typed strings).
  expect(captured.body?.model_id).toBe(MODEL_ID)
  expect(captured.body?.assistant_id).toBe(ASSISTANT_ID)
})

test('TEST-3: no timezone input; read-only detected tz saved on create', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page)
  const captured = await mockTasksEndpoint(page)

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  await byTestId(page, 'scheduled-tasks-new').click()

  // The detected timezone is read-only text — and there is NO editable tz input.
  const note = byTestId(page, 'schedule-timezone-note')
  await expect(note).toBeVisible({ timeout: 10000 })
  await expect(page.getByTestId('schedule-timezone')).toHaveCount(0)

  const noteText = (await note.textContent()) ?? ''
  const detectedTz = noteText.replace(/.*your timezone:\s*/i, '').trim()
  expect(detectedTz.length).toBeGreaterThan(0)

  await byTestId(page, 'task-form-name').fill('TZ task')
  await byTestId(page, 'task-form-prompt').fill('Do a thing.')
  await pickSelect(page, 'task-form-model', MODEL_ID)
  await byTestId(page, 'task-form-save').click()

  await expect(page.getByText('TZ task')).toBeVisible({ timeout: 10000 })
  // Saved with the browser-detected tz — the user never entered it.
  expect(captured.body?.timezone).toBe(detectedTz)
})

test('TEST-21: weekly multi-day (Mon+Wed+Fri) → "Weekly on Mon, Wed, Fri"', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page)
  const captured = await mockTasksEndpoint(page)

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  await byTestId(page, 'scheduled-tasks-new').click()
  await byTestId(page, 'task-form-name').fill('Multi day')
  await byTestId(page, 'task-form-prompt').fill('Report.')
  await pickSelect(page, 'task-form-model', MODEL_ID)

  // Default recurring schedule is `0 9 * * 1` → weekly preset, Monday selected.
  // Toggle Wed (3) + Fri (5) via the day multi-toggle.
  await expect(byTestId(page, 'schedule-dow-1')).toHaveAttribute(
    'aria-pressed',
    'true',
  )
  await byTestId(page, 'schedule-dow-3').click()
  await byTestId(page, 'schedule-dow-5').click()

  await byTestId(page, 'task-form-save').click()

  // The list summary humanizes the multi-day cron.
  await expect(
    page.getByText(/Weekly on Mon, Wed, Fri at 09:00/),
  ).toBeVisible({ timeout: 10000 })
  expect(captured.body?.cron_expr).toBe('0 9 * * 1,3,5')
})

test('TEST-2: workflow target — typed inputs (and JSON fallback) render; create succeeds', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page, { workflows: [workflowWithInputs] })
  const captured = await mockTasksEndpoint(page)

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  await byTestId(page, 'scheduled-tasks-new').click()
  await byTestId(page, 'task-form-name').fill('Workflow task')

  // Switch the target to Workflow (Segmented) → a Workflow picker appears, and
  // with no workflow chosen yet the inputs fall back to the free-form JSON editor.
  await byTestId(page, 'task-form-target-kind-opt-workflow').click()
  await expect(byTestId(page, 'task-form-workflow')).toBeVisible({
    timeout: 10000,
  })
  await expect(byTestId(page, 'task-form-inputs')).toBeVisible({
    timeout: 10000,
  })

  // Picking a workflow that DECLARES inputs swaps the JSON editor for a typed
  // field per declared input.
  await pickCombobox(page, 'task-form-workflow', WORKFLOW_NAME, WORKFLOW_ID)
  await expect(byTestId(page, 'task-form-input-topic')).toBeVisible({
    timeout: 10000,
  })
  await expect(page.getByTestId('task-form-inputs')).toHaveCount(0)
  await byTestId(page, 'task-form-input-topic').fill('CRISPR')

  await pickSelect(page, 'task-form-model', MODEL_ID)
  await byTestId(page, 'task-form-save').click()

  await expect(page.getByText('Workflow task')).toBeVisible({ timeout: 10000 })
  expect(captured.body?.target_kind).toBe('workflow')
  expect(captured.body?.workflow_id).toBe(WORKFLOW_ID)
})

test('TEST-30: allow-list picker present; selections persist on create', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  await mockPickerEndpoints(page, {
    servers: [{ id: SERVER_ID, name: SERVER_NAME, display_name: SERVER_NAME }],
  })
  const captured = await mockTasksEndpoint(page)

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  await byTestId(page, 'scheduled-tasks-new').click()
  await byTestId(page, 'task-form-name').fill('Unattended task')
  await byTestId(page, 'task-form-prompt').fill('Run tools.')
  await pickSelect(page, 'task-form-model', MODEL_ID)

  // The "Tools this task may use unattended" picker is present (empty by default).
  await expect(byTestId(page, 'task-form-allowed-tools')).toBeVisible({
    timeout: 10000,
  })
  await pickMultiSelect(page, 'task-form-allowed-tools', SERVER_ID)

  await byTestId(page, 'task-form-save').click()
  await expect(page.getByText('Unattended task')).toBeVisible({ timeout: 10000 })

  // The picked server persisted as a whole-server unattended grant.
  const grants = captured.body?.allowed_unattended_tools as
    | Array<{ server_id: string }>
    | undefined
  expect(grants).toEqual([{ server_id: SERVER_ID }])
})

// TEST-50 (ITEM-45 / DEC-21): "Open thread" is present + enabled for a fired
// prompt task (bound conversation), disabled before first fire, and ABSENT for a
// workflow task (which has no thread).
test('TEST-50: Open thread affordance reflects target kind + bound conversation', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  const BOUND = 'cccccccc-1111-1111-1111-1111111111cc'
  const firedPrompt = baseRow({
    id: '5a000000-0000-0000-0000-0000000000a1',
    name: 'Fired prompt',
    target_kind: 'prompt',
    bound_conversation_id: BOUND,
  })
  const neverFired = baseRow({
    id: '5a000000-0000-0000-0000-0000000000a2',
    name: 'Never fired',
    target_kind: 'prompt',
    bound_conversation_id: null,
  })
  const workflowTask = baseRow({
    id: '5a000000-0000-0000-0000-0000000000a3',
    name: 'Workflow task',
    target_kind: 'workflow',
    workflow_id: WORKFLOW_ID,
    prompt: null,
    bound_conversation_id: null,
  })

  await page.route(/\/api\/scheduled-tasks$/, async route =>
    route.fulfill({ status: 200, json: [firedPrompt, neverFired, workflowTask] }),
  )

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  // Fired prompt: enabled Open thread → navigates to the bound conversation.
  const openThread = byTestId(page, `task-open-thread-${firedPrompt.id}`)
  await expect(openThread).toBeEnabled({ timeout: 10000 })
  // Never-fired prompt: Open thread present but disabled.
  await expect(byTestId(page, `task-open-thread-${neverFired.id}`)).toBeDisabled()
  // Workflow task: no Open thread at all.
  await expect(page.getByTestId(`task-open-thread-${workflowTask.id}`)).toHaveCount(0)

  await openThread.click()
  await expect(page).toHaveURL(new RegExp(`/conversations/${BOUND}`), { timeout: 10000 })
})
