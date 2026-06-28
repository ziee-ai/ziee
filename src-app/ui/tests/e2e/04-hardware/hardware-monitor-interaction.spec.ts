import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — Hardware settings USER INTERACTION (not just a page-load smoke test).
 *
 * The "Monitor" button in the Hardware settings page title
 * (HardwareMonitorButton.tsx) opens the real-time monitor popup at
 * `/hardware-monitor`. This drives that click and asserts the popup
 * actually navigates + renders the monitor view — exercising behaviour
 * the existing smoke tests never touch.
 */

test.describe('Hardware Settings — monitor interaction', () => {
  test('clicking Monitor opens the hardware-monitor popup', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/hardware`)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    const monitorButton = page.getByRole('button', { name: 'Monitor' })
    await expect(monitorButton).toBeVisible()

    const [popup] = await Promise.all([
      page.waitForEvent('popup'),
      monitorButton.click(),
    ])

    await popup.waitForLoadState('domcontentloaded')
    await expect(popup).toHaveURL(/\/hardware-monitor$/)
    // The popup renders the dedicated monitor view (sr-only h1 in
    // HardwareMonitor.tsx).
    await expect(
      popup.getByRole('heading', { name: 'Hardware Monitor' }),
    ).toBeAttached({ timeout: 30000 })

    await popup.close()
  })
})
