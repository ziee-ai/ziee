import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'

test.describe('LLM Providers', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to LLM providers settings
    await page.goto(`${baseURL}/settings/llm-providers`)
    await page.waitForLoadState('networkidle')

    // Wait for page to load
    await page.waitForSelector('text=LLM Providers', { timeout: 30000 })

    // Check accessibility
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to LLM providers settings
    await page.goto(`${baseURL}/settings/llm-providers`)
    await page.waitForLoadState('networkidle')
    await page.waitForSelector('text=LLM Providers', { timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await page.waitForSelector('text=LLM Providers', { timeout: 30000 })

    // Verify dark mode is active
    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    // Check accessibility in dark mode
    await assertNoAccessibilityViolations(page)
  })
})

test.describe('LLM Repositories', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to LLM repositories settings
    await page.goto(`${baseURL}/settings/llm-repositories`)
    await page.waitForLoadState('networkidle')

    // Wait for page to load
    await page.waitForSelector('text=LLM Repositories', { timeout: 30000 })

    // Check accessibility
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Login as admin
    await loginAsAdmin(page, baseURL)

    // Navigate to LLM repositories settings
    await page.goto(`${baseURL}/settings/llm-repositories`)
    await page.waitForLoadState('networkidle')
    await page.waitForSelector('text=LLM Repositories', { timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await page.waitForSelector('text=LLM Repositories', { timeout: 30000 })

    // Verify dark mode is active
    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    // Check accessibility in dark mode
    await assertNoAccessibilityViolations(page)
  })
})
