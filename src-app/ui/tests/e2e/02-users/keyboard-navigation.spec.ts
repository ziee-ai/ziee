import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToUsers, openCreateUserDrawer } from './helpers/user-navigation'

/**
 * E2E — keyboard navigation on the Create User drawer (audit 2eb7131978e8).
 *
 * No prior spec exercised keyboard interaction; every flow drove the mouse.
 * The Create User drawer is a stable surface with deterministic keyboard
 * semantics:
 *   - Tab moves focus forward through the plain antd `Input` fields
 *     (username → email → password — none use `allowClear`, so no clear-icon
 *     button interrupts the tab order; see CreateUserDrawer.tsx:58-87).
 *   - Escape closes the drawer: the shared `Drawer` wrapper is an antd
 *     `<Drawer>` with `keyboard` left at its default (`true`) and `onClose`
 *     wired, so Esc invokes onClose (layouts/app-layout/components/Drawer.tsx).
 *   - Enter inside a field submits the form: the primary CTA is
 *     `htmlType="submit"` (CreateUserDrawer.tsx:106), so antd Form submits on
 *     Enter from any input.
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
    const drawer = page.locator('.ant-drawer.ant-drawer-open')

    const username = drawer.getByLabel(/username/i)
    const email = drawer.getByLabel(/email/i)
    const password = drawer.getByLabel(/^password/i)

    // Focus the first field explicitly, then Tab forward and assert focus
    // lands on each subsequent field in order (real keyboard traversal).
    await username.focus()
    await expect(username).toBeFocused()

    await page.keyboard.press('Tab')
    await expect(email).toBeFocused()

    await page.keyboard.press('Tab')
    await expect(password).toBeFocused()

    // Escape closes the drawer (antd Drawer keyboard default → onClose).
    await page.keyboard.press('Escape')
    await expect(page.locator('.ant-drawer.ant-drawer-open')).not.toBeVisible({
      timeout: 5000,
    })
  })

  test('Enter inside a field submits the create form', async ({ page }) => {
    await openCreateUserDrawer(page)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')

    const tag = Date.now()
    const username = `kbduser${tag}`

    await drawer.getByLabel(/username/i).fill(username)
    await drawer.getByLabel(/email/i).fill(`${username}@example.com`)
    await drawer.getByLabel(/^password/i).fill('password123')

    // Submit purely via the keyboard — Enter from a field, no button click.
    await drawer.getByLabel(/^password/i).press('Enter')

    // The form submitted: success toast fires and the drawer closes.
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 5000,
    })
    await expect(page.locator('.ant-drawer.ant-drawer-open')).not.toBeVisible({
      timeout: 5000,
    })

    // And the user the keyboard-submit created is really in the list.
    await expect(page.getByText(username).first()).toBeVisible({
      timeout: 10000,
    })
  })
})
