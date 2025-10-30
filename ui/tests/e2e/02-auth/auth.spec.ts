import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'

test.describe('Authentication', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // First create an admin user so we can access auth page
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Navigate to auth page (this will trigger a fresh page load without auth state)
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })

    // Check accessibility
    await assertNoAccessibilityViolations(page)
  })

  test('should display login form by default', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page
    await page.goto(`${baseURL}/auth`)
    await page.waitForSelector('#login_username', { timeout: 30000 })

    // Should show Welcome title
    await expect(page.locator('h2')).toContainText('Welcome')

    // Should show login form fields
    await expect(page.locator('#login_username')).toBeVisible()
    await expect(page.locator('#login_password')).toBeVisible()
    await expect(page.getByRole('button', { name: /sign in/i })).toBeVisible()

    // Should show switch to register link
    await expect(page.getByRole('button', { name: /sign up/i })).toBeVisible()
  })

  test('should validate required fields on login form', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page
    await page.goto(`${baseURL}/auth`)
    await page.waitForSelector('#login_username', { timeout: 30000 })

    // Try to submit without filling form
    await page.click('button:has-text("Sign In")')

    // Should show validation errors
    await expect(page.locator('text=Please input your username or email!')).toBeVisible()
    await expect(page.locator('text=Please input your password!')).toBeVisible()
  })

  test('should switch to register form', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page
    await page.goto(`${baseURL}/auth`)
    await page.waitForSelector('#login_username', { timeout: 30000 })

    // Click Sign Up link
    await page.click('button:has-text("Sign Up")')

    // Should show registration form
    await expect(page.locator('h3')).toContainText('Create Account')
    await expect(page.locator('#register_email')).toBeVisible()
    await expect(page.locator('#register_confirmPassword')).toBeVisible()
    await expect(page.getByRole('button', { name: /sign up/i })).toBeVisible()
  })

  test('should display registration form fields', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')

    // Wait for registration form
    await expect(page.locator('h3')).toContainText('Create Account')

    // Check all fields are present
    await expect(page.locator('#register_username')).toBeVisible()
    await expect(page.locator('#register_email')).toBeVisible()
    await expect(page.locator('#register_password')).toBeVisible()
    await expect(page.locator('#register_confirmPassword')).toBeVisible()

    // Check labels
    await expect(page.locator('label[for="register_username"]')).toBeVisible()
    await expect(page.locator('label[for="register_email"]')).toBeVisible()
    await expect(page.locator('label[for="register_password"]')).toBeVisible()
    await expect(page.locator('label[for="register_confirmPassword"]')).toBeVisible()
  })

  test('should validate username minimum length on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')
    await expect(page.locator('h3')).toContainText('Create Account')

    // Fill with short username
    await page.fill('#register_username', 'ab')
    await page.fill('#register_email', 'test@example.com')
    await page.fill('#register_password', 'password123')

    // Trigger validation
    await page.click('#register_email')

    // Should show validation error
    await expect(page.locator('text=Username must be at least 3 characters long!')).toBeVisible()
  })

  test('should validate email format on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')
    await expect(page.locator('h3')).toContainText('Create Account')

    // Fill with invalid email
    await page.fill('#register_username', 'testuser')
    await page.fill('#register_email', 'not-an-email')
    await page.fill('#register_password', 'password123')

    // Trigger validation
    await page.click('#register_password')

    // Should show validation error
    await expect(page.locator('text=Please enter a valid email address!')).toBeVisible()
  })

  test('should validate password minimum length on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')
    await expect(page.locator('h3')).toContainText('Create Account')

    // Fill with short password
    await page.fill('#register_username', 'testuser')
    await page.fill('#register_email', 'test@example.com')
    await page.fill('#register_password', 'pass')
    await page.fill('#register_confirmPassword', 'pass')

    // Trigger validation
    await page.click('#register_confirmPassword')

    // Should show validation error
    await expect(page.locator('text=Password must be at least 6 characters long!')).toBeVisible()
  })

  test('should validate password confirmation match', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')
    await expect(page.locator('h3')).toContainText('Create Account')

    // Fill with mismatched passwords
    await page.fill('#register_username', 'testuser')
    await page.fill('#register_email', 'test@example.com')
    await page.fill('#register_password', 'password123')
    await page.fill('#register_confirmPassword', 'password456')

    // Trigger validation by blurring the field
    await page.click('#register_username')

    // Should show validation error
    await expect(page.locator('text=Passwords do not match!')).toBeVisible()
  })

  test('should switch back to login form', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')
    await expect(page.locator('h3')).toContainText('Create Account')

    // Click Sign In link
    await page.click('button:has-text("Sign In")')

    // Should show login form
    await expect(page.locator('label:has-text("Username or Email")')).toBeVisible()
    await expect(page.getByRole('button', { name: /^sign in$/i })).toBeVisible()
  })

  test('should register new user successfully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')
    await expect(page.locator('h3')).toContainText('Create Account')

    // Fill registration form
    await page.fill('#register_username', 'testuser')
    await page.fill('#register_email', 'test@example.com')
    await page.fill('#register_password', 'password123')
    await page.fill('#register_confirmPassword', 'password123')

    // Submit form
    await page.click('button:has-text("Sign Up")')

    // Should redirect to home page after successful registration
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  })

  test('should login with valid credentials', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Register a regular user
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')
    await expect(page.locator('h3')).toContainText('Create Account')
    await page.fill('#register_username', 'testuser')
    await page.fill('#register_email', 'test@example.com')
    await page.fill('#register_password', 'password123')
    await page.fill('#register_confirmPassword', 'password123')
    await page.click('button:has-text("Sign Up")')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out after registration and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Navigate to auth page (this will trigger a fresh page load without auth state)
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })

    // Fill login form
    await page.fill('#login_username', 'testuser')
    await page.fill('#login_password', 'password123')

    // Submit form
    await page.click('button:has-text("Sign In")')

    // Should redirect to home page after successful login
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  })

  test('should login with email instead of username', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Register a regular user
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')
    await expect(page.locator('h3')).toContainText('Create Account')
    await page.fill('#register_username', 'testuser')
    await page.fill('#register_email', 'test@example.com')
    await page.fill('#register_password', 'password123')
    await page.fill('#register_confirmPassword', 'password123')
    await page.click('button:has-text("Sign Up")')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Now visit auth page and login with email
    await page.goto(`${baseURL}/auth`)
    await page.waitForSelector('#login_username', { timeout: 30000 })

    // Fill login form with email
    await page.fill('#login_username', 'test@example.com')
    await page.fill('#login_password', 'password123')

    // Submit form
    await page.click('button:has-text("Sign In")')

    // Should redirect to home page after successful login
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  })

  test('should validate all required fields on registration', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Create admin first
    await page.goto(`${baseURL}/setup`)
    await page.waitForSelector('#username', { timeout: 30000 })
    await page.fill('#username', 'admin')
    await page.fill('#email', 'admin@example.com')
    await page.fill('#password', 'password123')
    await page.fill('#confirm_password', 'password123')
    await page.click('button[type="submit"]')
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })

    // Clear localStorage/sessionStorage to log out and navigate directly to auth
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })

    // Visit auth page and switch to register
    await page.goto(`${baseURL}/auth`, { waitUntil: 'networkidle' })
    await page.waitForSelector('#login_username', { timeout: 30000 })
    await page.click('button:has-text("Sign Up")')
    await expect(page.locator('h3')).toContainText('Create Account')

    // Try to submit without filling form
    await page.click('button:has-text("Sign Up")')

    // Should show validation errors
    await expect(page.locator('text=Please input your username!')).toBeVisible()
    await expect(page.locator('text=Please input your email!')).toBeVisible()
    await expect(page.locator('text=Please input your password!')).toBeVisible()
    await expect(page.locator('text=Please confirm your password!')).toBeVisible()
  })
})
