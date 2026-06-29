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
 * E2E — editing a previously-sent message restores that message's ORIGINALLY
 * attributed assistant in the picker, and cancelling restores the pre-edit
 * selection (assistant/chat-extension/extension.tsx:48-83). Untested.
 *
 * Flow: select assistant A → send (message attributed to A) → switch to B →
 * Edit the message (picker should restore to A) → Cancel (picker restores B).
 * Real-LLM gated (sending needs a model to produce the attributed turn).
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''

const CHIP = '.ant-tag-purple'

test.describe('Chat — edit-message assistant attribution restoration', () => {
  test.skip(ANTHROPIC_KEY.length === 0, 'ANTHROPIC_API_KEY not set — real-LLM edit-attribution skipped')

  test('editing restores the message assistant; cancel restores the pre-edit one', async ({
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

    // Two distinct assistants the picker can attribute messages to.
    const nameA = `EditAttrA_${Date.now()}`
    const nameB = `EditAttrB_${Date.now()}`
    for (const name of [nameA, nameB]) {
      const res = await fetch(`${apiURL}/api/assistants`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ name, instructions: 'Be brief.', is_default: false }),
      })
      expect(res.ok).toBeTruthy()
    }

    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')

    // Select assistant A in the composer's assistant picker.
    const picker = page.getByRole('combobox', { name: 'Select Assistant' })
    await picker.click()
    await page.getByRole('option', { name: nameA, exact: true }).click()
    await expect(page.locator(CHIP)).toContainText(nameA)

    // Send a message → it is attributed to A; wait for the reply.
    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await textarea.fill('Say hi in one word.')
    await page.getByRole('button', { name: 'Send message' }).click()
    await waitForAssistantResponse(page)

    // Switch the picker to assistant B (the "pre-edit" selection).
    await picker.click()
    await page.getByRole('option', { name: nameB, exact: true }).click()
    await expect(page.locator(CHIP)).toContainText(nameB)

    // Edit the sent user message → the extension restores A's attribution.
    const userMsg = page.locator('[data-testid="chat-message"][data-role="user"]').first()
    await userMsg.hover()
    await userMsg.getByTestId('edit-message-button').click()
    await expect(page.locator(CHIP)).toContainText(nameA, { timeout: 15000 })

    // Cancel the edit → the pre-edit selection (B) is restored, not cleared.
    await page.getByRole('button', { name: 'Cancel edit' }).click()
    await expect(page.locator(CHIP)).toContainText(nameB, { timeout: 15000 })
  })
})
