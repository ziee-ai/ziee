import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — open-split + per-pane INPUT isolation (TEST-14 / TEST-17).
 *
 * No LLM needed: conversations are created via the API, the split is opened via
 * the header "Split" affordance, and we assert two panes render side-by-side and
 * that each pane's composer holds its OWN draft text (per-pane TextStore — the
 * core of composer isolation). Selectors are scoped under the pane wrappers
 * (`chat-pane-0` / `chat-pane-1`) because a >=2-pane split renders the same
 * per-pane data-testids more than once.
 */
test.describe('Split chat — open-split + input isolation', () => {
  test('opening split shows two independent panes; typing in one does not appear in the other', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // A provider + model so the composer isn't in the "models unavailable" state.
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    // Two empty conversations (no LLM turn) — pane 0 opens conv A.
    const mkConv = async (title: string): Promise<string> => {
      const res = await page.request.post(`${apiURL}/api/conversations`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { title },
      })
      expect(res.status()).toBeLessThan(300)
      return (await res.json()).id as string
    }
    const convA = await mkConv('Split Input A')

    // Land in conversation A (single-pane), then open the split.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    const splitBtn = byTestId(page, 'chat-split-btn')
    await expect(splitBtn).toBeVisible({ timeout: 30000 })
    await splitBtn.click()

    // Two panes render side-by-side (TEST-14).
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0).toBeVisible({ timeout: 15000 })
    await expect(pane1).toBeVisible({ timeout: 15000 })
    // A resize divider sits between them.
    await expect(byTestId(page, 'split-divider-0')).toBeVisible()
    // pane 1 opens the conversation PICKER (ITEM-27); "Start a new chat" reaches
    // the new-chat composer so pane 1 has its own textarea to type into.
    await pane1.getByTestId('pane-start-new-chat').click()
    await expect(pane1.getByTestId('pane-new-chat-greeting')).toBeVisible()

    // Each pane has its own composer textarea.
    const inputA = pane0.locator('textarea[placeholder*="Type your message"]')
    const inputB = pane1.locator('textarea[placeholder*="Type your message"]')
    await expect(inputA).toBeVisible({ timeout: 15000 })
    await expect(inputB).toBeVisible({ timeout: 15000 })

    // Type distinct drafts into each pane (TEST-17: per-pane TextStore).
    await inputA.fill('draft-for-pane-A')
    await inputB.fill('draft-for-pane-B')

    // Isolation: neither draft bleeds into the other pane.
    await expect(inputA).toHaveValue('draft-for-pane-A')
    await expect(inputB).toHaveValue('draft-for-pane-B')

    // Editing one does not disturb the other.
    await inputA.fill('draft-for-pane-A-edited')
    await expect(inputA).toHaveValue('draft-for-pane-A-edited')
    await expect(inputB).toHaveValue('draft-for-pane-B')
  })

  test('single-pane regression: without splitting, the conversation renders normally', async ({
    page,
    testInfra,
  }) => {
    // TEST-26 (bridge single-pane regression): with the split feature present but
    // only ONE pane, the legacy single-conversation surface is unchanged (no
    // chat-pane wrapper, the normal composer works).
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Single Pane Regression' },
    })
    expect(res.status()).toBeLessThan(300)
    const convId = (await res.json()).id as string

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForLoadState('load')

    // The single-pane surface: composer present, NO split-pane wrapper.
    const input = page.locator('textarea[placeholder*="Type your message"]')
    await expect(input).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'chat-pane-0')).toHaveCount(0)
    await input.fill('single pane draft')
    await expect(input).toHaveValue('single pane draft')
  })
})
