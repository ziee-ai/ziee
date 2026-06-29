import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

/**
 * Assistant deletion error-recovery in the chat surface. The assistant
 * chat-extension soft-fails when a selected assistant is gone
 * (AssistantStatusChip returns null when the id isn't in availableAssistants;
 * the edit-attribution hook clears on a failed lookup). This selects an
 * assistant (chip shows), deletes it via the API, and asserts the chat
 * surface recovers — the deleted assistant is absent from the picker and the
 * composer remains functional (no error boundary).
 */
test.describe('Chat - assistant deletion recovery', () => {
  test('deleting a selected assistant leaves the chat functional + picker clean', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')

    const assistantName = `Doomed Assistant ${Date.now()}`
    const createResp = await page.request.post(`${apiURL}/api/assistants`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        name: assistantName,
        description: 'deletion-recovery e2e',
        instructions: 'You are a test assistant.',
        is_template: false,
      },
    })
    expect(createResp.ok()).toBe(true)
    const assistantId: string = (await createResp.json()).id

    await goToNewChatPage(page, baseURL)

    // Select the assistant → its chip appears in the toolbar status row.
    await page.getByRole('button', { name: 'Add attachment' }).click()
    await page.getByText('Select assistant').click()
    await expect(page.getByText(assistantName)).toBeVisible()
    await page.getByText(assistantName).click()
    await expect(
      page.locator('.ant-tag').filter({ hasText: assistantName }),
    ).toBeVisible()

    // Delete the assistant out from under the chat.
    const del = await page.request.delete(
      `${apiURL}/api/assistants/${assistantId}`,
      { headers: { Authorization: `Bearer ${adminToken}` } },
    )
    expect(del.ok()).toBe(true)

    // Reload the chat surface — it recovers cleanly.
    await page.reload()
    await goToNewChatPage(page, baseURL)

    // The composer is still functional (no crash / error boundary).
    await expect(
      page.getByRole('button', { name: 'Send message' }),
    ).toBeVisible({ timeout: 30000 })

    // The deleted assistant is gone from the picker (no stale reference).
    await page.getByRole('button', { name: 'Add attachment' }).click()
    await page.getByText('Select assistant').click()
    await expect(page.getByText(assistantName)).toHaveCount(0)
  })
})
