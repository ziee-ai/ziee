import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { createProviderViaAPI } from '../../common/provider-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the per-provider "Proxy Settings" card (ProviderProxySettingsForm.tsx).
 *
 * Audit gap: no spec exercised this card. It renders on a remote provider's
 * settings page; this enables the proxy, fills a valid proxy URL, and saves,
 * asserting the real PUT (RemoteProviderSettings handleProxySettingsSave →
 * LlmProvider.updateLlmProvider) succeeds.
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

    const card = byTestId(page, 'llm-proxy-settings-card')
    await expect(card).toBeVisible({ timeout: 30000 })

    await byTestId(page, 'llm-proxy-enabled-switch').click()
    await byTestId(page, 'llm-proxy-url-input').fill('http://proxy.example.com:8080')

    // Save → assert the real PUT to the provider endpoint succeeds.
    const [resp] = await Promise.all([
      page.waitForResponse(
        r =>
          r.url().includes('/api/llm-providers') &&
          r.request().method() === 'PUT',
        { timeout: 30000 },
      ),
      byTestId(page, 'llm-proxy-save-btn').click(),
    ])
    expect(resp.ok()).toBeTruthy()
  })
})
