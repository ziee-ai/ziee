import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToUsers, openCreateUserDrawer } from './helpers/user-navigation'

/**
 * E2E — keyboard navigation on the Create User drawer (audit 2eb7131978e8).
 *   - Tab moves focus forward through the username → email → password inputs.
 *   - Escape closes the drawer (Radix Dialog default).
 *   - Enter inside a field submits the form (primary CTA is type="submit").
 *
 * Nothing is mocked — real login, real drawer, real create endpoint.
 */

test.describe('Users — keyboard navigation', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToUsers(page, baseURL)
  })

  test('Tab advances focus through the form fields and Escape closes the drawer', async ({
    page,
  }) => {
    await openCreateUserDrawer(page)

    const username = byTestId(page, 'user-create-username-input')
    const email = byTestId(page, 'user-create-email-input')
    const password = byTestId(page, 'user-create-password-input')

    await username.focus()
    await expect(username).toBeFocused()

    await page.keyboard.press('Tab')
    await expect(email).toBeFocused()

    await page.keyboard.press('Tab')
    await expect(password).toBeFocused()

    // Escape closes the drawer.
    await page.keyboard.press('Escape')
    await expect(page.getByRole('dialog')).toHaveCount(0, { timeout: 5000 })
  })

  test('Enter inside a field submits the create form', async ({ page }) => {
    await openCreateUserDrawer(page)

    const tag = Date.now()
    const username = `kbduser${tag}`

    await byTestId(page, 'user-create-username-input').fill(username)
    await byTestId(page, 'user-create-email-input').fill(
      `${username}@example.com`,
    )
    const password = byTestId(page, 'user-create-password-input')
    await password.fill('password123')

    // Submit purely via the keyboard — Enter from a field, no button click.
    await password.press('Enter')

    // The form submitted: success toast fires and the drawer closes.
    await expect(
      page.locator('[data-sonner-toast][data-type="success"]').first(),
    ).toBeVisible({ timeout: 5000 })
    await expect(page.getByRole('dialog')).toHaveCount(0, { timeout: 5000 })

    // And the user the keyboard-submit created is really in the list.
    await expect(byTestId(page, `user-row-${username}`)).toBeVisible({
      timeout: 10000,
    })
  })
})
