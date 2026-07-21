import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { byTestId } from '../testid'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import { createBridgeToolModel, HAS_BRIDGE, BRIDGE_SKIP } from './helpers/agent-llm-helpers'

/**
 * TEST-19 / ITEM-8 — a model-launched background sub-agent is NON-BLOCKING: the
 * chat composer stays usable while the detached run proceeds, and the run's status
 * surfaces on the background-tasks page.
 *
 * The built-in `background_mcp` server is always-on for tool-capable models, so the
 * agent-core chat path offers `spawn_background`. `spawn_background` LAUNCHES a
 * detached agent, so it is forced through the manual approval gate even under
 * auto-approve (the security posture). We prompt the model to spawn a trivial
 * background sub-agent → the approval card surfaces → approve → the fire-and-forget
 * run is launched WITHOUT blocking the foreground chat (composer immediately
 * usable) → its status surfaces as a Sub-agent run card on `/background-tasks`.
 *
 * Requires the agent-core chat path (ZIEE_CHAT_AGENT_CORE=1) + a real LLM bridge.
 * Skips cleanly when the bridge env is unset.
 */
test.describe('background sub-agent — non-blocking status (real LLM, agent-core)', () => {
  test.skip(!HAS_BRIDGE, BRIDGE_SKIP)
  test.setTimeout(240_000)

  test('model spawns a background sub-agent → approval → composer stays usable → status surfaces', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createBridgeToolModel(page, apiURL, token, providerId, 'Background Status Model')

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Background Status Model')

    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await textarea.fill(
      'Use the `spawn_background` tool to launch a DETACHED background sub-agent so we can keep ' +
        'chatting while it runs. Call `spawn_background` NOW with kind "subagent" and a `spec` ' +
        'whose `task` is "Reply with the single word DONE and nothing else.". You MUST call the ' +
        'spawn_background tool — do not do the work inline.',
    )
    await page.getByRole('button', { name: 'Send message' }).click()

    // spawn_background is a WRITE → forced through the manual approval gate. The
    // model chose to call it → the approval card surfaces.
    const approveBtn = page.locator('[data-testid="tool-approval-approve-once"]').first()
    await expect(approveBtn).toBeVisible({ timeout: 210_000 })
    await approveBtn.click()

    // Non-blocking: the detached run does NOT freeze the chat — the composer is
    // immediately usable again (we can type a follow-up while the sub-agent runs).
    await expect(textarea).toBeEditable({ timeout: 60_000 })
    await textarea.fill('Meanwhile, what is 2 + 2?')
    await expect(textarea).toHaveValue('Meanwhile, what is 2 + 2?')
    await textarea.fill('')

    // The launched run's status surfaces on the background-tasks page as a
    // Sub-agent run card (fetched from the real GET /api/background/runs).
    await page.goto(`${baseURL}/background-tasks`)
    await expect(byTestId(page, 'background-tasks-page')).toBeVisible({ timeout: 30_000 })
    const card = page.locator('[data-testid^="background-run-card-"]').first()
    await expect(card).toBeVisible({ timeout: 30_000 })
    await expect(
      page.locator('[data-testid^="background-run-kind-"]').first(),
    ).toHaveText('Sub-agent')
  })
})
