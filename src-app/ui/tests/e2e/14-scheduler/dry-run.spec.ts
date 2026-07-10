import { expect, test } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import { MODEL_ID, mockPickerEndpoints, pickSelect } from './helpers'

/**
 * E2E — Dry-run "Test" + change-detection toggle (ITEM-35 / ITEM-37): in the
 * create drawer the user clicks **Test**, the currently-edited (unsaved) config
 * is test-fired and the result renders inline WITHOUT the task being saved; the
 * "only when something changed" toggle is present and flippable. Mocks the
 * test-fire + list endpoints at the HTTP boundary.
 */

test('Test button runs a dry-run inline without saving; change-detection toggle flips', async ({
  page,
  testInfra,
}) => {
  const { baseURL } = testInfra
  let testFireCalls = 0
  let created = false

  await mockPickerEndpoints(page)
  await page.route(/\/api\/scheduled-tasks\/test-fire$/, async route => {
    testFireCalls += 1
    await route.fulfill({
      status: 200,
      json: { ok: true, text: 'Hello from the dry run.', error: null },
    })
  })
  await page.route(/\/api\/scheduled-tasks$/, async (route, req) => {
    if (req.method() === 'POST') {
      created = true
      await route.fulfill({ status: 201, json: {} })
      return
    }
    await route.fulfill({ status: 200, json: [] })
  })

  await loginAsAdmin(page, baseURL)
  await page.goto(`${baseURL}/scheduled-tasks`)

  await byTestId(page, 'scheduled-tasks-new').click()
  await byTestId(page, 'task-form-name').fill('Dry run me')
  await byTestId(page, 'task-form-prompt').fill('Say hi.')
  await pickSelect(page, 'task-form-model', MODEL_ID)

  // Flip the change-detection ("only when something changed") toggle.
  await byTestId(page, 'task-form-notify-on-change').click()

  // Click Test → the result renders inline.
  await byTestId(page, 'task-form-test').click()
  await expect(byTestId(page, 'task-form-test-result')).toContainText(
    'Hello from the dry run.',
    { timeout: 10000 },
  )

  // The dry-run fired but the task was NOT saved.
  expect(testFireCalls).toBe(1)
  expect(created).toBe(false)
})
