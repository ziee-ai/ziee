import { test, expect } from '../permissions/no-403'
import { Page } from '@playwright/test'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  clearAuthState,
  login,
} from '../../common/auth-helpers'
import { waitForSettingsPageLoad } from './helpers/navigation-helpers'

/**
 * Self-service profile page (`/settings/profile`).
 *
 * The whole spec runs under the `no-403` fixture: every test logs in as
 * a NON-admin user, so any accidental admin-gated /api call from the
 * profile page would surface as a 403 → test failure. This is the
 * regression guard that the page reads only Stores.Auth + self endpoints.
 *
 * NOTE: the "password section hidden for OAuth/LDAP accounts" case is not
 * reproducible in E2E without a configured external provider — it's pinned
 * by the backend integration tests (`has_password=false` + NO_LOCAL_PASSWORD).
 *
 * NOTE: the edit-controls gate (`profile::edit`) is likewise not cleanly
 * E2E-testable: a user with `profile::read` but NOT `profile::edit` cannot
 * even complete onboarding (the onboarding-complete endpoint itself requires
 * `profile::edit`), so AuthGuard bounces them to /onboarding before they can
 * reach /settings/profile. The 403 gate is pinned by the backend tests
 * `update_profile_without_permission_returns_403` +
 * `change_password_without_permission_returns_403`.
 */

const PASSWORD = 'password123'

interface RegularUser {
  username: string
  email: string
  password: string
}

/**
 * Bootstrap the admin (first-run setup), mint an admin token, create a
 * fresh non-admin user with a known local password, then drop admin auth
 * and log in AS that user. Leaves the page on the authenticated home view.
 */
async function loginAsFreshUser(
  page: Page,
  baseURL: string,
  apiURL: string,
  tag: string,
): Promise<RegularUser> {
  await loginAsAdmin(page, baseURL)
  const adminToken = await getAdminToken(apiURL)

  const suffix = `${tag}_${Date.now().toString(36)}${Math.floor(Math.random() * 1000)}`
  const username = `prof_${suffix}`
  const email = `${username}@example.com`
  await createTestUser(apiURL, adminToken, username, email, PASSWORD, [])

  await clearAuthState(page)
  await login(page, baseURL, username, PASSWORD)
  return { username, email, password: PASSWORD }
}

async function gotoProfile(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/profile`)
  await page
    .getByRole('heading', { name: 'Profile' })
    .waitFor({ timeout: 30000 })
}

test.describe('Settings - Profile (self-service)', () => {
  test('passes accessibility checks', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsFreshUser(page, baseURL, apiURL, 'a11y')
    await gotoProfile(page, baseURL)
    await assertNoAccessibilityViolations(page, { disabledRules: ['label'] })
  })

  test('opens from the user-profile widget dropdown', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsFreshUser(page, baseURL, apiURL, 'nav')

    await page.getByTestId('user-profile-widget').click()
    await page.getByRole('menuitem', { name: 'Profile' }).click()

    await expect(page).toHaveURL(/\/settings\/profile$/)
    await expect(page.getByRole('heading', { name: 'Profile' })).toBeVisible()
  })

  test('shows the read-only account info card', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'acct')
    await gotoProfile(page, baseURL)

    // The account-info Descriptions surface the user's email + temporal stats.
    await expect(page.getByText(user.email)).toBeVisible()
    await expect(page.getByText('Member since')).toBeVisible()
    await expect(page.getByText('Last login')).toBeVisible()
    // A fresh local registration is not email-verified.
    await expect(page.getByText(/Email (verified|unverified)/)).toBeVisible()
  })

  test('edits display name and persists across reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsFreshUser(page, baseURL, apiURL, 'disp')
    await gotoProfile(page, baseURL)

    const newName = 'Edited Display Name'
    await page.getByLabel('Display name').fill(newName)
    await page.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Profile saved.')).toBeVisible()

    await page.reload()
    await gotoProfile(page, baseURL)
    await expect(page.getByLabel('Display name')).toHaveValue(newName)
  })

  test('edits username and the sidebar widget reflects it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'uname')
    await gotoProfile(page, baseURL)

    const newUsername = `${user.username}_renamed`
    await page.getByLabel('Username').fill(newUsername)
    await page.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Profile saved.')).toBeVisible()

    // The sidebar widget renders the username — refreshCurrentUser should
    // update it without a reload. Assert on text content (not visibility:
    // a collapsed sidebar renders the label with opacity 0).
    await expect(page.getByTestId('user-profile-widget')).toContainText(
      newUsername,
    )
  })

  test("rejects taking another user's username (409 surfaces an error)", async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    // Bootstrap admin, then create two users: one whose name we collide
    // with, and the actor. getAdminToken is a standalone API login, so it
    // works regardless of the page's auth state.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const taken = `taken_${Date.now().toString(36)}`
    const me = `clash_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      taken,
      `${taken}@example.com`,
      PASSWORD,
      [],
    )
    await createTestUser(
      apiURL,
      adminToken,
      me,
      `${me}@example.com`,
      PASSWORD,
      [],
    )

    await clearAuthState(page)
    await login(page, baseURL, me, PASSWORD)
    await gotoProfile(page, baseURL)

    await page.getByLabel('Username').fill(taken)
    await page.getByRole('button', { name: 'Save' }).click()

    // 409 → message.error (a 409 is NOT a 403, so the no-403 fixture is happy).
    await expect(page.locator('.ant-message-error')).toBeVisible()
  })

  test('changes password and can log in with the new one', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'cpw')
    await gotoProfile(page, baseURL)

    const newPassword = 'BrandNewStrongPass456!'
    await page.getByLabel('Current password').fill(user.password)
    await page.getByLabel('New password', { exact: true }).fill(newPassword)
    await page.getByLabel('Confirm new password').fill(newPassword)
    await page.getByRole('button', { name: 'Change password' }).click()
    await expect(page.getByText('Password changed.')).toBeVisible()

    // The new password authenticates against the backend.
    const res = await page.request.post(`${apiURL}/api/auth/login`, {
      data: { username: user.username, password: newPassword },
    })
    expect(res.status()).toBe(200)
  })

  test('shows an error when the current password is wrong', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsFreshUser(page, baseURL, apiURL, 'cpwrong')
    await gotoProfile(page, baseURL)

    await page.getByLabel('Current password').fill('not-my-password')
    await page
      .getByLabel('New password', { exact: true })
      .fill('AnotherStrongPass789!')
    await page.getByLabel('Confirm new password').fill('AnotherStrongPass789!')
    await page.getByRole('button', { name: 'Change password' }).click()

    await expect(page.locator('.ant-message-error')).toBeVisible()
  })

  test('saving the profile form with no changes still succeeds (no-op save)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'noop')
    await gotoProfile(page, baseURL)

    // Don't touch any field — just save. The form submits the unchanged
    // values and the backend treats it as an idempotent update.
    await page.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Profile saved.')).toBeVisible()

    // The username is unchanged after the no-op save.
    await page.reload()
    await gotoProfile(page, baseURL)
    await expect(page.getByLabel('Username')).toHaveValue(user.username)
  })

  test('OAuth/password-less account hides the change-password form', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'oauth')

    // Simulate an external-only (OAuth/LDAP) account: the user has no
    // local password hash. The profile page gates the password form on
    // `has_password`, so the change-password form must NOT render — only
    // the "external provider" notice.
    await testInfra.sql(
      'UPDATE users SET password_hash = NULL WHERE username = $1',
      [user.username],
    )

    await gotoProfile(page, baseURL)
    await page.reload()
    await gotoProfile(page, baseURL)

    await expect(
      page.getByText(
        'You sign in through an external provider, so there is no password to change here.',
      ),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: 'Change password' }),
    ).toHaveCount(0)
  })

  test('read-only profile (no profile::edit) disables the form + shows the notice', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsFreshUser(page, baseURL, apiURL, 'readonly')

    // The fresh user has profile::edit via the default Users group (so it could
    // complete onboarding). To exercise the canEdit=false branch we strip
    // profile::edit from the /auth/me bootstrap — the FRONTEND gate
    // (`usePermission(ProfileEdit)`) is the unit under test, with the permission
    // set (its sole input) coming from that endpoint.
    await page.route(/\/api\/auth\/me$/, async (route) => {
      const res = await route.fetch()
      const body = await res.json()
      body.permissions = (body.permissions as string[]).filter(
        (p) => p !== 'profile::edit',
      )
      await route.fulfill({ response: res, json: body })
    })

    await gotoProfile(page, baseURL)

    // The read-only notice renders, the form fields are disabled, and the
    // edit-only actions (Save / Change password) are gone.
    await expect(
      page.getByText(/Fields are read-only/),
    ).toBeVisible()
    await expect(page.getByLabel('Display name')).toBeDisabled()
    await expect(page.getByLabel('Username')).toBeDisabled()
    await expect(page.getByRole('button', { name: 'Save' })).toHaveCount(0)
  })

  test('blocks weak new password and mismatched confirmation client-side', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'cpval')
    await gotoProfile(page, baseURL)

    // Too short (< 8) → field rule blocks submit.
    await page.getByLabel('Current password').fill(user.password)
    await page.getByLabel('New password', { exact: true }).fill('short')
    await page.getByLabel('Confirm new password').fill('short')
    await page.getByRole('button', { name: 'Change password' }).click()
    await expect(
      page.getByText('Password must be at least 8 characters'),
    ).toBeVisible()

    // Mismatched confirm → confirm rule blocks submit.
    await page
      .getByLabel('New password', { exact: true })
      .fill('GoodStrongPass123!')
    await page.getByLabel('Confirm new password').fill('DifferentPass123!')
    await page.getByRole('button', { name: 'Change password' }).click()
    await expect(page.getByText('Passwords do not match')).toBeVisible()
  })

  /// UserProfileWidget logout (UserProfileWidget.tsx:109-112). The widget
  /// dropdown's "Profile" item is tested; "Logout" → Stores.Auth.logoutUser was
  /// not. Clicking it must clear auth and return to the login page.
  test('the user-profile widget Logout returns to the login page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsFreshUser(page, baseURL, apiURL, 'logout')

    await page.getByTestId('user-profile-widget').click()
    await page.getByRole('menuitem', { name: 'Logout' }).click()

    // Logged out → redirected to /auth; the widget is gone.
    await expect(page).toHaveURL(/\/auth(\b|\/|$)/, { timeout: 15000 })
    await expect(page.getByTestId('user-profile-widget')).toHaveCount(0)
  })

  /// /settings/general is gated only by `requiresAuth` (no permission), so a
  /// NON-admin user must be able to open it. Every other settings spec logs in
  /// as admin; this asserts the unprivileged access path renders (no 403).
  test('a non-admin user can open /settings/general', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsFreshUser(page, baseURL, apiURL, 'genaccess')

    await page.goto(`${baseURL}/settings/general`)
    await waitForSettingsPageLoad(page, 'General')
    await expect(page.getByText(/Not authorized/i)).toHaveCount(0)
    // The Appearance theme card (always on the General page) renders.
    await expect(page.getByLabel('Theme')).toBeVisible({ timeout: 15000 })
  })
})
