import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * The self-service "LLM Providers" page (/settings/user-llm-providers,
 * UserLlmProvidersPage.tsx) renders fallback Empty states when there is no
 * provider to configure a key for. On a fresh deployment no admin has added a
 * provider+model yet, so `renderContent()` hits its no-provider branch.
 */
test.describe('User LLM providers — no-provider fallback', () => {
  test('shows the empty state when no providers are available', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await expect(
      page.getByRole('heading', { name: 'LLM Providers' }),
    ).toBeVisible({ timeout: 15000 })

    // No provider configured anywhere → the page renders its Empty fallback
    // instead of a provider key form.
    await expect(
      page.getByText('No AI providers are available yet.'),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: /save key/i }),
    ).toHaveCount(0)
  })
})
