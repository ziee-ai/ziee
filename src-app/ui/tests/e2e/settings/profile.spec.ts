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
import { byTestId } from '../testid.ts'

// Sonner toast feedback (i18n-safe: select by toast type, not message text).
const successToast = (page: Page) =>
  page.locator('[data-sonner-toast][data-type="success"]')
const errorToast = (page: Page) =>
  page.locator('[data-sonner-toast][data-type="error"]')

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
  await byTestId(page, 'settings-page-title').waitFor({ timeout: 30000 })
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

    await byTestId(page, 'user-profile-widget').click()
    await byTestId(page, 'userprofile-menu-dropdown-item-profile').click()

    await expect(page).toHaveURL(/\/settings\/profile$/)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible()
  })

  test('shows the read-only account info card', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'acct')
    await gotoProfile(page, baseURL)

    // The account-info Descriptions surface the user's email + temporal stats.
    const descriptions = byTestId(page, 'profile-account-descriptions')
    await expect(descriptions).toContainText(user.email)
    await expect(descriptions).toContainText('Member since')
    await expect(descriptions).toContainText('Last login')
    // A fresh local registration renders the email-verified status tag.
    await expect(byTestId(page, 'profile-email-verified-tag')).toBeVisible()
  })

  test('edits display name and persists across reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsFreshUser(page, baseURL, apiURL, 'disp')
    await gotoProfile(page, baseURL)

    const newName = 'Edited Display Name'
    await byTestId(page, 'profile-display-name-input').fill(newName)
    await byTestId(page, 'profile-save-button').click()
    await expect(successToast(page)).toBeVisible()

    await page.reload()
    await gotoProfile(page, baseURL)
    await expect(byTestId(page, 'profile-display-name-input')).toHaveValue(
      newName,
    )
  })

  test('edits username and the sidebar widget reflects it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'uname')
    await gotoProfile(page, baseURL)

    const newUsername = `${user.username}_renamed`
    await byTestId(page, 'profile-username-input').fill(newUsername)
    await byTestId(page, 'profile-save-button').click()
    await expect(successToast(page)).toBeVisible()

    // The sidebar widget renders display_name, falling back to the username —
    // and this user has none (createTestUser sends no display_name, and
    // POST /api/users stores it verbatim without defaulting), so the fallback
    // is what shows. refreshCurrentUser should update it without a reload.
    // Assert on text content (not visibility: a collapsed sidebar renders the
    // label with opacity 0).
    await expect(byTestId(page, 'user-profile-widget')).toContainText(
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

    await byTestId(page, 'profile-username-input').fill(taken)
    await byTestId(page, 'profile-save-button').click()

    // 409 → error toast (a 409 is NOT a 403, so the no-403 fixture is happy).
    await expect(errorToast(page)).toBeVisible()
  })

  test('changes password and can log in with the new one', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'cpw')
    await gotoProfile(page, baseURL)

    const newPassword = 'BrandNewStrongPass456!'
    await byTestId(page, 'profile-current-password-input').fill(user.password)
    await byTestId(page, 'profile-new-password-input').fill(newPassword)
    await byTestId(page, 'profile-confirm-password-input').fill(newPassword)
    await byTestId(page, 'profile-change-password-button').click()
    await expect(successToast(page)).toBeVisible()

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

    await byTestId(page, 'profile-current-password-input').fill('not-my-password')
    await byTestId(page, 'profile-new-password-input').fill(
      'AnotherStrongPass789!',
    )
    await byTestId(page, 'profile-confirm-password-input').fill(
      'AnotherStrongPass789!',
    )
    await byTestId(page, 'profile-change-password-button').click()

    await expect(errorToast(page)).toBeVisible()
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
    await byTestId(page, 'profile-save-button').click()
    await expect(successToast(page)).toBeVisible()

    // The username is unchanged after the no-op save.
    await page.reload()
    await gotoProfile(page, baseURL)
    await expect(byTestId(page, 'profile-username-input')).toHaveValue(
      user.username,
    )
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

    await expect(byTestId(page, 'profile-no-password-notice')).toBeVisible()
    await expect(
      byTestId(page, 'profile-change-password-button'),
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
    await expect(byTestId(page, 'profile-readonly-alert')).toBeVisible()
    await expect(byTestId(page, 'profile-display-name-input')).toBeDisabled()
    await expect(byTestId(page, 'profile-username-input')).toBeDisabled()
    await expect(byTestId(page, 'profile-save-button')).toHaveCount(0)
  })

  test('blocks weak new password and mismatched confirmation client-side', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL, 'cpval')
    await gotoProfile(page, baseURL)

    // Too short (< 8) → the new-password field rule blocks submit (the field
    // is flagged invalid and no success toast fires).
    await byTestId(page, 'profile-current-password-input').fill(user.password)
    await byTestId(page, 'profile-new-password-input').fill('short')
    await byTestId(page, 'profile-confirm-password-input').fill('short')
    await byTestId(page, 'profile-change-password-button').click()
    await expect(byTestId(page, 'profile-new-password-input')).toHaveAttribute(
      'aria-invalid',
      'true',
    )
    await expect(successToast(page)).toHaveCount(0)

    // Mismatched confirm → the confirm-password field rule blocks submit.
    await byTestId(page, 'profile-new-password-input').fill('GoodStrongPass123!')
    await byTestId(page, 'profile-confirm-password-input').fill(
      'DifferentPass123!',
    )
    await byTestId(page, 'profile-change-password-button').click()
    await expect(
      byTestId(page, 'profile-confirm-password-input'),
    ).toHaveAttribute('aria-invalid', 'true')
    await expect(successToast(page)).toHaveCount(0)
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

    await byTestId(page, 'user-profile-widget').click()
    await byTestId(page, 'userprofile-menu-dropdown-item-logout').click()

    // Logged out → the AuthGuard renders the login wall inline (no URL
    // redirect); the login form appears and the profile widget is gone.
    await expect(byTestId(page, 'auth-login-username')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'user-profile-widget')).toHaveCount(0)
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
    await expect(byTestId(page, 'settings-forbidden-result')).toHaveCount(0)
    // The Appearance theme select (always on the General page) renders.
    await expect(byTestId(page, 'settingsgen-theme-select')).toBeVisible({
      timeout: 15000,
    })
  })
})
