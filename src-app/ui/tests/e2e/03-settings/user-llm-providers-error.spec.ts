import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — UserLlmProvidersPage error state (UserLlmProvidersPage.tsx:110-112). A
 * failed providers load sets the store's `error`, which renders an error Alert
 * instead of the page content. We force the GET to 500 to drive that path.
 */

test.describe('User LLM Providers — error state', () => {
  test('a failed providers load renders the error alert', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Fail the providers list GET so loadProviders' catch sets `error`.
    await page.route(/\/api\/user-llm-providers$/, async (route, req) => {
      if (req.method() === 'GET') {
        return route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: { message: 'boom' } }),
        })
      }
      return route.fallback()
    })

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/user-llm-providers`)

    // The page renders an error Alert rather than the provider UI.
    await expect(page.locator('.ant-alert-error')).toBeVisible({ timeout: 30000 })
  })
})
