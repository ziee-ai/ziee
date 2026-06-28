import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from '../09-chat/helpers/chat-helpers'

/**
 * E2E (real-LLM) — selecting an assistant in the composer makes its
 * instructions reach the model.
 *
 * Audit gap: assistant settings + selection are tested, but no E2E sent a real
 * chat message with a selected assistant and verified the instructions were
 * applied. Mirrors 11-projects/message-uses-project-context (project-instruction
 * version) but for the ASSISTANT injection path. Soft-skips without ANTHROPIC_API_KEY.
 */

const HAS_ANTHROPIC = (process.env.ANTHROPIC_API_KEY ?? '').length > 0
const BEACON = 'ZZZ_ASSISTANT_BEACON_42'

test.describe('Assistants — instructions applied in chat (real LLM)', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — real-LLM E2E skipped')
  test.slow()

  test('a selected assistant\'s instructions reach the model', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, adminToken, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    // An assistant whose instructions force a deterministic beacon token.
    const assistantName = `Beacon Assistant ${Date.now().toString(36)}`
    const created = await fetch(`${apiURL}/api/assistants`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${adminToken}` },
      body: JSON.stringify({
        name: assistantName,
        instructions: `You are required to begin every response with the exact literal string '${BEACON}' (no preface). After that token you can respond normally.`,
      }),
    })
    expect(created.status).toBeLessThan(300)

    await goToNewChatPage(page, baseURL)

    // Select the assistant via the "+" dropdown → "Select assistant".
    await page.getByRole('button', { name: 'Add attachment' }).click()
    await page.getByText('Select assistant').click()
    await expect(page.getByText(assistantName)).toBeVisible({ timeout: 10000 })
    await page.getByText(assistantName).click()

    // Send a message → the streamed response must carry the beacon, proving the
    // assistant's instructions were injected into the system prompt.
    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await textarea.fill('Say hello.')
    const send = page.getByRole('button', { name: 'Send message' })
    await expect(send).toBeEnabled({ timeout: 10000 })
    await send.click()

    await expect(page.locator('body')).toContainText(BEACON, { timeout: 60000 })
  })
})
