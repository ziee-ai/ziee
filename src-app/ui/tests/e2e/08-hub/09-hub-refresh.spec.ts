import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'

/**
 * The Hub page's admin-only "Refresh" button (HubPage.tsx) calls
 * POST /api/hub/refresh (fetch latest signed catalog → verify → rotate).
 * The real endpoint reaches out to GitHub, so we mock ONLY that POST and
 * assert the observable UI contract: a success toast naming the new
 * catalog version. The follow-up catalog/version reloads hit the real
 * (already-seeded) backend.
 */
test.describe('Hub refresh', () => {
  test('admin Refresh button refreshes the catalog and toasts success', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.route(/\/api\/hub\/refresh$/, async (route, req) => {
      if (req.method() === 'POST') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            new_version: '9.9.9',
            previous_version: '9.9.8',
            updated: true,
          }),
        })
      }
      return route.continue()
    })

    await navigateToHub(page, baseURL)
    await waitForHubDataLoad(page)

    await page.getByRole('button', { name: 'Refresh' }).click()

    await expect(
      page.getByText(/Hub catalog refreshed to v/),
    ).toBeVisible({ timeout: 10000 })
  })
})
