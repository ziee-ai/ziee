import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { createProviderViaAPI } from '../../common/provider-helpers'
import {
  goToProvidersPage,
  waitForProvidersPageLoad,
  clickProviderCard,
  switchToTab,
} from './helpers/navigation-helpers'

/**
 * Proxy Settings card (ProviderProxySettingsForm), rendered on a REMOTE
 * provider's Settings tab via RemoteProviderSettings. Previously no E2E spec
 * exercised this card. Enable the proxy, set a URL, save → success toast.
 */
test.describe('LLM Providers - Proxy Settings card', () => {
  test('enable proxy + set URL persists with a success toast', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerName = `proxy-prov-${Date.now()}`
    await createProviderViaAPI(apiURL, adminToken, providerName, 'openai')

    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)
    await clickProviderCard(page, providerName)
    await expect(page).toHaveURL(/\/settings\/llm-providers\/[a-f0-9-]+/)
    await switchToTab(page, 'settings')

    const proxyCard = page
      .locator('.ant-card')
      .filter({ hasText: 'Proxy Settings' })
    await expect(proxyCard).toBeVisible({ timeout: 15000 })

    // Enable the proxy → the URL field becomes required/active.
    await proxyCard
      .getByRole('switch', { name: 'Enable or disable proxy settings' })
      .click()
    await proxyCard
      .getByPlaceholder('http://proxy.example.com:8080')
      .fill('http://proxy.company.com:8080')

    await proxyCard.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Proxy settings saved')).toBeVisible({
      timeout: 10000,
    })
  })
})
