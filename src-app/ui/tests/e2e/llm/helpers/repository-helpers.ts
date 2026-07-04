import { Page, expect, Locator } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * LLM Repository CRUD helpers (kit / data-testid based)
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

// A repository row scoped by the (dynamic) repository name it contains.
function repoRow(page: Page, repositoryName: string): Locator {
  return page
    .locator('[data-testid^="llmrepo-row-"]')
    .filter({ hasText: repositoryName })
    .first()
}

async function setKitSwitch(locator: Locator, desired: boolean) {
  const checked = (await locator.getAttribute('aria-checked')) === 'true'
  if (checked !== desired) await locator.click()
}

// =====================================================
// Navigation
// =====================================================

export async function goToRepositoriesPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/llm-repositories`)
  await page.waitForLoadState('load')
}

export async function waitForRepositoriesPageLoad(page: Page) {
  await byTestId(page, 'llmrepo-card').waitFor({ state: 'visible', timeout: 30000 })
  await page.waitForLoadState('load')
}

// =====================================================
// Repository Creation
// =====================================================

export async function openAddRepositoryDrawer(page: Page) {
  await byTestId(page, 'llmrepo-add-btn').click()
  await byTestId(page, 'llmrepo-form').waitFor({ state: 'visible', timeout: 30000 })
}

export async function fillRepositoryForm(page: Page, data: RepositoryFormData) {
  await byTestId(page, 'llmrepo-form-name').fill(data.name)
  await byTestId(page, 'llmrepo-form-url').fill(data.url)

  // Auth type — kit Select keyed by value.
  await byTestId(page, 'llmrepo-form-auth-type').click()
  await byTestId(page, `llmrepo-form-auth-type-opt-${data.authType}`).click()
  await page.waitForLoadState('load')

  if (data.authType === 'api_key' && data.apiKey) {
    await byTestId(page, 'llmrepo-form-api-key').fill(data.apiKey)
  }
  if (data.authType === 'basic_auth') {
    if (data.username) await byTestId(page, 'llmrepo-form-username').fill(data.username)
    if (data.password) await byTestId(page, 'llmrepo-form-password').fill(data.password)
  }
  if (data.authType === 'bearer_token' && data.bearerToken) {
    await byTestId(page, 'llmrepo-form-token').fill(data.bearerToken)
  }
  if (data.authTestEndpoint) {
    await byTestId(page, 'llmrepo-form-auth-test-endpoint').fill(data.authTestEndpoint)
  }
  if (data.enabled !== undefined) {
    await setKitSwitch(byTestId(page, 'llmrepo-form-enabled-switch'), data.enabled)
  }
}

export async function submitRepositoryForm(page: Page) {
  await byTestId(page, 'llmrepo-form-submit-btn').click()
  await page.waitForLoadState('load')
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

  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/.*repositor/.test(r.url()) && r.request().method() === 'POST',
      { timeout: 15000 }
    ),
    submitRepositoryForm(page),
  ])
  expect(resp.ok()).toBeTruthy()

  // Verify repository appears in the list (dynamic data the test created).
  await expect(repoRow(page, data.name)).toBeVisible({ timeout: 15000 })
}

// =====================================================
// Repository Editing
// =====================================================

export async function openEditRepositoryDrawer(page: Page, repositoryName: string) {
  await repoRow(page, repositoryName)
    .locator('[data-testid^="llmrepo-edit-btn-"]')
    .first()
    .click()
  await byTestId(page, 'llmrepo-form').waitFor({ state: 'visible', timeout: 30000 })
}

export async function updateRepositoryForm(page: Page) {
  await byTestId(page, 'llmrepo-form-submit-btn').click()
  await page.waitForLoadState('load')
}

export async function updateRepository(
  page: Page,
  repositoryName: string,
  updates: Partial<RepositoryFormData>
): Promise<void> {
  await openEditRepositoryDrawer(page, repositoryName)
  await fillRepositoryForm(page, updates as RepositoryFormData)
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/llm-repositories\/[0-9a-f-]+/.test(r.url()) && r.request().method() === 'POST',
      { timeout: 15000 }
    ),
    updateRepositoryForm(page),
  ])
  expect(resp.ok()).toBeTruthy()
}

// =====================================================
// Repository Deletion
// =====================================================

export async function openDeleteRepositoryDialog(page: Page, repositoryName: string) {
  await repoRow(page, repositoryName)
    .locator('[data-testid^="llmrepo-delete-btn-"]')
    .first()
    .click()
  // Kit Confirm dialog content.
  await page
    .locator('[data-testid^="llmrepo-delete-confirm-"]')
    .first()
    .waitFor({ state: 'visible', timeout: 5000 })
}

export async function confirmDeleteRepository(page: Page) {
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/.*repositor/.test(r.url()) && r.request().method() === 'DELETE',
      { timeout: 15000 }
    ),
    page
      .locator('[data-testid^="llmrepo-delete-confirm-"][data-testid$="-confirm"]')
      .first()
      .click(),
  ])
  expect(resp.ok()).toBeTruthy()
}

export async function cancelDeleteRepository(page: Page) {
  await page
    .locator('[data-testid^="llmrepo-delete-confirm-"][data-testid$="-cancel"]')
    .first()
    .click()
}

export async function deleteRepository(page: Page, repositoryName: string): Promise<void> {
  await openDeleteRepositoryDialog(page, repositoryName)
  await confirmDeleteRepository(page)
  await page.waitForLoadState('load')
  await expect(repoRow(page, repositoryName)).not.toBeVisible()
}

// =====================================================
// Repository Enable/Disable
// =====================================================

function repoToggle(page: Page, repositoryName: string): Locator {
  return repoRow(page, repositoryName).locator('[data-testid^="llmrepo-toggle-"]').first()
}

export async function toggleRepositoryStatus(page: Page, repositoryName: string): Promise<void> {
  await repoToggle(page, repositoryName).click()
  await page.waitForLoadState('load')
}

export async function enableRepository(page: Page, repositoryName: string): Promise<void> {
  await setKitSwitch(repoToggle(page, repositoryName), true)
  await page.waitForLoadState('load')
}

export async function disableRepository(page: Page, repositoryName: string): Promise<void> {
  await setKitSwitch(repoToggle(page, repositoryName), false)
  await page.waitForLoadState('load')
}

// =====================================================
// Repository Assertions
// =====================================================

export async function assertRepositoryExists(page: Page, repositoryName: string): Promise<void> {
  await expect(repoRow(page, repositoryName)).toBeVisible()
}

export async function assertRepositoryNotExists(page: Page, repositoryName: string): Promise<void> {
  await expect(repoRow(page, repositoryName)).not.toBeVisible()
}

export async function assertRepositoryEnabled(page: Page, repositoryName: string): Promise<void> {
  await expect(repoToggle(page, repositoryName)).toHaveAttribute('aria-checked', 'true')
}

export async function assertRepositoryDisabled(page: Page, repositoryName: string): Promise<void> {
  await expect(repoToggle(page, repositoryName)).toHaveAttribute('aria-checked', 'false')
}

export async function assertRepositoryBuiltIn(page: Page, repositoryName: string): Promise<void> {
  await expect(
    repoRow(page, repositoryName).filter({ hasText: 'Built-in' })
  ).toBeVisible()
}

// =====================================================
// Repository Connection Testing
// =====================================================

export async function clickTestConnectionFromList(page: Page, repositoryName: string): Promise<void> {
  await repoRow(page, repositoryName)
    .locator('[data-testid^="llmrepo-test-btn-"]')
    .first()
    .click()
}

export async function clickTestConnectionFromDrawer(page: Page): Promise<void> {
  await byTestId(page, 'llmrepo-form-test-btn').click()
}

export async function assertTestButtonLoading(page: Page, repositoryName: string, loading: boolean): Promise<void> {
  const testButton = repoRow(page, repositoryName)
    .locator('[data-testid^="llmrepo-test-btn-"]')
    .first()
  // Kit Button reflects loading via [data-loading] / disabled.
  if (loading) {
    await expect(testButton).toBeDisabled()
  } else {
    await expect(testButton).toBeEnabled()
  }
}

export async function assertTestConnectionButtonVisible(page: Page, repositoryName: string): Promise<void> {
  await expect(
    repoRow(page, repositoryName).locator('[data-testid^="llmrepo-test-btn-"]').first()
  ).toBeVisible()
}

export async function assertTestConnectionButtonInDrawerVisible(page: Page): Promise<void> {
  await expect(byTestId(page, 'llmrepo-form-test-btn')).toBeVisible()
}

export async function waitForConnectionTestResult(page: Page, expectedType: 'success' | 'error'): Promise<void> {
  if (expectedType === 'success') {
    await page
      .locator('[data-sonner-toast][data-type="success"]')
      .first()
      .waitFor({ timeout: 15000 })
  } else {
    await expect(
      page.locator('[data-sonner-toast][data-type="error"], [data-sonner-toast][data-type="warning"]').first()
    ).toBeVisible({ timeout: 15000 })
  }
}

// =====================================================
// Repository Configuration (for tests requiring auth)
// =====================================================

/**
 * Configure the built-in HuggingFace repository with API key from environment.
 * Required for download tests that access HuggingFace models.
 */
export async function configureHuggingFaceAuth(page: Page, baseURL: string): Promise<void> {
  const apiKey = process.env.HUGGINGFACE_API_KEY

  if (!apiKey) {
    throw new Error('HUGGINGFACE_API_KEY not set in environment. Please ensure tests/.env.test is loaded.')
  }

  await goToRepositoriesPage(page, baseURL)
  await waitForRepositoriesPageLoad(page)

  await openEditRepositoryDrawer(page, 'Hugging Face Hub')
  await byTestId(page, 'llmrepo-form-api-key').fill(apiKey)

  const [resp] = await Promise.all([
    page.waitForResponse(
      r => /\/api\/llm-repositories\/[0-9a-f-]+/.test(r.url()) && r.request().method() === 'POST',
      { timeout: 15000 }
    ),
    updateRepositoryForm(page),
  ])
  expect(resp.ok()).toBeTruthy()

  // Navigate to a clean providers page so any open drawer is dismissed.
  await page.goto(`${baseURL}/settings/llm-providers`)
  await page.waitForLoadState('load')
}
