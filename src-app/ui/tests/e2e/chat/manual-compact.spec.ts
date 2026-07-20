import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getCurrentUserToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import { createBridgeToolModel, HAS_BRIDGE, BRIDGE_SKIP } from './helpers/agent-llm-helpers'
import { byTestId } from '../testid'

/**
 * TEST-237 / ITEM-61 — manual `/compact` (DEC-137).
 *
 * The composer compact affordance triggers a REAL context compaction
 * (`POST /conversations/{id}/compact`); the backend regenerates the rolling summary
 * and emits a `historyReplaced` SSE frame on the persistent per-user chat stream, so
 * the timeline renders a "context compacted" marker in place — and the conversation
 * stays fully usable afterward (a second turn still works). Outbound-only: the stored
 * messages are never rewritten.
 *
 * Requires the agent-core chat path + a real LLM bridge (a real turn establishes the
 * conversation + its live SSE subscription). Skips cleanly when the bridge is unset.
 * --workers=1.
 */
test.describe('chat manual compaction — /compact affordance (ITEM-61)', () => {
  test.skip(!HAS_BRIDGE, BRIDGE_SKIP)
  test.setTimeout(300_000)

  test('the compact button compacts the conversation → a "context compacted" marker renders and the chat stays usable', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getCurrentUserToken(page)

    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createBridgeToolModel(page, apiURL, token, providerId, 'Compact Model')

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Compact Model')

    // One real turn → the conversation now has history AND a live per-user SSE
    // subscription scoped to it (so a between-turns `historyReplaced` frame reaches
    // this client).
    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await textarea.fill('Say hello in one short sentence.')
    await page.getByRole('button', { name: 'Send message' }).click()

    // The assistant reply lands (≥2 message bubbles: user + assistant).
    await expect
      .poll(async () => page.locator('[data-testid="chat-message"]').count(), {
        timeout: 180_000,
      })
      .toBeGreaterThanOrEqual(2)

    // Trigger the manual compaction from the composer affordance.
    const compactBtn = byTestId(page, 'chat-compact-button')
    await expect(compactBtn).toBeVisible({ timeout: 15_000 })
    await expect(compactBtn).toBeEnabled({ timeout: 30_000 })
    await compactBtn.click()

    // The timeline shows the "context compacted" marker (delivered live over the
    // persistent chat SSE stream; ITEM-61).
    await expect(byTestId(page, 'chat-history-replaced-marker')).toBeVisible({
      timeout: 60_000,
    })

    // The conversation stays usable — a second turn still works after compaction.
    await textarea.fill('Now say goodbye in one short sentence.')
    await page.getByRole('button', { name: 'Send message' }).click()
    await expect
      .poll(async () => page.locator('[data-testid="chat-message"]').count(), {
        timeout: 180_000,
      })
      .toBeGreaterThanOrEqual(4)
  })
})
