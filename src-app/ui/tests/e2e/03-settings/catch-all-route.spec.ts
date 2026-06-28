import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the router catch-all (`*`) route.
 *
 * `RouterComponent.tsx:201` maps any unknown path to
 * `guardProtected(<Navigate to="/" replace />)`, so an authenticated user who
 * deep-links to a nonexistent route is redirected home (and the bogus path is
 * replaced, not pushed). Untested before.
 */

test.describe('Router — catch-all fallback', () => {
  test('an unknown route redirects an authenticated user to home', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Deep-link to a route that no module registers.
    await page.goto(`${baseURL}/this-route-does-not-exist-1234`)

    // The catch-all replaces it with "/" → the URL no longer contains the bogus path.
    await expect(page).not.toHaveURL(/this-route-does-not-exist-1234/, {
      timeout: 15000,
    })
    await expect(page).toHaveURL(new RegExp(`${baseURL}/?$`), { timeout: 15000 })
  })
})
