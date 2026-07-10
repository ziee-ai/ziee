import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — pop-out into a new tab (TEST-P3 / ITEM-P2/P3).
 *
 * The header "Open in new window/tab" action opens the conversation in a second
 * top-level page (web: `window.open('/chat/<id>', '_blank')`). The new page
 * shares the session cookie, so it authenticates and renders the same
 * conversation independently. No LLM needed.
 */
test.describe('Split chat — pop-out to a new tab', () => {
  test('the pop-out action opens the conversation in a second, independent page', async ({
    page,
    context,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Pop Out Me' },
    })
    const convId = (await res.json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForLoadState('load')

    const popoutBtn = byTestId(page, 'chat-open-in-new-window')
    await expect(popoutBtn).toBeVisible({ timeout: 30000 })

    // Clicking it opens a NEW top-level page for the same conversation.
    const [popup] = await Promise.all([
      context.waitForEvent('page'),
      popoutBtn.click(),
    ])
    await popup.waitForLoadState('load')

    // The new page is the same conversation, authenticated + rendered
    // independently (its own composer).
    expect(popup.url()).toContain(`/chat/${convId}`)
    await expect(
      popup.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 30000 })

    // The original page is untouched and still usable.
    await expect(
      page.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible()
    await popup.close()
  })
})
