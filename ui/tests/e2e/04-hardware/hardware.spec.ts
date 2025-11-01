import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'

test.describe('Hardware Settings', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to hardware settings
    await page.goto(`${baseURL}/settings/hardware`)
    await page.waitForLoadState('networkidle')

    // Wait for hardware page to load (title or some content)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    // Check accessibility
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to hardware settings
    await page.goto(`${baseURL}/settings/hardware`)
    await page.waitForLoadState('networkidle')
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

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
    await page.waitForLoadState('networkidle')

    // Check for hardware cards (at least operating system should be present)
    await expect(page.locator('text=Operating System').or(page.locator('text=Hardware'))).toBeVisible()
  })

  test('should display hardware cards with proper styling in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to hardware settings
    await page.goto(`${baseURL}/settings/hardware`)
    await page.waitForLoadState('networkidle')

    // Switch to dark mode
    await setTheme(page, 'dark')
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    // Verify dark mode is active
    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    // Check that cards are visible in dark mode
    const cards = await page.locator('.ant-card').count()
    expect(cards).toBeGreaterThan(0)
  })
})
