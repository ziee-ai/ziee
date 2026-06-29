import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * The hardware "Connect" (real-time monitoring) button is gated on
 * `hardware::monitor` (HardwareSettings.tsx). The /settings/hardware route only
 * needs `hardware::read`, so a read-only user can view the page but must NOT
 * see the Connect button — and never triggers the monitor SSE.
 */
test.describe('Hardware - monitor permission gating', () => {
  test('read-only user sees the page but not the Connect button; admin sees it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A user who can VIEW hardware info but not MONITOR it.
    const username = `hw_readonly_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'hardware::read'],
    )
    await login(page, baseURL, username, 'password123')

    await page.goto(`${baseURL}/settings/hardware`)
    await expect(byTestId(page, 'hardware-os-card')).toBeVisible({ timeout: 30000 })
    // The monitoring status card renders, but without hardware::monitor the
    // viewer neither auto-connects (stays Disconnected) nor gets the gated
    // Connect button.
    await expect(byTestId(page, 'hardware-settings-connection-tag')).toBeVisible()
    await expect(byTestId(page, 'hardware-settings-connection-tag')).toContainText('Disconnected')
    await expect(byTestId(page, 'hardware-settings-connect-btn')).toHaveCount(0)
  })
})
