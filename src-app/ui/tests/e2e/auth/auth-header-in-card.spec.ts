import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { createAdminViaSetup } from './helpers/form-helpers'
import { logoutAndGoToAuth } from './helpers/navigation-helpers'

// TEST-8 (covers ITEM-7, FB-5): the login/register heading lives INSIDE the card
// (mirroring the setup screen), not as a sibling above it — and there is exactly
// ONE heading per screen (no external + in-card double header on register).
test.describe('Auth header placement (in-card)', () => {
  test('login + register headings are inside their cards, one each', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await createAdminViaSetup(page, baseURL)
    await logoutAndGoToAuth(page, baseURL)

    // Login: "Welcome back" is a DESCENDANT of the login card.
    const loginCard = byTestId(page, 'auth-login-card')
    await expect(
      loginCard.getByRole('heading', { name: 'Welcome back' }),
    ).toBeVisible()
    // Exactly one heading on the login screen (no external duplicate above the card).
    await expect(page.getByRole('heading', { name: 'Welcome back' })).toHaveCount(1)

    // Switch to register.
    await byTestId(page, 'auth-login-switch-to-register').click()
    await expect(byTestId(page, 'auth-register-form')).toBeVisible()

    // Register: "Create Account" is a DESCENDANT of the register card, exactly one.
    const registerCard = byTestId(page, 'auth-register-card')
    await expect(
      registerCard.getByRole('heading', { name: 'Create Account' }),
    ).toBeVisible()
    await expect(
      page.getByRole('heading', { name: 'Create Account' }),
    ).toHaveCount(1)
    // The old external "Create your account" title must be gone.
    await expect(
      page.getByRole('heading', { name: 'Create your account' }),
    ).toHaveCount(0)
  })
})
