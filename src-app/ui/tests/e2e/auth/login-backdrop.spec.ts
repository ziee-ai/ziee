import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { setTheme } from '../../utils/theme'
import { createAdminViaSetup } from './helpers/form-helpers'
import { logoutAndGoToAuth } from './helpers/navigation-helpers'

// TEST-2 (covers ITEM-2, ITEM-3): the login page renders the shared themed
// backdrop + the login card over it, and exposes exactly ONE `main` landmark
// (the shared AuthScreenLayout is now the sole chrome — no double landmark).
test.describe('Login shared backdrop', () => {
  test('renders the backdrop, the card over it, and one main landmark', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await createAdminViaSetup(page, baseURL)
    await logoutAndGoToAuth(page, baseURL)

    await expect(byTestId(page, 'auth-screen-backdrop')).toBeVisible()
    await expect(byTestId(page, 'auth-login-card')).toBeVisible()
    // Exactly one main landmark — a second would be an a11y violation and would
    // mean AuthScreenLayout nested inside a router layout's main.
    await expect(page.getByRole('main')).toHaveCount(1)

    // Backdrop is still present in dark mode.
    await setTheme(page, 'dark')
    await byTestId(page, 'auth-login-username').waitFor({ timeout: 30000 })
    await expect(byTestId(page, 'auth-screen-backdrop')).toBeVisible()
    await expect(page.getByRole('main')).toHaveCount(1)
  })
})
