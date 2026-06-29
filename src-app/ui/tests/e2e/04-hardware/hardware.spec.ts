import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

test.describe('Hardware Settings', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to hardware settings
    await page.goto(`${baseURL}/settings/hardware`)

    // Wait for hardware page to load (title or some content)
    await byTestId(page, 'hardware-os-card').waitFor({ timeout: 30000 })

    // Check accessibility
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to hardware settings
    await page.goto(`${baseURL}/settings/hardware`)
    await byTestId(page, 'hardware-os-card').waitFor({ timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await byTestId(page, 'hardware-os-card').waitFor({ timeout: 30000 })

    // Verify dark mode is active
    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    // Check accessibility in dark mode
    await assertNoAccessibilityViolations(page)
  })

  test('should display hardware information', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to hardware settings
    await page.goto(`${baseURL}/settings/hardware`)
    await byTestId(page, 'hardware-os-card').waitFor({ timeout: 30000 })

    // Wait for hardware data to load by checking for specific content
    await expect(byTestId(page, 'hardware-os-card')).toBeVisible({ timeout: 30000 })

    // Check that hardware info cards rendered.
    await expect(byTestId(page, 'hardware-os-card')).toBeVisible()
    await expect(byTestId(page, 'hardware-cpu-info-card')).toBeVisible()
  })

  test('should display hardware cards with proper styling in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to hardware settings
    await page.goto(`${baseURL}/settings/hardware`)
    await byTestId(page, 'hardware-os-card').waitFor({ timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await byTestId(page, 'hardware-os-card').waitFor({ timeout: 30000 })

    // Verify dark mode is active
    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    // Wait for hardware data to load by checking for specific content
    await expect(byTestId(page, 'hardware-os-card')).toBeVisible({ timeout: 30000 })

    // Check that hardware info cards rendered in dark mode.
    await expect(byTestId(page, 'hardware-os-card')).toBeVisible()
    await expect(byTestId(page, 'hardware-cpu-info-card')).toBeVisible()
  })

  // The "Monitor" button (HardwareMonitorButton) opens the live monitoring
  // surface. On web/embedded it opens `/hardware-monitor` as a separate window;
  // the desktop build swaps in a Tauri window via the same affordance. This
  // exercises the web/embedded path: an admin sees the button and clicking it
  // opens the monitor at /hardware-monitor.
  test('Monitor button opens the live hardware-monitor window', async ({
    page,
    context,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/hardware`)
    await byTestId(page, 'hardware-os-card').waitFor({ timeout: 30000 })

    const monitorButton = byTestId(page, 'hardware-monitor-btn')
    await expect(monitorButton).toBeVisible({ timeout: 15000 })

    // window.open → a new page in the same browser context.
    const [popup] = await Promise.all([
      context.waitForEvent('page'),
      monitorButton.click(),
    ])
    await popup.waitForLoadState('domcontentloaded')
    expect(popup.url()).toContain('/hardware-monitor')
    await popup.close()
  })
})
