import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToProvidersPage,
  waitForProvidersPageLoad,
  goToProviderDetail,
} from './helpers/navigation-helpers'
import { openAddProviderDrawer } from './helpers/provider-helpers'
import { byTestId } from '../testid'

/**
 * TEST-11 (ITEM-4, ITEM-6): OpenRouter is a first-class provider type; a
 * catalog-deprecated model shows the Deprecated badge; and the "Refresh models"
 * button reconciles against the provider's live list.
 */
test.describe('LLM Models - deprecated badge + refresh + OpenRouter type', () => {
  test('OpenRouter appears in the provider-type list', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    await openAddProviderDrawer(page)
    await byTestId(page, 'llm-provider-type-select').click()
    await expect(byTestId(page, 'llm-provider-type-select-opt-openrouter')).toBeVisible({
      timeout: 10000,
    })
  })

  test('deprecated model shows a badge and refresh reconciles', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerName = `dep-${Date.now()}`
    const providerId = await createProviderViaAPI(apiURL, adminToken, providerName, 'openai')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)

    // gpt-3.5-turbo is catalog-deprecated → create-time flag sets is_deprecated.
    const modelId = await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'gpt-3.5-turbo',
      'GPT-3.5 Turbo',
    )

    await goToProviderDetail(page, baseURL, providerId)
    await byTestId(page, 'llm-models-section-card').waitFor({ state: 'visible', timeout: 15000 })

    // The Deprecated badge renders on the deprecated model's row.
    await expect(byTestId(page, `llm-model-deprecated-badge-${modelId}`)).toBeVisible({
      timeout: 15000,
    })

    // The "Refresh models" button reconciles against the live list (real POST).
    const [refreshResp] = await Promise.all([
      page.waitForResponse(
        r => /\/refresh-models$/.test(r.url()) && r.request().method() === 'POST',
        { timeout: 20000 },
      ),
      byTestId(page, 'llm-models-refresh-btn').click(),
    ])
    expect(refreshResp.ok()).toBeTruthy()

    // Still deprecated after refresh (a keyless openai fetch is a no-op).
    await expect(byTestId(page, `llm-model-deprecated-badge-${modelId}`)).toBeVisible()
  })
})
