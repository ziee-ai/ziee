import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'

test.describe('Authentication', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // First create an admin user so we can access auth page
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Navigate to auth page (this will trigger a fresh page load without auth state)
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Check accessibility
    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // First create an admin user so we can access auth page
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Navigate to auth page
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Switch to dark mode
    await setTheme(page, 'dark')
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Verify dark mode is active
    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    // Check accessibility in dark mode
    await assertNoAccessibilityViolations(page)
  })

  test('should display login form by default', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page
    await page.goto(`${baseURL}/auth`)
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Should show Welcome title
    await expect(page.getByRole('heading', { level: 2, name: /welcome/i })).toBeVisible()

    // Should show login form fields
    await expect(page.getByLabel('Username or Email')).toBeVisible()
    await expect(page.getByLabel('Password', { exact: true })).toBeVisible()
    await expect(page.getByRole('button', { name: /^sign in$/i })).toBeVisible()

    // Should show switch to register link
    await expect(page.getByRole('button', { name: /sign up/i })).toBeVisible()
  })

  test('should validate required fields on login form', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page
    await page.goto(`${baseURL}/auth`)
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Try to submit without filling form
    await page.getByRole('button', { name: /^sign in$/i }).click()

    // Should show validation errors
    await expect(page.getByText('Please input your username or email!')).toBeVisible()
    await expect(page.getByText('Please input your password!')).toBeVisible()
  })

  test('should switch to register form', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page
    await page.goto(`${baseURL}/auth`)
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Click Sign Up link
    await page.getByRole('button', { name: /sign up/i }).click()

    // Should show registration form
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()
    await expect(page.getByLabel('Email')).toBeVisible()
    await expect(page.getByLabel('Confirm Password')).toBeVisible()
    await expect(page.getByRole('button', { name: /^sign up$/i })).toBeVisible()
  })

  test('should display registration form fields', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()

    // Wait for registration form
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()

    // Check all fields are present using semantic selectors
    await expect(page.getByLabel('Username')).toBeVisible()
    await expect(page.getByLabel('Email')).toBeVisible()
    await expect(page.getByLabel('Password', { exact: true })).toBeVisible()
    await expect(page.getByLabel('Confirm Password')).toBeVisible()

    // Check labels via getByText
    await expect(page.getByText('Username')).toBeVisible()
    await expect(page.getByText('Email')).toBeVisible()
    await expect(page.getByText('Password').first()).toBeVisible()
    await expect(page.getByText('Confirm Password')).toBeVisible()
  })

  test('should validate username minimum length on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()

    // Fill with short username
    await page.getByLabel('Username').fill('ab')
    await page.getByLabel('Email').fill('test@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')

    // Trigger validation
    await page.getByLabel('Email').click()

    // Should show validation error
    await expect(page.getByText('Username must be at least 3 characters long!')).toBeVisible()
  })

  test('should validate email format on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()

    // Fill with invalid email
    await page.getByLabel('Username').fill('testuser')
    await page.getByLabel('Email').fill('not-an-email')
    await page.getByLabel('Password', { exact: true }).fill('password123')

    // Trigger validation
    await page.getByLabel('Password', { exact: true }).click()

    // Should show validation error
    await expect(page.getByText('Please enter a valid email address!')).toBeVisible()
  })

  test('should validate password minimum length on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()

    // Fill with short password
    await page.getByLabel('Username').fill('testuser')
    await page.getByLabel('Email').fill('test@example.com')
    await page.getByLabel('Password', { exact: true }).fill('pass')
    await page.getByLabel('Confirm Password').fill('pass')

    // Trigger validation
    await page.getByLabel('Confirm Password').click()

    // Should show validation error
    await expect(page.getByText('Password must be at least 6 characters long!')).toBeVisible()
  })

  test('should validate password confirmation match', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()

    // Fill with mismatched passwords
    await page.getByLabel('Username').fill('testuser')
    await page.getByLabel('Email').fill('test@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password456')

    // Trigger validation by blurring the field
    await page.getByLabel('Username').click()

    // Should show validation error
    await expect(page.getByText('Passwords do not match!')).toBeVisible()
  })

  test('should switch back to login form', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()

    // Click Sign In link
    await page.getByRole('button', { name: /^sign in$/i }).click()

    // Should show login form
    await expect(page.getByText('Username or Email')).toBeVisible()
    await expect(page.getByRole('button', { name: /^sign in$/i })).toBeVisible()
  })

  test('should register new user successfully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()

    // Fill registration form
    await page.getByLabel('Username').fill('testuser')
    await page.getByLabel('Email').fill('test@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')

    // Submit form
    await page.getByRole('button', { name: /^sign up$/i }).click()

    // Should redirect to home page after successful registration
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  })

  test('should login with valid credentials', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Register a regular user
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()
    await page.getByLabel('Username').fill('testuser')
    await page.getByLabel('Email').fill('test@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /^sign up$/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out after registration and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Navigate to auth page (this will trigger a fresh page load without auth state)
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Fill login form
    await page.getByLabel('Username or Email').fill('testuser')
    await page.getByLabel('Password', { exact: true }).fill('password123')

    // Submit form
    await page.getByRole('button', { name: /^sign in$/i }).click()

    // Should redirect to home page after successful login
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  })

  test('should login with email instead of username', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Register a regular user
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()
    await page.getByLabel('Username').fill('testuser')
    await page.getByLabel('Email').fill('test@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /^sign up$/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Now visit auth page and login with email
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })

    // Fill login form with email
    await page.getByLabel('Username or Email').fill('test@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')

    // Submit form
    await page.getByRole('button', { name: /^sign in$/i }).click()

    // Should redirect to home page after successful login
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  })

  test('should validate all required fields on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')
    await page.getByRole('button', { name: /create admin account/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Wait for authentication token to be stored
    await page.waitForFunction(
      () => {
        const authStorage = localStorage.getItem('auth-storage')
        if (!authStorage) return false
        try {
          const parsed = JSON.parse(authStorage)
          return parsed.state?.token !== null && parsed.state?.token !== undefined
        } catch {
          return false
        }
      },
      { timeout: 10000 }
    )

    // Clear localStorage/sessionStorage to log out
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.getByLabel('Username or Email').waitFor({ timeout: 30000 })
    await page.getByRole('button', { name: /sign up/i }).click()
    await expect(page.getByRole('heading', { level: 3, name: /create account/i })).toBeVisible()

    // Try to submit without filling form
    await page.getByRole('button', { name: /^sign up$/i }).click()

    // Should show validation errors
    await expect(page.getByText('Please input your username!')).toBeVisible()
    await expect(page.getByText('Please input your email!')).toBeVisible()
    await expect(page.getByText('Please input your password!')).toBeVisible()
    await expect(page.getByText('Please confirm your password!')).toBeVisible()
  })
})
