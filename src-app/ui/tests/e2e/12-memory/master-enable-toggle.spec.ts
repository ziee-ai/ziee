import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

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
    const memoryCard = page
      .locator('.ant-card')
      .filter({ hasText: 'Enable memory deployment-wide' })
    await expect(memoryCard).toBeVisible({ timeout: 20000 })

    // Default: memory is disabled.
    const masterSwitch = memoryCard.getByRole('switch', {
      name: 'Enable memory deployment-wide',
    })
    await expect(masterSwitch).toHaveAttribute('aria-checked', 'false')

    // Turn it on and save.
    await masterSwitch.click()
    await expect(masterSwitch).toHaveAttribute('aria-checked', 'true')
    await memoryCard.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Memory settings saved.')).toBeVisible()

    // Server persisted the master toggle.
    const token = await getAdminToken(apiURL)
    const res = await page.request.get(`${apiURL}/api/memory/admin-settings`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    expect((await res.json()).enabled).toBe(true)
  })
})
