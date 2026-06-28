import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, clearAuthState } from '../../common/auth-helpers'

/**
 * E2E — login with WRONG credentials surfaces the server error Alert.
 *
 * `LoginForm` renders an antd Alert from `Stores.Auth` `error` when
 * authentication fails (LoginForm.tsx:33-41). The existing auth.spec only
 * covers client-side required-field validation, never a real failed login.
 */

test.describe('Authentication — failed login', () => {
  test('submitting wrong credentials shows the server error alert', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Ensure an admin exists, then drop auth state so we land on the login form.
    await loginAsAdmin(page, baseURL)
    await clearAuthState(page)

    await page.goto(`${baseURL}/auth`, { waitUntil: 'load' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Valid form (passes client-side validation) but wrong password → the
    // backend rejects and the store sets `error`, rendered as an error Alert.
    await page.getByLabel('Username or Email').fill('admin')
    await page.getByLabel('Password', { exact: true }).fill('definitely-wrong')
    await page.getByRole('button', { name: /^sign in$/i }).click()

    // The server-error Alert appears (not a client validation message).
    await expect(page.locator('.ant-alert-error')).toBeVisible({ timeout: 15000 })
    // And we stay on the auth page (login did not succeed).
    await expect(page.getByLabel('Username or Email')).toBeVisible()
  })
})
