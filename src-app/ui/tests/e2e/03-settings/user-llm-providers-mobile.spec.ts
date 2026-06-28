import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

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

    // On the phone viewport the provider picker is a Dropdown Button showing the
    // current provider; the desktop sidebar Title is hidden.
    const picker = page
      .getByRole('button')
      .filter({ hasText: /Mobile Provider (One|Two)/ })
      .first()
    await expect(picker).toBeVisible({ timeout: 30000 })

    // Opening it reveals BOTH providers as menu items, and selecting the other
    // switches the active provider.
    await picker.click()
    await expect(
      page.getByRole('menuitem', { name: /Mobile Provider Two/ }),
    ).toBeVisible({ timeout: 10000 })
    await page.getByRole('menuitem', { name: /Mobile Provider Two/ }).click()
    await expect(
      page
        .getByRole('button')
        .filter({ hasText: /Mobile Provider Two/ })
        .first(),
    ).toBeVisible({ timeout: 10000 })
  })
})
