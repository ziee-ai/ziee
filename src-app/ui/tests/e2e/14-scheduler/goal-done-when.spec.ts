import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { createBridgeToolModel, HAS_BRIDGE, BRIDGE_SKIP } from '../chat/helpers/agent-llm-helpers'
import {
  openScheduleDialog,
  switchSegment,
  pickSelectValue,
  getTasksForConversation,
} from './chat-schedule-helpers'

/**
 * TEST-123 / ITEM-24 — goal-seeking "done when…": the in-chat loop dialog exposes a
 * natural-language completion condition, and a real goal-seeking run marks the task
 * COMPLETE only after an independent evaluator judges that condition met.
 *
 * Flow (real backend + real bridge): open the composer loop dialog → switch to Loop
 * mode → the "Stop when…" (completion_condition) field is present → fill a crisp
 * condition + a prompt whose reply satisfies it → submit → `run-now` fires ONE
 * self-paced turn, whose result the isolated evaluator judges against the condition.
 * Because the reply meets the condition, the goal loop SELF-STOPS `completed`
 * (`paused_reason='completed'`), which surfaces as the "Completed" badge + a run row
 * on the /scheduled-tasks timeline.
 *
 * The evaluator is deliberately biased toward `not_done` (never a false `done`), so
 * the condition is written to be unambiguously satisfied by the reply. Requires the
 * agent-core path + a real LLM bridge; skips cleanly when unset. --workers=1.
 */
test.describe('scheduler goal-seeking — done-when completion (ITEM-24)', () => {
  test.skip(!HAS_BRIDGE, BRIDGE_SKIP)
  test.setTimeout(300_000)

  test('the loop dialog exposes "done when…" and a real run completes only once the condition is met', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    // A tool-capable bridge model + a real owned conversation to bind the loop to.
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    const modelId = await createBridgeToolModel(page, apiURL, token, providerId, 'Goal Loop Model')

    const convRes = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Goal-seeking loop conversation', model_id: modelId },
    })
    expect(convRes.ok(), `seed conversation: ${convRes.status()} ${await convRes.text()}`).toBeTruthy()
    const conversationId = (await convRes.json()).id as string

    // Open the in-chat schedule/loop dialog and switch to LOOP mode — the
    // "Stop when…" completion field is exposed ONLY for a self-paced loop.
    await openScheduleDialog(page, baseURL, conversationId)
    await switchSegment(page, 'schedule-loop-mode-opt-loop')

    // ITEM-24 UI — the dialog exposes the natural-language "done when…" field.
    const doneWhen = byTestId(page, 'schedule-loop-completion')
    await expect(doneWhen).toBeVisible({ timeout: 15_000 })

    // A crisp condition unambiguously satisfied by the reply (so the not_done-biased
    // evaluator still judges `done`).
    await byTestId(page, 'schedule-loop-prompt').fill(
      'Reply with exactly the single word FINISHED and nothing else.',
    )
    await doneWhen.fill('the reply contains the word FINISHED')
    await pickSelectValue(page, 'task-form-model', modelId)

    await byTestId(page, 'schedule-loop-submit').click()
    // The dialog closes on a successful create.
    await expect(byTestId(page, 'schedule-loop-form')).toBeHidden({ timeout: 15_000 })

    // Resolve the created goal-seeking task (owner-scoped, bound to this chat).
    const tasks = await getTasksForConversation(page, apiURL, token, conversationId)
    expect(tasks.length, 'a goal-seeking task was created').toBeGreaterThan(0)
    const task = tasks.find(t => (t.completion_condition as string | null)?.includes('FINISHED')) ?? tasks[0]
    const taskId = task.id as string
    expect(task.completion_condition, 'the task carries the done-when condition').toContain('FINISHED')

    // Fire ONE self-paced turn synchronously; the evaluator then judges the result.
    const runRes = await page.request.post(`${apiURL}/api/scheduled-tasks/${taskId}/run-now`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    expect(runRes.ok(), `run-now: ${runRes.status()} ${await runRes.text()}`).toBeTruthy()

    // The condition is met → the goal loop SELF-STOPS completed. Poll the real task
    // (two bridge calls — the turn + the evaluator — can take a while under load).
    await expect
      .poll(
        async () => {
          const res = await page.request.get(`${apiURL}/api/scheduled-tasks/${taskId}`, {
            headers: { Authorization: `Bearer ${token}` },
          })
          if (!res.ok()) return `http_${res.status()}`
          return (await res.json()).paused_reason as string | null
        },
        {
          timeout: 240_000,
          intervals: [2_000, 5_000, 10_000],
          message: 'goal-seeking task should self-stop "completed" once the condition is met',
        },
      )
      .toBe('completed')

    // The completion + its run surface on the /scheduled-tasks timeline (no reload).
    await page.goto(`${baseURL}/scheduled-tasks`)
    await expect(byTestId(page, `task-completed-${taskId}`)).toBeVisible({ timeout: 30_000 })
    // At least one run row is present in the task's run timeline.
    await expect(page.locator('[data-testid^="run-row-"]').first()).toBeVisible({ timeout: 30_000 })
  })
})
