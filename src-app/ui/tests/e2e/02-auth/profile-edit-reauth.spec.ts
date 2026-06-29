/**
 * Profile-edit → re-auth E2E (gap 26e4).
 *
 * The existing social-login specs cover the OAuth callback flow but NOT the
 * substantive sequence: a user edits their profile username, logs out, and logs
 * back in with the NEW username. (Re-auth by *username* is a local-credentials
 * concern — OAuth re-auth keys off the provider identity, not the username — so
 * the testable behavior is "username change persists and the new username
 * authenticates".)
 */
import { test, expect } from '../../fixtures/test-context'
import {
  clearAuthState,
  createTestUser,
  getAdminToken,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

test.describe('Profile edit then re-auth with new username', () => {
  test('username change persists and the new username can log in', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    const suffix = `${Date.now()}`
    const oldName = `peruser_${suffix}`
    const newName = `peruser_${suffix}_renamed`
    const password = 'password123'

    await createTestUser(
      apiURL,
      adminToken,
      oldName,
      `${oldName}@example.com`,
      password,
      ['profile::edit'],
    )

    // Log in as the freshly-created user.
    await login(page, baseURL, oldName, password)

    // Edit the username in profile settings and save.
    await page.goto(`${baseURL}/settings/profile`)
    const usernameField = byTestId(page, 'profile-username-input')
    await usernameField.waitFor({ timeout: 30000 })
    await usernameField.fill(newName)
    // Save succeeded → the profile update request returns 200.
    const savePromise = page.waitForResponse(
      r => r.url().includes('/api/auth/profile') && r.request().method() === 'POST',
      { timeout: 15000 },
    )
    await byTestId(page, 'profile-save-button').click()
    expect((await savePromise).ok()).toBeTruthy()

    // Log out completely, then re-authenticate with the NEW username.
    await clearAuthState(page)
    await login(page, baseURL, newName, password)

    // Re-auth succeeded and the change persisted: the profile form now shows
    // the new username.
    await page.goto(`${baseURL}/settings/profile`)
    const reloaded = byTestId(page, 'profile-username-input')
    await reloaded.waitFor({ timeout: 30000 })
    await expect(reloaded).toHaveValue(newName)
  })
})
