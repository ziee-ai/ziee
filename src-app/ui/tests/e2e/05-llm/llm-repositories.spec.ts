import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToRepositoriesPage,
  waitForRepositoriesPageLoad,
  openAddRepositoryDrawer,
  submitRepositoryForm,
  createRepository,
  deleteRepository,
  toggleRepositoryStatus,
  openEditRepositoryDrawer,
  assertRepositoryExists,
  assertRepositoryNotExists,
  assertRepositoryEnabled,
  assertRepositoryDisabled,
  clickTestConnectionFromList,
  clickTestConnectionFromDrawer,
  assertTestConnectionButtonVisible,
  assertTestConnectionButtonInDrawerVisible,
  waitForConnectionTestResult,
} from './helpers/repository-helpers'
import { RepoHealthMock } from './helpers/repository-health-mock'

test.describe('LLM Repositories - List Page', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await setTheme(page, 'dark')
    await page.waitForTimeout(500) // Wait for theme transition

    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    await assertNoAccessibilityViolations(page)
  })

  test('should display repositories page with Add Repository button', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    // Verify we're on the repositories page
    await expect(page).toHaveURL(new RegExp('/settings/llm-repositories'))

    // Verify page title and subtitle
    await expect(page.getByRole('heading', { name: 'LLM Repositories' })).toBeVisible()
    await expect(page.locator('text=Manage your LLM model repositories and their authentication settings')).toBeVisible()

    // Verify Add Repository button exists (icon button with plus)
    await expect(page.locator('button:has([data-icon="plus"])')).toBeVisible()
  })
})

test.describe('LLM Repositories - Create Repository', () => {
  test('should create repository with no authentication', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-repo-none-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://huggingface.co',
      authType: 'none',
      enabled: true,
    })

    // Verify repository appears in list
    await assertRepositoryExists(page, repositoryName)

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should create repository with API key authentication', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-repo-apikey-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'api_key',
      apiKey: 'sk-test-key-123',
      enabled: true,
    })

    // Verify repository appears in list
    await assertRepositoryExists(page, repositoryName)

    // Verify auth type is displayed
    const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
    await expect(repositoryRow.locator('text=API Key')).toBeVisible()

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should create repository with basic auth', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-repo-basic-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'basic_auth',
      username: 'testuser',
      password: 'testpass',
      enabled: true,
    })

    // Verify repository appears in list
    await assertRepositoryExists(page, repositoryName)

    // Verify auth type is displayed
    const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
    await expect(repositoryRow.locator('text=Basic Auth')).toBeVisible()

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should create repository with bearer token', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-repo-bearer-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'bearer_token',
      bearerToken: 'Bearer abc123',
      enabled: true,
    })

    // Verify repository appears in list
    await assertRepositoryExists(page, repositoryName)

    // Verify auth type is displayed
    const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
    await expect(repositoryRow.locator('text=Bearer Token')).toBeVisible()

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should open Add Repository drawer with correct structure', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Verify drawer is open with correct title
    await expect(page.locator('.ant-drawer-title:has-text("Add Repository")')).toBeVisible()

    // Verify Repository Name field
    await expect(page.locator('label:has-text("Repository Name")')).toBeVisible()
    await expect(page.locator('#llm-repository-form_name')).toBeVisible()

    // Verify Repository URL field
    await expect(page.locator('label:has-text("Repository URL")')).toBeVisible()
    await expect(page.locator('#llm-repository-form_url')).toBeVisible()

    // Verify Authentication Type field
    await expect(page.locator('label:has-text("Authentication Type")')).toBeVisible()
    await expect(page.locator('#llm-repository-form_auth_type')).toBeVisible()

    // Verify Enable Repository switch. The form field (#..._enabled) is a
    // HIDDEN Form.Item; the user-visible Switch is a separate local-state
    // control with aria-label "Enable repository".
    await expect(page.locator('label:has-text("Enable Repository")')).toBeVisible()
    await expect(
      page.getByRole('switch', { name: 'Enable repository' }),
    ).toBeVisible()

    // Verify buttons. Drawer submit label was standardised to verb-only
    // (audit I-2): "Add Repository" → "Add". Scope by primary-button
    // class to keep the assertion stable across naming changes.
    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
    await expect(drawer.locator('button:has-text("Cancel")')).toBeVisible()
    await expect(drawer.locator('.ant-btn-primary[type="submit"]')).toBeVisible()

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })

  test('should show auth fields based on auth type selection', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Tall viewport so the drawer's full form fits and the auth_type
    // Select stays in view across all toggles. The default viewport
    // is just tall enough that opening the drawer pushes the Select
    // partially offscreen mid-test.
    await page.setViewportSize({ width: 1280, height: 1400 })

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Fresh locators each call — Form.Item children change as the
    // conditional sections render, which can invalidate cached
    // locators that hold a snapshot of the DOM.
    const authTypeCombobox = () => page
      .locator('.ant-form-item:has-text("Authentication Type")')
      .first()
      .getByRole('combobox')

    const selectAuthType = async (
      value: 'none' | 'api_key' | 'basic_auth' | 'bearer_token',
    ) => {
      // Drive the Select by walking React Fiber up from the Select's
      // input element to find the rc-select's onChange handler. This
      // is the same handler the option's mousedown triggers.
      await page.evaluate((v) => {
        const input = document.getElementById(
          'llm-repository-form_auth_type',
        )
        if (!input) throw new Error('auth_type input not found')
        // Walk Fiber up to find the rc-select wrapper that has the
        // `onChange` prop accepting (value, option) signature.
        const fiberKey = Object.keys(input).find(k => k.startsWith('__reactFiber$'))
        if (!fiberKey) throw new Error('input has no React Fiber')
        let fiber = (input as any)[fiberKey]
        let onChange: ((val: any, opt: any) => void) | null = null
        while (fiber && !onChange) {
          const props = fiber.memoizedProps || fiber.pendingProps
          if (props?.onChange && (props.options || props.children)) {
            onChange = props.onChange
            break
          }
          fiber = fiber.return
        }
        if (!onChange) throw new Error('Could not find Select onChange via Fiber')
        // rc-select onChange signature is (value, option)
        onChange(v, { value: v, label: v })
      }, value)
      await page.waitForTimeout(500) // let conditional fields render
    }

    // Select API Key
    await selectAuthType('api_key')
    await expect(page.locator('label:has-text("API Key")')).toBeVisible()
    await expect(page.locator('#llm-repository-form_api_key')).toBeVisible()

    // Select Basic Authentication
    await selectAuthType('basic_auth')
    await expect(page.locator('label:has-text("Username")')).toBeVisible()
    await expect(page.locator('#llm-repository-form_username')).toBeVisible()
    await expect(page.locator('label:has-text("Password")')).toBeVisible()
    await expect(page.locator('#llm-repository-form_password')).toBeVisible()

    // Select Bearer Token
    await selectAuthType('bearer_token')
    await authTypeCombobox().press('Enter')

    // Verify Bearer Token field appears
    await expect(page.locator('label:has-text("Bearer Token")')).toBeVisible()
    await expect(page.locator('#llm-repository-form_token')).toBeVisible()

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })
})

test.describe('LLM Repositories - Edit Repository', () => {
  test('should edit custom repository', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-edit-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create repository
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
      enabled: true,
    })

    // Open edit drawer
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
    await repositoryRow.locator('button:has-text("Edit")').click()

    // Wait for drawer
    await page.waitForSelector('.ant-drawer-title', { timeout: 30000 })

    // Update URL
    // Use `example.org` (resolvable real domain) — backend's outbound
    // URL validator (A2) rejects unresolvable hosts as potential SSRF.
    await page.fill('#llm-repository-form_url', 'https://example.org')

    // Submit
    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
    await drawer.locator('.ant-btn-primary[type="submit"]').click()
    await page.waitForSelector('text=Repository updated successfully', { timeout: 15000 })

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should edit repository authentication', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-edit-auth-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create repository with no auth
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
      enabled: true,
    })

    // Open edit drawer
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
    await repositoryRow.locator('button:has-text("Edit")').click()
    await page.waitForSelector('.ant-drawer-title', { timeout: 30000 })

    // Change auth type to API key (index 1: none, api_key, basic, bearer)
    // Use keyboard nav — option click is flaky due to AntD animation.
    {
      const combobox = page
        .locator('.ant-form-item:has-text("Authentication Type")')
        .first()
        .getByRole('combobox')
      await combobox.click()
      await page.waitForTimeout(200)
      await combobox.press('Home')
      await combobox.press('ArrowDown')
      await combobox.press('Enter')
      await page.waitForLoadState('load')
    }

    // Fill API key
    await page.fill('#llm-repository-form_api_key', 'new-api-key-123')

    // Submit
    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
    await drawer.locator('.ant-btn-primary[type="submit"]').click()
    await page.waitForSelector('text=Repository updated successfully', { timeout: 15000 })

    // Verify auth type changed
    await expect(repositoryRow.locator('text=API Key')).toBeVisible()

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should not allow editing built-in repository name/url', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    // Look for Hugging Face repository (built-in)
    const hfRepo = page.locator('div').filter({ hasText: /^Hugging Face/ }).first()

    if (await hfRepo.isVisible()) {
      // Open edit drawer
      await hfRepo.locator('button:has-text("Edit")').click()
      await page.waitForSelector('.ant-drawer-title', { timeout: 30000 })

      // Verify name and URL fields are disabled
      await expect(page.locator('#llm-repository-form_name')).toBeDisabled()
      await expect(page.locator('#llm-repository-form_url')).toBeDisabled()

      // Close drawer
      await page.click('button:has-text("Cancel")')
    }
  })

  test('Enable switch OFF in the edit drawer disables the repository', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-edit-disable-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
      enabled: true,
    })
    await assertRepositoryEnabled(page, repositoryName)

    // Open the EDIT drawer and flip the Enable switch OFF. Edit-mode OFF is a
    // minimal PUT (enabled:false, no connection probe) → "Repository disabled".
    await openEditRepositoryDrawer(page, repositoryName)
    const enableSwitch = page.locator('#llm-repository-form_enabled')
    await expect(enableSwitch).toBeChecked()
    await enableSwitch.click()
    await expect(
      page.locator('.ant-message-success', { hasText: 'Repository disabled' }),
    ).toBeVisible({ timeout: 10000 })

    await page.click('button:has-text("Cancel")')
    await assertRepositoryDisabled(page, repositoryName)

    await deleteRepository(page, repositoryName)
  })
})

test.describe('LLM Repositories - Form Validation', () => {
  test('should validate required repository name field', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Try to submit without name
    await submitRepositoryForm(page)

    // Should show validation error
    await expect(page.locator('.ant-form-item-explain-error:has-text("Please enter a repository name")')).toBeVisible()

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })

  test('should validate required repository URL field', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Fill name but not URL
    await page.fill('#llm-repository-form_name', 'Test Repository')

    // Try to submit
    await submitRepositoryForm(page)

    // Should show validation error
    await expect(page.locator('.ant-form-item-explain-error:has-text("Please enter a repository URL")')).toBeVisible()

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })

  test('should validate URL format', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Fill with invalid URL
    await page.fill('#llm-repository-form_name', 'Test Repository')
    await page.fill('#llm-repository-form_url', 'not-a-valid-url')

    // Try to submit
    await submitRepositoryForm(page)

    // Should show validation error
    await expect(page.locator('.ant-form-item-explain-error')).toBeVisible()

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })
})

test.describe('LLM Repositories - Delete Repository', () => {
  test('should delete a custom repository', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-delete-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create repository
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
    })

    // Delete the repository
    await deleteRepository(page, repositoryName)

    // Verify repository is gone
    await assertRepositoryNotExists(page, repositoryName)
  })

  test('should not show delete button for built-in repositories', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    // Look for Hugging Face repository (built-in)
    const hfRepo = page.locator('div').filter({ hasText: /^Hugging Face/ }).first()

    if (await hfRepo.isVisible()) {
      // Built-in repository should NOT have a delete button
      await expect(hfRepo.locator('button:has-text("Delete")')).not.toBeVisible()

      // Should have (Built-in) indicator
      await expect(hfRepo.locator('text=Built-in')).toBeVisible()
    }
  })
})

test.describe('LLM Repositories - Enable/Disable Toggle', () => {
  test('should toggle repository enabled status', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-toggle-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create enabled repository
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
      enabled: true,
    })
    await assertRepositoryEnabled(page, repositoryName)

    // Toggle to disabled
    await toggleRepositoryStatus(page, repositoryName)
    await page.waitForTimeout(500) // Wait for state update
    await assertRepositoryDisabled(page, repositoryName)

    // Toggle back to enabled
    await toggleRepositoryStatus(page, repositoryName)
    await page.waitForTimeout(500)
    await assertRepositoryEnabled(page, repositoryName)

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should create a DISABLED repository and enable it afterwards', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-disabled-create-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create with the enabled switch OFF — the repo lands disabled.
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
      enabled: false,
    })
    await assertRepositoryExists(page, repositoryName)
    await assertRepositoryDisabled(page, repositoryName)

    // Later enable it from the list toggle.
    await toggleRepositoryStatus(page, repositoryName)
    await page.waitForTimeout(500)
    await assertRepositoryEnabled(page, repositoryName)

    // Cleanup
    await deleteRepository(page, repositoryName)
  })
})

test.describe('LLM Repositories - Connection Testing', () => {
  // Get HuggingFace API key from environment
  const HF_API_KEY = process.env.HUGGINGFACE_API_KEY || ''

  test('should show test connection button for repositories', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-connection-button-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create a repository
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://huggingface.co',
      authType: 'none',
      enabled: true,
    })

    // Verify Test button is visible
    await assertTestConnectionButtonVisible(page, repositoryName)

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should successfully test connection with valid HuggingFace credentials', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-connection-hf-success-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create repository with valid HuggingFace credentials
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://huggingface.co',
      authType: 'bearer_token',
      bearerToken: HF_API_KEY,
      authTestEndpoint: 'https://huggingface.co/api/whoami-v2',
      enabled: true,
    })

    // Test connection
    await clickTestConnectionFromList(page, repositoryName)

    // Should show success message
    await waitForConnectionTestResult(page, 'success')

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should fail connection test with invalid API key', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-connection-invalid-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create repository with invalid API key
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://huggingface.co',
      authType: 'bearer_token',
      bearerToken: 'hf_invalid_key_12345',
      authTestEndpoint: 'https://huggingface.co/api/whoami-v2',
      enabled: true,
    })

    // Test connection
    await clickTestConnectionFromList(page, repositoryName)

    // Should show error message (backend now has 10s timeout for faster feedback)
    await waitForConnectionTestResult(page, 'error')

    // Cleanup
    await deleteRepository(page, repositoryName)
  })

  test('should fail connection test with an unreachable URL', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-connection-unreachable-${Date.now()}`

    // The original test used an UNRESOLVABLE hostname, which the outbound
    // URL validator (security A2, F-01/F-03) now rejects at create time to
    // prevent SSRF / DNS-rebinding. Instead use a RESOLVABLE-but-unreachable
    // URL: start the local mock to claim a free 127.0.0.1 port, then dispose
    // it so nothing is listening. 127.0.0.1 resolves, so create-validation
    // (DEV_LOCAL policy in debug builds, allow_localhost: true) accepts it;
    // the connection test then gets ECONNREFUSED — exercising the
    // connection-failure error UI deterministically and fully offline
    // (distinct from the invalid-credentials/401 path covered above).
    const mock = await RepoHealthMock.start()
    const unreachableUrl = mock.url()
    await mock.dispose()

    await loginAsAdmin(page, baseURL)

    await createRepository(page, baseURL, {
      name: repositoryName,
      url: unreachableUrl,
      authType: 'none',
      enabled: true,
    })

    await clickTestConnectionFromList(page, repositoryName)
    await waitForConnectionTestResult(page, 'error')

    await deleteRepository(page, repositoryName)
  })

  test('should show Test Connection button in drawer when form is valid', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Fill in valid repository data
    await page.fill('#llm-repository-form_name', 'Test Repository')
    await page.fill('#llm-repository-form_url', 'https://huggingface.co')

    // Test Connection button should be visible now
    await assertTestConnectionButtonInDrawerVisible(page)

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })

  test('should successfully test connection from drawer with valid credentials', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Fill in repository data with valid HuggingFace credentials
    await page.fill('#llm-repository-form_name', 'Test HF Repository')
    await page.fill('#llm-repository-form_url', 'https://huggingface.co')

    // Select bearer token auth (index 3) via keyboard
    {
      const combobox = page
        .locator('.ant-form-item:has-text("Authentication Type")')
        .first()
        .getByRole('combobox')
      await combobox.click()
      await page.waitForTimeout(200)
      await combobox.press('Home')
      await combobox.press('ArrowDown')
      await combobox.press('ArrowDown')
      await combobox.press('ArrowDown')
      await combobox.press('Enter')
      await page.waitForLoadState('load')
    }

    // Fill valid bearer token
    await page.fill('#llm-repository-form_token', HF_API_KEY)

    // Fill auth test endpoint
    await page.fill('#llm-repository-form_auth_test_api_endpoint', 'https://huggingface.co/api/whoami-v2')

    // Click Test Connection
    await clickTestConnectionFromDrawer(page)

    // Should show success message
    await waitForConnectionTestResult(page, 'success')

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })

  test('should fail connection test from drawer with invalid credentials', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Fill in repository data with invalid credentials
    await page.fill('#llm-repository-form_name', 'Test Invalid Repository')
    await page.fill('#llm-repository-form_url', 'https://huggingface.co')

    // Select bearer token auth (index 3) via keyboard
    {
      const combobox = page
        .locator('.ant-form-item:has-text("Authentication Type")')
        .first()
        .getByRole('combobox')
      await combobox.click()
      await page.waitForTimeout(200)
      await combobox.press('Home')
      await combobox.press('ArrowDown')
      await combobox.press('ArrowDown')
      await combobox.press('ArrowDown')
      await combobox.press('Enter')
      await page.waitForLoadState('load')
    }

    // Fill invalid bearer token
    await page.fill('#llm-repository-form_token', 'hf_invalid_token_xyz')

    // Fill auth test endpoint
    await page.fill('#llm-repository-form_auth_test_api_endpoint', 'https://huggingface.co/api/whoami-v2')

    // Click Test Connection
    await clickTestConnectionFromDrawer(page)

    // Should show error message (backend now has 10s timeout for faster feedback)
    await waitForConnectionTestResult(page, 'error')

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })

  test('should hide Test Connection button without URL', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Fill only name, no URL
    await page.fill('#llm-repository-form_name', 'Test Repository')

    // Test Connection button should not be visible (form validation)
    const testButton = page.locator('.ant-drawer.ant-drawer-open').last().locator('button:has-text("Test Connection")')
    await expect(testButton).not.toBeVisible()

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })
})

test.describe('LLM Repositories - Empty States', () => {
  test('should handle empty repository list gracefully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    // The page should still be functional even if there are repositories
    // (Empty state would show if no repositories exist, but we likely have built-ins)
    await expect(page.locator('button:has([data-icon="plus"])')).toBeVisible()
  })
})
