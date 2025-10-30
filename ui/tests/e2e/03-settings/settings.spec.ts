import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'

// Helper to log in an admin user
async function loginAsAdmin(page: any, baseURL: string) {
  // Create admin if needed
  await page.goto(`${baseURL}/setup`)
  const usernameField = await page.locator('#username').count()

  if (usernameField > 0) {
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  } else {
    // Admin already exists, need to login
    await page.goto(`${baseURL}/auth`)
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.fill('#login_username', 'admin')
    await page.fill('#login_password', 'password123')
    await page.click('button:has-text("Sign In")')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  }
}

test.describe('Settings - General', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to general settings
    await page.goto(`${baseURL}/settings/general`)
    await page.waitForLoadState('networkidle')

    // Wait for settings page to load
    await page.waitForSelector('text=General', { timeout: 30000 })

    // Check accessibility
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to general settings
    await page.goto(`${baseURL}/settings/general`)
    await page.waitForLoadState('networkidle')
    await page.waitForSelector('text=General', { timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await page.waitForSelector('text=General', { timeout: 30000 })

    // Verify dark mode is active
    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    // Check accessibility in dark mode
    await assertNoAccessibilityViolations(page)
  })

  test('should display theme selector', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to general settings
    await page.goto(`${baseURL}/settings/general`)
    await page.waitForLoadState('networkidle')

    // Check for theme selector
    await expect(page.locator('text=Appearance')).toBeVisible()
    await expect(page.locator('text=Theme')).toBeVisible()
  })

  test('should change theme using selector', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to general settings
    await page.goto(`${baseURL}/settings/general`)
    await page.waitForLoadState('networkidle')

    // Find and click the theme selector
    const themeSelect = page.locator('.ant-select').first()
    await themeSelect.click()

    // Select Dark option
    await page.click('text=Dark')

    // Wait for theme to apply
    await page.waitForTimeout(500)

    // Verify dark mode is active
    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)
  })
})
