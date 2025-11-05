import { Page, expect } from '@playwright/test'
import { fillProviderForm, submitProviderForm, updateProviderForm, type ProviderFormData } from './form-helpers'
import { goToProvidersPage, waitForProvidersPageLoad, clickProviderCard } from './navigation-helpers'

/**
 * LLM Provider CRUD helpers
 */

// =====================================================
// Provider Creation
// =====================================================

export async function openAddProviderDrawer(page: Page) {
  // "Add Provider" is an Ant Design Menu item, not a button
  await page.click('.ant-menu-item:has-text("Add Provider")')
  // Wait for the drawer to open - it shows "Add Provider" as title
  await page.waitForSelector('.ant-drawer-title:has-text("Add Provider")', { timeout: 30000 })
}

export async function selectProviderType(page: Page, type: 'local' | 'openai' | 'anthropic' | 'groq' | 'gemini' | 'mistral' | 'deepseek' | 'huggingface' | 'custom') {
  // Provider Type is an Ant Design Select dropdown
  // Click anywhere on the select to open it
  await page.click('.ant-select-selector:has(.ant-select-selection-item):visible')

  // Wait for dropdown to appear
  await page.waitForSelector('.ant-select-dropdown', { state: 'visible', timeout: 5000 })

  // Map type to label text shown in UI
  const labelMap: Record<typeof type, string> = {
    'local': 'Local',
    'openai': 'OpenAI',
    'anthropic': 'Anthropic',
    'groq': 'Groq',
    'gemini': 'Google Gemini',
    'mistral': 'Mistral AI',
    'deepseek': 'DeepSeek',
    'huggingface': 'Hugging Face',
    'custom': 'Custom',
  }

  // Click the option in the dropdown
  await page.click(`.ant-select-item-option:has-text("${labelMap[type]}")`)

  await page.waitForLoadState('networkidle')
}

export async function createProvider(
  page: Page,
  baseURL: string,
  data: ProviderFormData,
  type: 'local' | 'openai' | 'anthropic' | 'groq' | 'gemini' | 'mistral' | 'deepseek' | 'huggingface' | 'custom' = 'local'
): Promise<void> {
  await goToProvidersPage(page, baseURL)
  await waitForProvidersPageLoad(page)

  await openAddProviderDrawer(page)
  await selectProviderType(page, type)

  await fillProviderForm(page, data)
  await submitProviderForm(page)

  // Wait for success message - actual message is "Provider added successfully"
  await page.waitForSelector('text=Provider added successfully', { timeout: 15000 })

  // Verify provider appears in list
  await expect(page.locator(`text=${data.name}`)).toBeVisible()
}

export async function createLocalProvider(
  page: Page,
  baseURL: string,
  name: string,
  description?: string
): Promise<void> {
  await createProvider(page, baseURL, { name, description }, 'local')
}

export async function createRemoteProvider(
  page: Page,
  baseURL: string,
  name: string,
  baseUrl: string,
  apiKey?: string
): Promise<void> {
  // Use 'openai' as the remote provider type (there's no generic 'remote')
  await createProvider(page, baseURL, { name, baseUrl, apiKey }, 'openai')
}

// =====================================================
// Provider Editing
// =====================================================

export async function openEditProviderDrawer(page: Page, providerName: string) {
  const providerCard = page.locator(`text=${providerName}`).first()
  const editButton = providerCard.locator('button[aria-label="Edit"]')
  await editButton.click()
  await page.waitForSelector('text=Edit Provider', { timeout: 30000 })
}

export async function updateProvider(
  page: Page,
  providerName: string,
  updates: Partial<ProviderFormData>
): Promise<void> {
  await openEditProviderDrawer(page, providerName)
  await fillProviderForm(page, updates as ProviderFormData)
  await updateProviderForm(page)
  await page.waitForSelector('text=Provider updated successfully', { timeout: 15000 })
}

// =====================================================
// Provider Deletion
// =====================================================

export async function openDeleteProviderDialog(page: Page, providerName: string) {
  // Navigate to provider detail page first - delete button is in ProviderHeader
  await clickProviderCard(page, providerName)

  // Click delete button in ProviderHeader
  const deleteButton = page.locator('button[aria-label="Delete provider"]')
  await deleteButton.click()

  // Wait for confirmation modal - use the modal container selector
  await page.waitForSelector('.ant-modal-confirm', { state: 'visible', timeout: 5000 })
  await expect(page.locator('.ant-modal-confirm .ant-modal-confirm-title:has-text("Confirm Deletion")').first()).toBeVisible()
}

export async function confirmDeleteProvider(page: Page) {
  // Click "Delete" button in confirmation modal
  await page.click('.ant-modal-confirm-btns button:has-text("Delete")')
  await page.waitForSelector('text=Provider deleted successfully', { timeout: 15000 })
}

export async function cancelDeleteProvider(page: Page) {
  await page.click('.ant-modal-confirm-btns button:has-text("Cancel")')
}

export async function deleteProvider(page: Page, providerName: string): Promise<void> {
  await openDeleteProviderDialog(page, providerName)
  await confirmDeleteProvider(page)

  // Wait for navigation back to list page
  await page.waitForLoadState('networkidle')

  // Verify provider no longer in list
  await expect(page.locator(`text=${providerName}`).first()).not.toBeVisible()
}

// =====================================================
// Provider Enable/Disable
// =====================================================

export async function toggleProviderStatus(page: Page, providerName: string): Promise<void> {
  // Navigate to provider detail page first - toggle switch is in ProviderHeader
  await clickProviderCard(page, providerName)

  // Find the toggle switch by aria-label that contains the provider name
  const toggle = page.locator(`.ant-switch[aria-label*="${providerName}"]`)
  await toggle.click()
  await page.waitForLoadState('networkidle')

  // Navigate back to list page
  await page.goBack()
  await page.waitForLoadState('networkidle')
}

export async function enableProvider(page: Page, providerName: string): Promise<void> {
  // Navigate to provider detail page first
  await clickProviderCard(page, providerName)

  const toggle = page.locator(`.ant-switch[aria-label*="${providerName}"]`)
  const isEnabled = await toggle.getAttribute('aria-checked')

  if (isEnabled === 'false') {
    await toggle.click()
    await page.waitForLoadState('networkidle')
  }

  // Navigate back to list page
  await page.goBack()
  await page.waitForLoadState('networkidle')
}

export async function disableProvider(page: Page, providerName: string): Promise<void> {
  // Navigate to provider detail page first
  await clickProviderCard(page, providerName)

  const toggle = page.locator(`.ant-switch[aria-label*="${providerName}"]`)
  const isEnabled = await toggle.getAttribute('aria-checked')

  if (isEnabled === 'true') {
    await toggle.click()
    await page.waitForLoadState('networkidle')
  }

  // Navigate back to list page
  await page.goBack()
  await page.waitForLoadState('networkidle')
}

// =====================================================
// Provider Assertions
// =====================================================

export async function assertProviderExists(page: Page, providerName: string): Promise<void> {
  await expect(page.locator(`text=${providerName}`).first()).toBeVisible()
}

export async function assertProviderNotExists(page: Page, providerName: string): Promise<void> {
  await expect(page.locator(`text=${providerName}`).first()).not.toBeVisible()
}

export async function assertProviderEnabled(page: Page, providerName: string): Promise<void> {
  // Navigate to provider detail page to check toggle state
  await clickProviderCard(page, providerName)

  const toggle = page.locator(`.ant-switch[aria-label*="${providerName}"]`)
  await expect(toggle).toHaveAttribute('aria-checked', 'true')

  // Navigate back to list page
  await page.goBack()
  await page.waitForLoadState('networkidle')
}

export async function assertProviderDisabled(page: Page, providerName: string): Promise<void> {
  // Navigate to provider detail page to check toggle state
  await clickProviderCard(page, providerName)

  const toggle = page.locator(`.ant-switch[aria-label*="${providerName}"]`)
  await expect(toggle).toHaveAttribute('aria-checked', 'false')

  // Navigate back to list page
  await page.goBack()
  await page.waitForLoadState('networkidle')
}
