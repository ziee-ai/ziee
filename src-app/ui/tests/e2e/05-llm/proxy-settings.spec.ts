import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { createProviderViaAPI } from '../../common/provider-helpers'

/**
 * E2E — the per-provider "Proxy Settings" card (ProviderProxySettingsForm.tsx).
 *
 * Audit gap: no spec exercised this card. It renders on a remote provider's
 * settings page; this enables the proxy, fills a valid proxy URL, and saves,
 * asserting the "Proxy settings saved" toast (RemoteProviderSettings
 * handleProxySettingsSave → LlmProvider.updateLlmProvider — the real PUT).
 */

test.describe('LLM providers — proxy settings card', () => {
  test('enable proxy + set URL + Save round-trips', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(
      () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(
      apiURL,
      token,
      'Proxy Test Provider',
      'openai',
    )

    await page.goto(`${baseURL}/settings/llm-providers/${providerId}`)

    const card = page.locator('.ant-card:has-text("Proxy Settings")')
    await expect(card).toBeVisible({ timeout: 30000 })

    await card
      .getByRole('switch', { name: 'Enable or disable proxy settings' })
      .click()
    await card
      .getByPlaceholder('http://proxy.example.com:8080')
      .fill('http://proxy.example.com:8080')

    await card.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Proxy settings saved')).toBeVisible({
      timeout: 30000,
    })
  })
})
