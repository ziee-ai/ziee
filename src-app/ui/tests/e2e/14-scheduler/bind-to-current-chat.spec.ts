import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  getTasksForConversation,
  openScheduleDialog,
  pickSelectValue,
  seedModelAndConversation,
  switchSegment,
} from './chat-schedule-helpers'

/**
 * TEST-90 (ITEM-22) — scheduling from inside a conversation BINDS to it.
 *
 * asserts (TESTS.md): scheduling in-conv binds; run result lands same chat;
 * attached list pause/edit/delete.
 *
 * The pure-UI, no-LLM half proven here: a loop created via the in-chat dialog
 * carries `bound_conversation_id = <this conversation>` (verified against the real
 * backend), and that bound task then exposes its pause / edit / delete affordances
 * + a link back to its conversation on the standalone list. ("run result lands
 * same chat" requires a real firing turn — LLM — and is reported separately.)
 */
test.describe('Schedule-from-chat binds to the conversation (ITEM-22)', () => {
  test('a loop created in-chat binds to the conversation and is manageable (pause/edit/delete)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const seed = await seedModelAndConversation(page, apiURL)
    await openScheduleDialog(page, baseURL, seed.conversationId)

    // Create a LOOP (self-paced) task — a bound loop surfaces the "Open its
    // conversation" link on the standalone card (self_paced + bound_conversation_id).
    const taskName = `Bound loop ${Date.now().toString(36)}`
    // Switch to Loop mode (base-ui Tabs hit-test needs the hover+force sequence;
    // the assertions still prove the switch to loop / self_paced).
    await switchSegment(page, 'schedule-loop-mode-opt-loop')
    await byTestId(page, 'schedule-loop-prompt').fill(
      'Keep checking the sequencing run and summarise progress',
    )
    await byTestId(page, 'schedule-loop-name').fill(taskName)
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

    // Bind is real: the task is bound to THIS conversation and is self-paced.
    const tasks = await getTasksForConversation(
      page,
      apiURL,
      seed.adminToken,
      seed.conversationId,
    )
    expect(tasks.length).toBe(1)
    const task = tasks[0]
    expect(task.bound_conversation_id).toBe(seed.conversationId)
    expect(task.schedule_kind).toBe('self_paced')
    const taskId = task.id as string

    // On the standalone list the bound loop shows its loop tag, a link to its
    // conversation, and the pause / edit / delete affordances (the "attached list
    // pause/edit/delete" half of the assert).
    await page.goto(`${baseURL}/scheduled-tasks`)
    await page.waitForLoadState('load')

    const card = byTestId(page, `task-card-${taskId}`)
    await expect(card).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, `task-name-${taskId}`)).toHaveText(taskName)
    await expect(byTestId(page, `task-loop-${taskId}`)).toBeVisible()
    await expect(byTestId(page, `task-bound-conversation-${taskId}`)).toBeVisible()

    // Hover reveals the action group (opacity toggle); the controls are present.
    await card.hover()
    await expect(byTestId(page, `task-edit-${taskId}`)).toBeVisible()
    await expect(byTestId(page, `task-delete-${taskId}`)).toBeVisible()

    // Pause via the enable Switch → it flips to unchecked, and the change persists
    // through the real backend (reload shows it still paused).
    const enableSwitch = byTestId(page, `task-enabled-${taskId}`)
    await expect(enableSwitch).toHaveAttribute('aria-checked', 'true')
    await enableSwitch.click()
    await expect(enableSwitch).toHaveAttribute('aria-checked', 'false', {
      timeout: 10000,
    })

    await page.goto(`${baseURL}/scheduled-tasks`)
    await page.waitForLoadState('load')
    await expect(byTestId(page, `task-enabled-${taskId}`)).toHaveAttribute(
      'aria-checked',
      'false',
      { timeout: 15000 },
    )
  })
})
