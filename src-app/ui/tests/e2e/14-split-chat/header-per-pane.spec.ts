import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — per-pane header / chrome (TEST-57, ITEM-37). Each pane owns its
 * own title editor, find toggle, and new-chat composer selection key
 * (`newChatModelKey(paneId)`), so:
 *  - a title-edit Save in one pane updates ONLY that pane's title;
 *  - the find toggle opens the find bar in the pane it was clicked in only;
 *  - the new-chat pane's model selection is independent of the other pane's.
 * The two-simultaneous-new-chat-panes sentinel collision is exercised via the
 * shipped existing+new-chat pane pair (the paneId-suffixed key; DRIFT-1.2/2.9);
 * the pure key logic is unit-proven. No LLM.
 */
test.describe('Split chat — per-pane header/chrome', () => {
  test('title-edit, find toggle, and new-chat model selection are each pane-scoped', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    const alphaId = await createModelViaAPI(apiURL, token, providerId, 'model-alpha', 'Model Alpha', 'openai')
    const betaId = await createModelViaAPI(apiURL, token, providerId, 'model-beta', 'Model Beta', 'openai')
    void alphaId

    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Header Pane A' },
    })
    const convA = (await res.json()).id as string

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    // [A | new-chat]: split, then start a new chat in pane 1.
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId('pane-start-new-chat').click()
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()

    // --- Title-edit is pane-scoped (focus pane 0 first; TitleEditor uses the
    // Stores.Chat bridge which resolves to the focused pane). ---
    await pane0.click()
    await pane0.getByTestId('chat-title-edit-btn').click()
    const titleInput = pane0.getByTestId('chat-title-input')
    await titleInput.fill('Header Pane A — Renamed')
    await pane0.getByTestId('chat-title-save-btn').click()
    await expect(pane0.getByTestId('conversation-title')).toContainText('Renamed', {
      timeout: 15000,
    })
    // Pane 1 is a new chat — it has no conversation title editor at all.
    await expect(pane1.getByTestId('conversation-title')).toHaveCount(0)

    // --- Find toggle is pane-scoped. ---
    await pane0.getByTestId('conversation-find-toggle-btn').click()
    await expect(pane0.getByTestId('conversation-find-bar')).toBeVisible()
    await expect(pane1.getByTestId('conversation-find-bar')).toHaveCount(0)

    // --- New-chat pane model selection is independent (paneId-suffixed key). ---
    await pane1.getByTestId('ullm-model-select').click()
    await page.locator(`[data-testid="ullm-model-select-opt-${betaId}"]`).click()
    await expect(pane1.getByTestId('model-selector')).toContainText('Beta')
    await expect(pane0.getByTestId('model-selector')).not.toContainText('Beta')
  })
})
