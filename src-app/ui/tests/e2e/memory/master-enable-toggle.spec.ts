import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * Memory admin — the master MemorySection card's deployment-wide enable toggle
 * (MemorySection.tsx). Memory ships ON by default deployment-wide (migration 56);
 * toggling the master switch off and saving persists
 * `memory_admin_settings.enabled = false`.
 */
test.describe('Memory admin — master enable toggle', () => {
  test('toggling the master switch off and saving persists enabled=false', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/memory-admin`)
    const masterSwitch = byTestId(page, 'memory-admin-enabled-switch')
    await expect(masterSwitch).toBeVisible({ timeout: 20000 })

    // Memory is ON by default deployment-wide (migration 56 documents this
    // intentionally; per-user extraction/retrieval stay opt-in). Verify the
    // master toggle persists a CHANGE by flipping it OFF and saving.
    await expect(masterSwitch).toHaveAttribute('aria-checked', 'true')

    // Turn it off and save.
    await masterSwitch.click()
    await expect(masterSwitch).toHaveAttribute('aria-checked', 'false')
    await byTestId(page, 'memory-admin-master-save-btn').click()
    await expect(page.locator('[data-sonner-toast]')).toContainText(
      'Memory settings saved.',
    )

    // Server persisted the master toggle.
    const token = await getAdminToken(apiURL)
    const res = await page.request.get(`${apiURL}/api/memory/admin-settings`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    expect((await res.json()).enabled).toBe(false)
  })
})
