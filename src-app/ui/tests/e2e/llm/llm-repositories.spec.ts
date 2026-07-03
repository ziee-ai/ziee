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
import { byTestId } from '../testid'
import { RepoHealthMock } from './helpers/repository-health-mock'
import type { Page } from '@playwright/test'

// A repository row scoped by the (dynamic) repository name it contains.
const repoRow = (page: Page, name: string) =>
  page.locator('[data-testid^="llmrepo-row-"]').filter({ hasText: name }).first()

// Drive the auth-type kit Select (trigger then option-by-value).
async function selectAuthType(
  page: Page,
  value: 'none' | 'api_key' | 'basic_auth' | 'bearer_token',
) {
  await byTestId(page, 'llmrepo-form-auth-type').click()
  await byTestId(page, `llmrepo-form-auth-type-opt-${value}`).click()
  await page.waitForLoadState('load')
}

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

    // Verify the repositories card + Add Repository button render.
    await expect(byTestId(page, 'llmrepo-card')).toBeVisible()
    await expect(byTestId(page, 'llmrepo-add-btn')).toBeVisible()
  })
})

test.describe('LLM Repositories - Create Repository', () => {
  test('should create repository with no authentication', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-repo-none-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: `https://huggingface.co/?r=${repositoryName}`,
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

    await assertRepositoryExists(page, repositoryName)

    // Verify auth type is displayed in the row.
    await expect(repoRow(page, repositoryName)).toContainText('API Key')

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

    await assertRepositoryExists(page, repositoryName)
    await expect(repoRow(page, repositoryName)).toContainText('Basic Auth')

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

    await assertRepositoryExists(page, repositoryName)
    await expect(repoRow(page, repositoryName)).toContainText('Bearer Token')

    await deleteRepository(page, repositoryName)
  })

  test('should open Add Repository drawer with correct structure', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // Verify the form + its fields render.
    await expect(byTestId(page, 'llmrepo-form')).toBeVisible()
    await expect(byTestId(page, 'llmrepo-form-name')).toBeVisible()
    await expect(byTestId(page, 'llmrepo-form-url')).toBeVisible()
    await expect(byTestId(page, 'llmrepo-form-auth-type')).toBeVisible()
    await expect(byTestId(page, 'llmrepo-form-enabled-switch')).toBeVisible()

    // Verify footer buttons.
    await expect(byTestId(page, 'llmrepo-form-cancel-btn')).toBeVisible()
    await expect(byTestId(page, 'llmrepo-form-submit-btn')).toBeVisible()

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })

  test('should show auth fields based on auth type selection', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await page.setViewportSize({ width: 1280, height: 1400 })

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    // API Key
    await selectAuthType(page, 'api_key')
    await expect(byTestId(page, 'llmrepo-form-api-key')).toBeVisible()

    // Basic Authentication
    await selectAuthType(page, 'basic_auth')
    await expect(byTestId(page, 'llmrepo-form-username')).toBeVisible()
    await expect(byTestId(page, 'llmrepo-form-password')).toBeVisible()

    // Bearer Token
    await selectAuthType(page, 'bearer_token')
    await expect(byTestId(page, 'llmrepo-form-token')).toBeVisible()

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })
})

test.describe('LLM Repositories - Edit Repository', () => {
  test('should edit custom repository', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-edit-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
      enabled: true,
    })

    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await repoRow(page, repositoryName).locator('[data-testid^="llmrepo-edit-btn-"]').first().click()
    await byTestId(page, 'llmrepo-form').waitFor({ timeout: 30000 })

    // Update URL (example.org is resolvable; the SSRF validator rejects unresolvable hosts).
    await byTestId(page, 'llmrepo-form-url').fill('https://example.org')

    const [resp] = await Promise.all([
      page.waitForResponse(
        r => /\/api\/llm-repositories\/[0-9a-f-]+/.test(r.url()) && r.request().method() === 'POST',
        { timeout: 15000 }
      ),
      byTestId(page, 'llmrepo-form-submit-btn').click(),
    ])
    expect(resp.ok()).toBeTruthy()

    await deleteRepository(page, repositoryName)
  })

  test('should edit repository authentication', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-edit-auth-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
      enabled: true,
    })

    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await repoRow(page, repositoryName).locator('[data-testid^="llmrepo-edit-btn-"]').first().click()
    await byTestId(page, 'llmrepo-form').waitFor({ timeout: 30000 })

    // Change auth type to API key.
    await selectAuthType(page, 'api_key')
    await byTestId(page, 'llmrepo-form-api-key').fill('new-api-key-123')

    const [resp] = await Promise.all([
      page.waitForResponse(
        r => /\/api\/llm-repositories\/[0-9a-f-]+/.test(r.url()) && r.request().method() === 'POST',
        { timeout: 15000 }
      ),
      byTestId(page, 'llmrepo-form-submit-btn').click(),
    ])
    expect(resp.ok()).toBeTruthy()

    // Verify auth type changed.
    await expect(repoRow(page, repositoryName)).toContainText('API Key')

    await deleteRepository(page, repositoryName)
  })

  test('should not allow editing built-in repository name/url', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    const hfRepo = repoRow(page, 'Hugging Face')

    if (await hfRepo.isVisible()) {
      await hfRepo.locator('[data-testid^="llmrepo-edit-btn-"]').first().click()
      await byTestId(page, 'llmrepo-form').waitFor({ timeout: 30000 })

      // Name and URL fields are disabled for built-ins.
      await expect(byTestId(page, 'llmrepo-form-name')).toBeDisabled()
      await expect(byTestId(page, 'llmrepo-form-url')).toBeDisabled()

      await byTestId(page, 'llmrepo-form-cancel-btn').click()
    }
  })

  test('edit drawer pre-fills the existing auth fields', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-prefill-${Date.now()}`
    const username = 'prefilluser123'
    // A resolvable host: the backend DNS-validates auth_test_api_endpoint on
    // create (a non-resolving host → 400). The value just needs to round-trip.
    const endpoint = 'https://example.com/whoami'

    await loginAsAdmin(page, baseURL)
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'basic_auth',
      username,
      password: 'secretpw',
      authTestEndpoint: endpoint,
    })

    // Re-open the EDIT drawer — the form must rehydrate from the saved row.
    await openEditRepositoryDrawer(page, repositoryName)

    await expect(byTestId(page, 'llmrepo-form-name')).toHaveValue(repositoryName)
    await expect(byTestId(page, 'llmrepo-form-url')).toHaveValue('https://example.com')
    // Username + the auth-test endpoint are pre-filled (password is NOT echoed).
    await expect(byTestId(page, 'llmrepo-form-username')).toHaveValue(username)
    await expect(byTestId(page, 'llmrepo-form-auth-test-endpoint')).toHaveValue(endpoint)

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
    await deleteRepository(page, repositoryName)
  })

  test('Enable switch OFF in the edit drawer disables the repository', async ({ page, testInfra }) => {
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

    // Open the EDIT drawer and flip the Enable switch OFF (minimal PUT, no probe).
    await openEditRepositoryDrawer(page, repositoryName)
    const enableSwitch = byTestId(page, 'llmrepo-form-enabled-switch')
    await expect(enableSwitch).toBeChecked()
    const [resp] = await Promise.all([
      page.waitForResponse(
        r => /\/api\/llm-repositories\/[0-9a-f-]+/.test(r.url()) && r.request().method() === 'POST',
        { timeout: 10000 }
      ),
      enableSwitch.click(),
    ])
    expect(resp.ok()).toBeTruthy()

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
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

    // Try to submit without name.
    await submitRepositoryForm(page)

    // Should show a validation error (FieldError carries role="alert").
    await expect(
      byTestId(page, 'llmrepo-form').getByRole('alert').filter({ hasText: 'repository name' })
    ).toBeVisible()

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })

  test('should validate required repository URL field', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    await byTestId(page, 'llmrepo-form-name').fill('Test Repository')
    await submitRepositoryForm(page)

    await expect(
      byTestId(page, 'llmrepo-form').getByRole('alert').filter({ hasText: 'repository URL' })
    ).toBeVisible()

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })

  test('should validate URL format', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    await byTestId(page, 'llmrepo-form-name').fill('Test Repository')
    await byTestId(page, 'llmrepo-form-url').fill('not-a-valid-url')
    await submitRepositoryForm(page)

    await expect(byTestId(page, 'llmrepo-form').getByRole('alert').first()).toBeVisible()

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })
})

test.describe('LLM Repositories - Delete Repository', () => {
  test('should delete a custom repository', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-delete-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
    })

    await deleteRepository(page, repositoryName)

    await assertRepositoryNotExists(page, repositoryName)
  })

  test('should not show delete button for built-in repositories', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    const hfRepo = repoRow(page, 'Hugging Face')

    if (await hfRepo.isVisible()) {
      // Built-in repository should NOT have a delete button.
      await expect(hfRepo.locator('[data-testid^="llmrepo-delete-btn-"]')).not.toBeVisible()
      // Should have a (Built-in) indicator.
      await expect(hfRepo.filter({ hasText: 'Built-in' })).toBeVisible()
    }
  })
})

test.describe('LLM Repositories - Enable/Disable Toggle', () => {
  test('should toggle repository enabled status', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-toggle-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
      enabled: true,
    })
    await assertRepositoryEnabled(page, repositoryName)

    await toggleRepositoryStatus(page, repositoryName)
    await page.waitForTimeout(500)
    await assertRepositoryDisabled(page, repositoryName)

    await toggleRepositoryStatus(page, repositoryName)
    await page.waitForTimeout(500)
    await assertRepositoryEnabled(page, repositoryName)

    await deleteRepository(page, repositoryName)
  })

  test('should create a DISABLED repository and enable it afterwards', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-disabled-create-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://example.com',
      authType: 'none',
      enabled: false,
    })
    await assertRepositoryExists(page, repositoryName)
    await assertRepositoryDisabled(page, repositoryName)

    await toggleRepositoryStatus(page, repositoryName)
    await page.waitForTimeout(500)
    await assertRepositoryEnabled(page, repositoryName)

    await deleteRepository(page, repositoryName)
  })
})

test.describe('LLM Repositories - Connection Testing', () => {
  test('the built-in Hugging Face repo exposes a working Test button', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Mock the probe so the built-in repo's Test button is deterministic.
    await page.route(/\/api\/llm-repositories(\/[0-9a-f-]+)?\/test$/, async (route, req) => {
      if (req.method() === 'POST') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ success: true, message: 'Connection successful' }),
        })
      }
      return route.continue()
    })

    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await assertTestConnectionButtonVisible(page, 'Hugging Face Hub')
    await clickTestConnectionFromList(page, 'Hugging Face Hub')
    await waitForConnectionTestResult(page, 'success')
  })

  const HF_API_KEY = process.env.HUGGINGFACE_API_KEY || ''

  test('should show test connection button for repositories', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-connection-button-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    await createRepository(page, baseURL, {
      name: repositoryName,
      url: `https://huggingface.co/?r=${repositoryName}`,
      authType: 'none',
      enabled: true,
    })

    await assertTestConnectionButtonVisible(page, repositoryName)

    await deleteRepository(page, repositoryName)
  })

  test('should successfully test connection with valid HuggingFace credentials', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-connection-hf-success-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    await createRepository(page, baseURL, {
      name: repositoryName,
      url: `https://huggingface.co/?r=${repositoryName}`,
      authType: 'bearer_token',
      bearerToken: HF_API_KEY,
      authTestEndpoint: 'https://huggingface.co/api/whoami-v2',
      enabled: true,
    })

    await clickTestConnectionFromList(page, repositoryName)
    await waitForConnectionTestResult(page, 'success')

    await deleteRepository(page, repositoryName)
  })

  test('should fail connection test with invalid API key', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-connection-invalid-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    await createRepository(page, baseURL, {
      name: repositoryName,
      url: `https://huggingface.co/?r=${repositoryName}`,
      authType: 'bearer_token',
      bearerToken: 'hf_invalid_key_12345',
      authTestEndpoint: 'https://huggingface.co/api/whoami-v2',
      enabled: true,
    })

    await clickTestConnectionFromList(page, repositoryName)
    await waitForConnectionTestResult(page, 'error')

    await deleteRepository(page, repositoryName)
  })

  test('should fail connection test with an unreachable URL', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const repositoryName = `test-connection-unreachable-${Date.now()}`

    // Resolvable-but-unreachable URL: claim a free 127.0.0.1 port then dispose
    // it so the connection test gets ECONNREFUSED deterministically.
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

    await byTestId(page, 'llmrepo-form-name').fill('Test Repository')
    await byTestId(page, 'llmrepo-form-url').fill('https://huggingface.co')

    await assertTestConnectionButtonInDrawerVisible(page)

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })

  test('should successfully test connection from drawer with valid credentials', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    await byTestId(page, 'llmrepo-form-name').fill('Test HF Repository')
    await byTestId(page, 'llmrepo-form-url').fill('https://huggingface.co')

    await selectAuthType(page, 'bearer_token')
    await byTestId(page, 'llmrepo-form-token').fill(HF_API_KEY)
    await byTestId(page, 'llmrepo-form-auth-test-endpoint').fill('https://huggingface.co/api/whoami-v2')

    await clickTestConnectionFromDrawer(page)
    await waitForConnectionTestResult(page, 'success')

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })

  test('should fail connection test from drawer with invalid credentials', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    await byTestId(page, 'llmrepo-form-name').fill('Test Invalid Repository')
    await byTestId(page, 'llmrepo-form-url').fill('https://huggingface.co')

    await selectAuthType(page, 'bearer_token')
    await byTestId(page, 'llmrepo-form-token').fill('hf_invalid_token_xyz')
    await byTestId(page, 'llmrepo-form-auth-test-endpoint').fill('https://huggingface.co/api/whoami-v2')

    await clickTestConnectionFromDrawer(page)
    await waitForConnectionTestResult(page, 'error')

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })

  test('should hide Test Connection button without URL', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await openAddRepositoryDrawer(page)

    await byTestId(page, 'llmrepo-form-name').fill('Test Repository')

    // Test Connection button should not be visible (form validation).
    await expect(byTestId(page, 'llmrepo-form-test-btn')).not.toBeVisible()

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })
})

test.describe('LLM Repositories - Empty States', () => {
  test('should handle empty repository list gracefully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    await expect(byTestId(page, 'llmrepo-add-btn')).toBeVisible()
  })

  test('renders the Empty state when no repositories exist', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Force an empty list to reach the Empty component.
    await page.route(/\/api\/llm-repositories(\?.*)?$/, async (route, req) => {
      if (req.method() === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ repositories: [], page: 1, per_page: 20, total: 0 }),
        })
      }
      return route.continue()
    })

    await goToRepositoriesPage(page, baseURL)
    await expect(byTestId(page, 'llmrepo-empty')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'llmrepo-empty')).toContainText('No repositories yet')
  })
})
