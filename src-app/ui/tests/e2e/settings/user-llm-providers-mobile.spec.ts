import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — mobile responsive rendering of the user LLM providers page. On a small
 * viewport the page swaps the desktop provider SIDEBAR for a Dropdown picker
 * (UserLlmProvidersPage.tsx:220-246). No E2E exercised the mobile layout. We set
 * a phone viewport and drive the provider Dropdown.
 */

test.use({ viewport: { width: 390, height: 780 } })

test.describe('User LLM Providers — mobile layout', () => {
  test('mobile viewport shows a provider Dropdown picker (not the sidebar)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Two providers so the picker has options to switch between.
    const p1 = await createProviderViaAPI(apiURL, token, 'Mobile Provider One', 'openai')
    await createModelViaAPI(apiURL, token, p1, 'm1', 'Mobile Model 1', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, p1)
    const p2 = await createProviderViaAPI(apiURL, token, 'Mobile Provider Two', 'anthropic')
    await createModelViaAPI(apiURL, token, p2, 'm2', 'Mobile Model 2', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, p2)

    await page.goto(`${baseURL}/settings/user-llm-providers`)

    // On the phone viewport the provider picker is a Dropdown trigger button
    // showing the current provider; the desktop sidebar is hidden.
    const picker = byTestId(page, 'ullm-provider-dropdown-trigger')
    await expect(picker).toBeVisible({ timeout: 30000 })

    // Opening it reveals BOTH providers as menu items (derived `${dropdown}-item-${id}`),
    // and selecting the other switches the active provider — the trigger then
    // shows "Mobile Provider Two" (dynamic data this test created).
    await picker.click()
    const p2Item = byTestId(page, `ullm-provider-dropdown-item-${p2}`)
    await expect(p2Item).toBeVisible({ timeout: 10000 })
    await p2Item.click()
    await expect(picker).toContainText('Mobile Provider Two', { timeout: 10000 })
  })
})
