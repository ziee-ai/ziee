import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * Memory admin — the master MemorySection card's deployment-wide enable toggle
 * (MemorySection.tsx). Memory ships OFF by default; flipping the master switch
 * on and saving persists `memory_admin_settings.enabled = true`.
 */
test.describe('Memory admin — master enable toggle', () => {
  test('flipping the master switch on and saving persists enabled=true', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/memory-admin`)
    const masterSwitch = byTestId(page, 'memory-admin-enabled-switch')
    await expect(masterSwitch).toBeVisible({ timeout: 20000 })

    // Default: memory is disabled.
    await expect(masterSwitch).toHaveAttribute('aria-checked', 'false')

    // Turn it on and save.
    await masterSwitch.click()
    await expect(masterSwitch).toHaveAttribute('aria-checked', 'true')
    await byTestId(page, 'memory-admin-master-save-btn').click()
    await expect(page.locator('[data-sonner-toast]')).toContainText(
      'Memory settings saved.',
    )

    // Server persisted the master toggle.
    const token = await getAdminToken(apiURL)
    const res = await page.request.get(`${apiURL}/api/memory/admin-settings`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    expect((await res.json()).enabled).toBe(true)
  })
})
