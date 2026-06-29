import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { createProviderViaAPI } from '../../common/provider-helpers'
import { byTestId } from '../testid'

/**
 * Dynamic + OPTIONAL route segments (router/types.ts: ":providerId?"). The LLM
 * providers route is registered as "/settings/llm-providers/:providerId?", so a
 * single RouteConfig must resolve BOTH the param-absent list view and the
 * param-present detail view. This pins that the optional segment is parsed and
 * routed correctly in both shapes.
 */
test.describe('Routing — optional dynamic route segment', () => {
  test('llm-providers route resolves with and without the :providerId param', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerName = `route-seg-${Date.now()}`
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      providerName,
      'openai',
    )

    // Param ABSENT → the list view renders (provider nav present).
    await page.goto(`${baseURL}/settings/llm-providers`)
    await expect(byTestId(page, 'llm-provider-nav-add-provider')).toBeVisible({ timeout: 15000 })

    // Param PRESENT → the SAME route renders the detail view for that id (the
    // header reflects the provider; its enable switch aria-label embeds the name).
    await page.goto(`${baseURL}/settings/llm-providers/${providerId}`)
    await expect(page).toHaveURL(
      new RegExp(`/settings/llm-providers/${providerId}$`),
    )
    await expect(byTestId(page, 'llm-models-section-card')).toBeVisible({ timeout: 15000 })
    await expect(page.locator(`[aria-label*="${providerName} provider"]`).first()).toBeVisible()
  })
})
