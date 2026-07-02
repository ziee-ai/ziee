import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { createProviderViaAPI } from '../../common/provider-helpers'
import {
  goToProvidersPage,
  waitForProvidersPageLoad,
  clickProviderCard,
  switchToTab,
} from './helpers/navigation-helpers'
import { byTestId } from '../testid'

/**
 * Proxy Settings card (ProviderProxySettingsForm), rendered on a REMOTE
 * provider's Settings tab via RemoteProviderSettings. Previously no E2E spec
 * exercised this card. Enable the proxy, set a URL, save → real PUT succeeds.
 */
test.describe('LLM Providers - Proxy Settings card', () => {
  test('enable proxy + set URL persists via the provider PUT', async ({
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

    const proxyCard = byTestId(page, 'llm-proxy-settings-card')
    await expect(proxyCard).toBeVisible({ timeout: 15000 })

    // Enable the proxy → the URL field becomes required/active.
    await byTestId(page, 'llm-proxy-enabled-switch').click()
    await byTestId(page, 'llm-proxy-url-input').fill('http://proxy.company.com:8080')

    const [resp] = await Promise.all([
      page.waitForResponse(
        r =>
          r.url().includes('/api/llm-providers') &&
          r.request().method() === 'POST',
        { timeout: 10000 },
      ),
      byTestId(page, 'llm-proxy-save-btn').click(),
    ])
    expect(resp.ok()).toBeTruthy()
  })
})
