import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — the SandboxResourceLimitsSection permission states.
 *
 * Audit gap: the section's two permission branches were untested:
 *   1. read-but-not-manage → the "Read-only view" info Alert + Save disabled.
 *   2. no resource_limits read (but route admitted via another sandbox perm)
 *      → the "don't have permission to view" denial Alert.
 *
 * Real backend throughout — users carry exactly the perms under test.
 */

async function gotoSandbox(page: import('@playwright/test').Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/sandbox`)
  // The page is a two-tab layout (Rootfs | Resource); the resource-limits
  // surface lives under the "Resource" tab, which shadcn Tabs keeps unmounted
  // until selected. Switch to it and wait for it to be selected — a stable
  // signal across all permission states (the card, read-only alert, and no-perm
  // alert are each a different per-perm rendering of the mounted panel, so the
  // per-test assertion below is what discriminates them).
  const resourceTab = page.getByRole('tab', { name: 'Resource' })
  await resourceTab.click()
  await expect(resourceTab).toHaveAttribute('aria-selected', 'true', { timeout: 30000 })
}

test.describe('Code Sandbox — resource-limits permission states', () => {
  test('read-only admin sees the "Read-only view" alert and a disabled Save', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const uname = `rl_ro_${Date.now()}`
    await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['code_sandbox::resource_limits::read'],
    )
    await login(page, baseURL, uname, 'password123')
    await gotoSandbox(page, baseURL)

    await expect(
      byTestId(page, 'sandbox-resource-limits-readonly-alert'),
    ).toBeVisible({ timeout: 30000 })
    // The section's Save control is disabled (form disabled={!canManage}).
    await expect(byTestId(page, 'sandbox-rl-save-btn')).toBeDisabled()
  })

  test('an admin without resource_limits read sees the section denial', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const uname = `rl_none_${Date.now()}`
    // environments::read admits the route (anyOf) but NOT the limits section.
    await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['code_sandbox::environments::read'],
    )
    await login(page, baseURL, uname, 'password123')
    await gotoSandbox(page, baseURL)

    await expect(
      byTestId(page, 'sandbox-resource-limits-noperm-alert'),
    ).toBeVisible({ timeout: 30000 })
  })
})
