import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToProviderDetail } from './helpers/navigation-helpers'
import { byTestId } from '../testid'

/**
 * TEST-10 (ITEM-1, ITEM-2, ITEM-3): the Add Remote Model drawer's discovery
 * picker. Opening the drawer calls /discover-models; with no valid key the live
 * call fails and the backend returns the curated catalog, so the picker is
 * populated. Selecting gpt-4o auto-fills the display name + capabilities (vision),
 * which persist onto the saved model. The custom-id toggle swaps to a text input.
 */
test.describe('LLM Models - remote model picker', () => {
  test('discovers catalog models, auto-fills capabilities, and saves', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerName = `picker-${Date.now()}`
    const providerId = await createProviderViaAPI(apiURL, adminToken, providerName, 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)

    await goToProviderDetail(page, baseURL, providerId)
    await byTestId(page, 'llm-models-section-card').waitFor({ state: 'visible', timeout: 15000 })

    // Opening the drawer triggers discover-models; the remote add button opens it.
    const [discoverResp] = await Promise.all([
      page.waitForResponse(r => /\/discover-models/.test(r.url()), { timeout: 20000 }),
      byTestId(page, 'llm-models-add-remote-btn').click(),
    ])
    expect(discoverResp.ok()).toBeTruthy()

    await byTestId(page, 'llm-add-remote-model-form').waitFor({ state: 'visible', timeout: 10000 })

    // The picker is populated from the real GET /discover-models (catalog
    // fallback when the key is invalid) — gpt-4o is a known catalog model.
    const picker = byTestId(page, 'llm-remote-model-picker')
    await picker.click()
    await expect(byTestId(page, 'llm-remote-model-picker-opt-gpt-4o')).toBeVisible({
      timeout: 15000,
    })

    // Select via keyboard — the robust path for a Base UI combobox (a bare/forced
    // click on the floating option is unstable). Filter to gpt-4o then commit.
    await picker.fill('gpt-4o')
    await page.keyboard.press('ArrowDown')
    await page.keyboard.press('Enter')

    // Selecting a model auto-fills the display name (the testid is on the Input).
    await expect(byTestId(page, 'llm-param-display_name')).not.toHaveValue('')

    // Save → POST /llm-models.
    const [createResp] = await Promise.all([
      page.waitForResponse(
        r => /\/api\/llm-models$/.test(r.url()) && r.request().method() === 'POST',
        { timeout: 15000 },
      ),
      byTestId(page, 'llm-add-remote-submit-btn').click(),
    ])
    expect(createResp.ok()).toBeTruthy()

    // The new model appears in the provider's model list — discovery → pick →
    // auto-fill → persist worked end-to-end. (Capability mapping itself is
    // asserted in the discoveredModelForm unit test.)
    await expect(byTestId(page, 'llm-models-section-card')).toContainText('Model ID:', {
      timeout: 15000,
    })
  })

  test('custom-id toggle swaps the picker for a text input', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerName = `picker-custom-${Date.now()}`
    const providerId = await createProviderViaAPI(apiURL, adminToken, providerName, 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)

    await goToProviderDetail(page, baseURL, providerId)
    await byTestId(page, 'llm-models-add-remote-btn').click()
    await byTestId(page, 'llm-add-remote-model-form').waitFor({ state: 'visible', timeout: 10000 })

    // Picker is shown by default; the custom-id input is not.
    await expect(byTestId(page, 'llm-remote-model-picker')).toBeVisible()
    await expect(byTestId(page, 'llm-remote-model-custom-id')).toHaveCount(0)

    // Toggle on → the plain text input replaces the picker.
    await byTestId(page, 'llm-remote-custom-id-toggle').click()
    await expect(byTestId(page, 'llm-remote-model-custom-id')).toBeVisible()
    await expect(byTestId(page, 'llm-remote-model-picker')).toHaveCount(0)
  })
})
