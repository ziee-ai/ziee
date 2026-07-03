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
  waitForAssistantResponse,
} from './helpers/chat-helpers'

/**
 * Editing a previously-sent message restores the assistant it was originally
 * attributed to (assistant chat-extension: on `editingMessage`, fetch
 * Message.getAssistant + re-select it in the picker). Flow: send with Assistant
 * A → switch the picker to B → edit the user message → the picker snaps BACK to
 * A. No E2E covered this restoration before. Uses the same OpenAI setup as the
 * branching specs.
 */

async function hoverAndClickAction(
  page: import('@playwright/test').Page,
  messageLocator: import('@playwright/test').Locator,
  buttonTestId: string,
) {
  await messageLocator.hover()
  await page.locator(`[data-testid="${buttonTestId}"]`).click()
}

test.describe('Chat — edit restores assistant attribution', () => {
  test('editing a sent message re-selects its original assistant', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    // Two distinct assistants.
    const ts = Date.now()
    const nameA = `EditAssistantA ${ts}`
    const nameB = `EditAssistantB ${ts}`
    const idByName: Record<string, string> = {}
    for (const name of [nameA, nameB]) {
      const r = await page.request.post(`${apiURL}/api/assistants`, {
        headers: { Authorization: `Bearer ${adminToken}`, 'Content-Type': 'application/json' },
        data: { name, instructions: `You are ${name}.`, enabled: true },
      })
      expect(r.ok()).toBeTruthy()
      idByName[name] = (await r.json()).id as string
    }

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')

    // The picker is now the "+" composer dropdown → assistant submenu; the
    // current selection surfaces as the `assistant-status-chip` Tag.
    const selectAssistant = async (id: string) => {
      await byTestId(page, 'chat-input-add-btn').click()
      await byTestId(page, 'assistant-menu-trigger').click()
      await byTestId(page, `assistant-option-${id}`).click()
    }
    const chip = byTestId(page, 'assistant-status-chip')

    // Select Assistant A, then send a message → the message is attributed to A.
    await selectAssistant(idByName[nameA])
    await expect(chip).toContainText(nameA)
    await byTestId(page, 'chat-message-textarea').fill('Hello there')
    await byTestId(page, 'chat-input-send-btn').click()
    await waitForAssistantResponse(page)

    // Switch the picker to Assistant B (so the current selection differs from
    // the sent message's attribution).
    await selectAssistant(idByName[nameB])
    await expect(chip).toContainText(nameB)

    // Edit the original user message → the picker restores Assistant A →
    // the status chip snaps back to A.
    const userMsg = page
      .locator('[data-testid="chat-message"][data-role="user"]')
      .first()
    await hoverAndClickAction(page, userMsg, 'edit-message-button')

    await expect(chip).toContainText(nameA, { timeout: 10000 })
  })
})
