import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import { createBridgeToolModel, HAS_BRIDGE, BRIDGE_SKIP } from './helpers/agent-llm-helpers'

/**
 * TEST-98 / ITEM-36 — the agent's LIVE task list renders inline in the assistant
 * turn as an evolving checklist, driven by the REAL agent-core chat loop + a real
 * tool-capable model (no mocks).
 *
 * The model is asked to use its `task_create` / `task_update` self-management
 * tools to build a 3-step plan and mark the first step in_progress. Each task
 * mutation emits an `AgentEvent::TaskListChanged` → `taskListChanged` chat SSE
 * frame → the `task-list` chat-extension keys the full snapshot to the in-flight
 * assistant message → the `TaskListChecklist` footer (testid `agent-task-list`)
 * re-renders in place. We assert the live checklist appears with ≥3 items and one
 * item in the `in_progress` state (present-continuous `active_form`).
 *
 * Requires the agent-core chat path (ZIEE_CHAT_AGENT_CORE=1) + a real LLM bridge
 * (OPENAI_BASE_URL + OPENAI_API_KEY + ZIEE_TEST_LLM_MODEL). Skips cleanly when the
 * bridge env is unset.
 */
test.describe('agent task-list — live checklist (real LLM, agent-core)', () => {
  test.skip(!HAS_BRIDGE, BRIDGE_SKIP)
  test.setTimeout(180_000)

  test('a tool-capable model builds a live task list → checklist renders with an in_progress step', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createBridgeToolModel(page, apiURL, token, providerId, 'Task Agent Model')

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Task Agent Model')

    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await textarea.fill(
      'Use your task-management tools to plan a 3-step task. Call task_create THREE times to add ' +
        'exactly these three steps (pass both content and active_form for each): ' +
        '(1) content "Gather ingredients", active_form "Gathering ingredients"; ' +
        '(2) content "Mix the batter", active_form "Mixing the batter"; ' +
        '(3) content "Bake the cake", active_form "Baking the cake". ' +
        'Then call task_update to set the FIRST step (Gather ingredients) to status "in_progress". ' +
        'You MUST call the task tools — do not just describe the plan.',
    )
    await page.getByRole('button', { name: 'Send message' }).click()

    // The live task-list checklist mounts inline in the assistant turn.
    const taskList = page.locator('[data-testid="agent-task-list"]').first()
    await expect(taskList).toBeVisible({ timeout: 150_000 })

    // ≥3 items land (the full-snapshot frame carries the whole list).
    await expect
      .poll(async () => taskList.locator('[data-testid^="agent-task-list-item-"]').count(), {
        timeout: 60_000,
      })
      .toBeGreaterThanOrEqual(3)

    // One step is live/in_progress — proves the checklist tracks per-item status
    // over SSE, not a static render.
    await expect(
      taskList.locator('[data-testid^="agent-task-list-item-"][data-status="in_progress"]').first(),
    ).toBeVisible({ timeout: 60_000 })

    // The in_progress step shows its present-continuous active_form ("…ing").
    await expect(
      taskList.locator('[data-testid^="agent-task-list-item-"][data-status="in_progress"]').first(),
    ).toContainText(/ing/i)
  })
})
