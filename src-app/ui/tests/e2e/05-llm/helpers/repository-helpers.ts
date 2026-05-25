import { Page, expect } from '@playwright/test'

/**
 * LLM Repository CRUD helpers
 */

export interface RepositoryFormData {
  name: string
  url: string
  authType: 'none' | 'api_key' | 'basic_auth' | 'bearer_token'
  enabled?: boolean
  // Auth fields
  apiKey?: string
  username?: string
  password?: string
  bearerToken?: string
  authTestEndpoint?: string
}

// =====================================================
// Navigation
// =====================================================

export async function goToRepositoriesPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/llm-repositories`)
  await page.waitForLoadState('networkidle')
}

export async function waitForRepositoriesPageLoad(page: Page) {
  // Wait for the repositories page to load
  await page.waitForSelector('text=LLM Repositories', { timeout: 30000 })
  await page.waitForLoadState('networkidle')
}

// =====================================================
// Repository Creation
// =====================================================

export async function openAddRepositoryDrawer(page: Page) {
  // Click Add Repository button (icon button with plus icon in the card header)
  await page.click('button:has([data-icon="plus"])')
  // Wait for the drawer to open
  await page.waitForSelector('.ant-drawer-title:has-text("Add Repository")', { timeout: 30000 })
}

export async function fillRepositoryForm(page: Page, data: RepositoryFormData) {
  // Fill name
  await page.fill('#llm-repository-form_name', data.name)

  // Fill URL
  await page.fill('#llm-repository-form_url', data.url)

  // Select auth type - click the container, not the input
  await page.click('.ant-select:has(#llm-repository-form_auth_type)')
  await page.waitForSelector('.ant-select-dropdown', { state: 'visible' })

  const authTypeLabels: Record<RepositoryFormData['authType'], string> = {
    'none': 'No Authentication',
    'api_key': 'API Key',
    'basic_auth': 'Basic Authentication',
    'bearer_token': 'Bearer Token',
  }

  await page.click(`.ant-select-item-option:has-text("${authTypeLabels[data.authType]}")`)
  await page.waitForLoadState('networkidle')

  // Fill auth fields based on type
  if (data.authType === 'api_key' && data.apiKey) {
    await page.fill('#llm-repository-form_api_key', data.apiKey)
  }

  if (data.authType === 'basic_auth') {
    if (data.username) {
      await page.fill('#llm-repository-form_username', data.username)
    }
    if (data.password) {
      await page.fill('#llm-repository-form_password', data.password)
    }
  }

  if (data.authType === 'bearer_token' && data.bearerToken) {
    await page.fill('#llm-repository-form_token', data.bearerToken)
  }

  // Fill optional auth test endpoint
  if (data.authTestEndpoint) {
    await page.fill('#llm-repository-form_auth_test_api_endpoint', data.authTestEndpoint)
  }

  // Handle enabled toggle if specified
  if (data.enabled !== undefined) {
    const toggle = page.locator('#llm-repository-form_enabled')
    const isChecked = await toggle.isChecked()
    if (isChecked !== data.enabled) {
      await toggle.click()
    }
  }
}

export async function submitRepositoryForm(page: Page) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
  await drawer.locator('button:has-text("Add Repository")').click()
  await page.waitForLoadState('networkidle')
}

export async function createRepository(
  page: Page,
  baseURL: string,
  data: RepositoryFormData
): Promise<void> {
  await goToRepositoriesPage(page, baseURL)
  await waitForRepositoriesPageLoad(page)

  await openAddRepositoryDrawer(page)
  await fillRepositoryForm(page, data)
  await submitRepositoryForm(page)

  // Wait for success message
  await page.waitForSelector('text=Repository added successfully', { timeout: 15000 })

  // Verify repository appears in list
  await expect(page.locator(`text=${data.name}`).first()).toBeVisible()
}

// =====================================================
// Repository Editing
// =====================================================

export async function openEditRepositoryDrawer(page: Page, repositoryName: string) {
  // Find the repository by name and click its edit button
  // Repositories are in a list, each with name, URL, and action buttons
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const editButton = repositoryRow.locator('button:has-text("Edit")')
  await editButton.click()

  // Wait for drawer - could be "Edit Repository" or "Edit Built-in Repository"
  await page.waitForSelector('.ant-drawer-title', { timeout: 30000 })
}

export async function updateRepositoryForm(page: Page) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
  await drawer.locator('button:has-text("Update Repository")').click()
  await page.waitForLoadState('networkidle')
}

export async function updateRepository(
  page: Page,
  repositoryName: string,
  updates: Partial<RepositoryFormData>
): Promise<void> {
  await openEditRepositoryDrawer(page, repositoryName)
  await fillRepositoryForm(page, updates as RepositoryFormData)
  await updateRepositoryForm(page)
  await page.waitForSelector('text=Repository updated successfully', { timeout: 15000 })
}

// =====================================================
// Repository Deletion
// =====================================================

export async function openDeleteRepositoryDialog(page: Page, repositoryName: string) {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const deleteButton = repositoryRow.locator('button:has-text("Delete")')
  await deleteButton.click()

  // Wait for confirmation popconfirm
  await page.waitForSelector('.ant-popover:visible', { timeout: 5000 })
}

export async function confirmDeleteRepository(page: Page) {
  // Popconfirm uses popover, not modal
  await page.click('.ant-popover .ant-btn-primary:has-text("Delete")')
  await page.waitForSelector('text=Repository removed successfully', { timeout: 15000 })
}

export async function cancelDeleteRepository(page: Page) {
  await page.click('.ant-popover .ant-btn-default:has-text("Cancel")')
}

export async function deleteRepository(page: Page, repositoryName: string): Promise<void> {
  await openDeleteRepositoryDialog(page, repositoryName)
  await confirmDeleteRepository(page)

  // Wait for removal from list
  await page.waitForLoadState('networkidle')
  await expect(page.locator(`text=${repositoryName}`).first()).not.toBeVisible()
}

// =====================================================
// Repository Enable/Disable
// =====================================================

export async function toggleRepositoryStatus(page: Page, repositoryName: string): Promise<void> {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const toggle = repositoryRow.locator('.ant-switch')
  await toggle.click()
  await page.waitForLoadState('networkidle')
}

export async function enableRepository(page: Page, repositoryName: string): Promise<void> {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const toggle = repositoryRow.locator('.ant-switch')
  const isEnabled = await toggle.getAttribute('aria-checked')

  if (isEnabled === 'false') {
    await toggle.click()
    await page.waitForLoadState('networkidle')
  }
}

export async function disableRepository(page: Page, repositoryName: string): Promise<void> {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const toggle = repositoryRow.locator('.ant-switch')
  const isEnabled = await toggle.getAttribute('aria-checked')

  if (isEnabled === 'true') {
    await toggle.click()
    await page.waitForLoadState('networkidle')
  }
}

// =====================================================
// Repository Assertions
// =====================================================

export async function assertRepositoryExists(page: Page, repositoryName: string): Promise<void> {
  await expect(page.locator(`text=${repositoryName}`).first()).toBeVisible()
}

export async function assertRepositoryNotExists(page: Page, repositoryName: string): Promise<void> {
  await expect(page.locator(`text=${repositoryName}`).first()).not.toBeVisible()
}

export async function assertRepositoryEnabled(page: Page, repositoryName: string): Promise<void> {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const toggle = repositoryRow.locator('.ant-switch')
  await expect(toggle).toHaveAttribute('aria-checked', 'true')
}

export async function assertRepositoryDisabled(page: Page, repositoryName: string): Promise<void> {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const toggle = repositoryRow.locator('.ant-switch')
  await expect(toggle).toHaveAttribute('aria-checked', 'false')
}

export async function assertRepositoryBuiltIn(page: Page, repositoryName: string): Promise<void> {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  await expect(repositoryRow.locator('text=Built-in')).toBeVisible()
}

// =====================================================
// Repository Connection Testing
// =====================================================

export async function clickTestConnectionFromList(page: Page, repositoryName: string): Promise<void> {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const testButton = repositoryRow.locator('button:has-text("Test")')
  await testButton.click()
}

export async function clickTestConnectionFromDrawer(page: Page): Promise<void> {
  const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
  const testButton = drawer.locator('button:has-text("Test Connection")')
  await testButton.click()
}

export async function assertTestButtonLoading(page: Page, repositoryName: string, loading: boolean): Promise<void> {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const testButton = repositoryRow.locator('button:has-text("Test")')

  if (loading) {
    await expect(testButton).toHaveClass(/ant-btn-loading/)
  } else {
    await expect(testButton).not.toHaveClass(/ant-btn-loading/)
  }
}

export async function assertTestConnectionButtonVisible(page: Page, repositoryName: string): Promise<void> {
  const repositoryRow = page.locator('div').filter({ hasText: new RegExp(`^${repositoryName}`) }).first()
  const testButton = repositoryRow.locator('button:has-text("Test")')
  await expect(testButton).toBeVisible()
}

export async function assertTestConnectionButtonInDrawerVisible(page: Page): Promise<void> {
  const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
  const testButton = drawer.locator('button:has-text("Test Connection")')
  await expect(testButton).toBeVisible()
}

export async function waitForConnectionTestResult(page: Page, expectedType: 'success' | 'error'): Promise<void> {
  if (expectedType === 'success') {
    // Wait for success message containing "Connection" and "successful"
    await page.waitForSelector('.ant-message-success', { timeout: 15000 })
  } else {
    // Wait for error message or warning
    const messageSelector = page.locator('.ant-message-error, .ant-message-warning').first()
    await expect(messageSelector).toBeVisible({ timeout: 15000 })
  }
}

// =====================================================
// Repository Configuration (for tests requiring auth)
// =====================================================

/**
 * Configure the built-in HuggingFace repository with API key from environment
 * This is required for download tests that access HuggingFace models
 *
 * Mirrors backend test helper: src-web/tests/llm_model/download_test.rs::get_huggingface_repository
 */
export async function configureHuggingFaceAuth(page: Page, baseURL: string): Promise<void> {
  const apiKey = process.env.HUGGINGFACE_API_KEY

  if (!apiKey) {
    throw new Error('HUGGINGFACE_API_KEY not set in environment. Please ensure tests/.env.test is loaded.')
  }

  // Navigate to repositories page
  await goToRepositoriesPage(page, baseURL)
  await waitForRepositoriesPageLoad(page)

  // Open edit drawer for Hugging Face Hub
  await openEditRepositoryDrawer(page, 'Hugging Face Hub')

  // Fill in the API key
  await page.fill('#llm-repository-form_api_key', apiKey)

  // Update the repository
  await updateRepositoryForm(page)
  await page.waitForSelector('text=Repository updated successfully', { timeout: 15000 })

  // Navigate to Settings > LLM Providers to clean up UI state
  // This ensures any open drawers are closed and we're on a clean page
  await page.goto(`${baseURL}/settings`)
  await page.waitForLoadState('networkidle')
  await page.click('text=LLM Providers')
  await page.waitForLoadState('networkidle')
}
