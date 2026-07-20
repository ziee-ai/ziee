import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  getTasksForConversation,
  openScheduleDialog,
  pickSelectValue,
  seedModelAndConversation,
} from './chat-schedule-helpers'

/**
 * TEST-92 (ITEM-23) — one source of truth.
 *
 * asserts (TESTS.md): an in-chat task ALSO appears on the standalone Scheduled
 * Tasks page; edits reflect on both.
 *
 * The in-chat dialog reuses the SAME `ScheduledTasks` store + `create_task`
 * endpoint as the standalone page (DEC-19 "no forked schedule store"), so a task
 * created in chat is the identical persisted row the standalone page lists — not a
 * parallel copy. This proves that: create in-chat → the row is on the standalone
 * page → edit it there → the edit persists (same row, same backend).
 */
test.describe('In-chat + standalone are one source of truth (ITEM-23)', () => {
  test('a task created in-chat appears on the standalone page and edits persist', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const seed = await seedModelAndConversation(page, apiURL)
    await openScheduleDialog(page, baseURL, seed.conversationId)

    const originalName = `Source-of-truth ${Date.now().toString(36)}`
    await byTestId(page, 'schedule-loop-prompt').fill('Summarise the week.')
    await byTestId(page, 'schedule-loop-name').fill(originalName)
    await pickSelectValue(page, 'task-form-model', seed.modelId)

    const [resp] = await Promise.all([
      page.waitForResponse(
        r =>
          /\/api\/scheduled-tasks$/.test(r.url()) &&
          r.request().method() === 'POST',
        { timeout: 20000 },
      ),
      byTestId(page, 'schedule-loop-submit').click(),
    ])
    expect(resp.ok()).toBeTruthy()

    const tasks = await getTasksForConversation(
      page,
      apiURL,
      seed.adminToken,
      seed.conversationId,
    )
    expect(tasks.length).toBe(1)
    const taskId = tasks[0].id as string

    // Same row surfaces on the standalone Scheduled Tasks page (not a fork).
    await page.goto(`${baseURL}/scheduled-tasks`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, `task-card-${taskId}`)).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, `task-name-${taskId}`)).toHaveText(originalName)

    // Edit it on the standalone page (rename via the edit drawer) — the change is
    // written to the SAME backend row.
    await byTestId(page, `task-card-${taskId}`).hover()
    await byTestId(page, `task-edit-${taskId}`).click()
    const nameInput = byTestId(page, 'task-form-name')
    await expect(nameInput).toBeVisible({ timeout: 15000 })
    const renamed = `${originalName} (edited)`
    await nameInput.fill(renamed)

    const [saveResp] = await Promise.all([
      page.waitForResponse(
        r =>
          /\/api\/scheduled-tasks\//.test(r.url()) &&
          r.request().method() === 'PUT',
        { timeout: 20000 },
      ),
      byTestId(page, 'task-form-save').click(),
    ])
    expect(saveResp.ok()).toBeTruthy()

    // The rename is the source of truth: it shows on the list AND survives a reload
    // (persisted to the one backing row the in-chat create produced).
    await expect(byTestId(page, `task-name-${taskId}`)).toHaveText(renamed, {
      timeout: 15000,
    })
    await page.goto(`${baseURL}/scheduled-tasks`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, `task-name-${taskId}`)).toHaveText(renamed, {
      timeout: 15000,
    })

    // And the edit is visible through the same owner-scoped API the in-chat side used.
    const after = await getTasksForConversation(
      page,
      apiURL,
      seed.adminToken,
      seed.conversationId,
    )
    expect(after[0].name).toBe(renamed)
  })
})
