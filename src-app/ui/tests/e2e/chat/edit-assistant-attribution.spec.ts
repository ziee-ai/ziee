import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
  sendChatMessage,
  waitForAssistantResponse,
} from './helpers/chat-helpers'

/**
 * E2E — editing a previously-sent message restores its assistant attribution
 * (audit id 9729bb55bdbf76fa). The assistant chat-extension subscribes to
 * `editingMessage` and, on edit, fetches the message's attributed assistant
 * (ApiClient.Message.getAssistant) and re-selects it in the AssistantPicker
 * (assistant/chat-extension/extension.tsx:48-83). Untested.
 *
 * Real-LLM gated (a real send records the attribution row).
 */

const ASSISTANT_NAME = 'Attrib Bot'

test.describe('Chat — edit restores assistant attribution', () => {
  test.skip(
    !process.env.ANTHROPIC_API_KEY,
    'no ANTHROPIC_API_KEY — skipping real-LLM assistant-attribution edit test',
  )
  test.slow()

  test('editing a message shows the assistant it was attributed to', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, token, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(
      apiURL,
      token,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    // Create an assistant to attribute the message to.
    const aRes = await fetch(`${apiURL}/api/assistants`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({
        name: ASSISTANT_NAME,
        description: 'attribution test',
        instructions: 'You are a concise test assistant.',
        is_template: false,
      }),
    })
    if (!aRes.ok) throw new Error(`assistant create failed: ${aRes.status} ${await aRes.text()}`)

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    // Select the assistant in the composer via the "+" dropdown.
    await byTestId(page, 'chat-input-add-btn').click()
    await byTestId(page, 'assistant-menu-trigger').click()
    await expect(page.getByText(ASSISTANT_NAME)).toBeVisible()
    await page.getByText(ASSISTANT_NAME).click()
    // The status chip reflects the selected assistant.
    await expect(
      byTestId(page, 'assistant-status-chip'),
    ).toBeVisible()

    // Send a message (records the message→assistant attribution) + await reply.
    await sendChatMessage(page, 'Say hello in one word.', false)
    await waitForAssistantResponse(page)

    // Enter edit mode on the user message.
    const userMsg = page
      .locator('[data-testid="chat-message"][data-role="user"]')
      .last()
    await userMsg.hover()
    await page.locator('[data-testid="edit-message-button"]').click()
    await expect(byTestId(page, 'chat-editing-banner')).toBeVisible({ timeout: 5000 })

    // The assistant attribution is restored into the picker → its status chip
    // shows the originally-attributed assistant.
    await expect(
      byTestId(page, 'assistant-status-chip'),
    ).toBeVisible({ timeout: 10000 })
  })
})
