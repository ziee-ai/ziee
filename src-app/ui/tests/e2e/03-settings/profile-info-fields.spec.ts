import { test, expect } from '../permissions/no-403'
import { Page } from '@playwright/test'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  clearAuthState,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * Profile page (`/settings/profile`) — user-info DISPLAY fields.
 *
 * ProfileSettingsPage.tsx:96-126 renders the logged-in user's identity:
 * a role Tag (Administrator/User), an email-verified Tag, an Email +
 * "Member since" + "Last login" Descriptions block, and the editable
 * Display name / Username form inputs (pre-filled from the account).
 *
 * The existing profile.spec.ts exercises EDITING (display name save) but
 * never asserts that the page DISPLAYS the actual account's values. This
 * ties each rendered field to the known account it was created with, so a
 * regression that mis-binds the profile (e.g. shows another user's email,
 * or stops pre-filling the username) fails loudly. Runs under the `no-403`
 * fixture — a non-admin user, so no admin endpoint is touched.
 */

interface RegularUser {
  username: string
  email: string
}

async function loginAsFreshUser(
  page: Page,
  baseURL: string,
  apiURL: string,
): Promise<RegularUser> {
  await loginAsAdmin(page, baseURL)
  const adminToken = await getAdminToken(apiURL)

  const suffix = `${Date.now().toString(36)}${Math.floor(Math.random() * 1000)}`
  const username = `info_${suffix}`
  const email = `${username}@example.com`
  await createTestUser(apiURL, adminToken, username, email, 'password123', [])

  await clearAuthState(page)
  await login(page, baseURL, username, 'password123')
  return { username, email }
}

test.describe('Settings - Profile (info display fields)', () => {
  test('displays the logged-in account identity (email, username, role)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const user = await loginAsFreshUser(page, baseURL, apiURL)

    await page.goto(`${baseURL}/settings/profile`)
    await byTestId(page, 'settings-page-title').waitFor({ timeout: 30000 })

    // Email shown in the Descriptions block matches the actual account.
    const descriptions = byTestId(page, 'profile-account-descriptions')
    await expect(descriptions).toContainText(user.email)

    // Username form input is pre-filled with the account's username.
    await expect(byTestId(page, 'profile-username-input')).toHaveValue(
      user.username,
    )

    // A non-admin account renders the "User" role tag (not Administrator).
    const roleTag = byTestId(page, 'profile-role-tag')
    await expect(roleTag).toContainText('User')
    await expect(roleTag).not.toContainText('Administrator')

    // The identity Descriptions render their temporal label fields.
    await expect(descriptions).toContainText('Member since')
    await expect(descriptions).toContainText('Last login')
  })
})
