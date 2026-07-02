import { Page, expect } from '@playwright/test'
import { fillProviderForm, submitProviderForm, updateProviderForm, type ProviderFormData } from './form-helpers'
import { byTestId } from '../../testid'
import { goToProvidersPage, waitForProvidersPageLoad, clickProviderCard } from './navigation-helpers'

/**
 * LLM Provider CRUD helpers (kit / data-testid based)
 */

// Locator for a provider's nav button (carries the provider name as text).
function providerNav(page: Page, providerName: string) {
  return page
    .locator('[data-testid^="llm-provider-nav-"]')
    .filter({ hasText: providerName })
    .first()
}

// Locator for the header enable/disable switch on the provider detail page.
function headerSwitch(page: Page) {
  return byTestId(page, 'llm-provider-header-enabled-switch')
}

// =====================================================
// Provider Creation
// =====================================================

export async function openAddProviderDrawer(page: Page) {
  await byTestId(page, 'llm-provider-nav-add-provider').click()
  // Wait for the provider form to render inside the drawer.
  await byTestId(page, 'llm-provider-form').waitFor({ state: 'visible', timeout: 30000 })
}

export async function selectProviderType(
  page: Page,
  type: 'local' | 'openai' | 'anthropic' | 'groq' | 'gemini' | 'mistral' | 'deepseek' | 'huggingface' | 'custom'
) {
  // Kit Select: click the trigger, then the option whose value === type.
  await byTestId(page, 'llm-provider-type-select').click()
  await byTestId(page, `llm-provider-type-select-opt-${type}`).click()
  await page.waitForLoadState('load')
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

  // Submit and assert the server accepted the create (real-path proof).
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => r.url().includes('/api/llm-providers') && r.request().method() === 'POST',
      { timeout: 15000 }
    ),
    submitProviderForm(page),
  ])
  expect(resp.ok()).toBeTruthy()

  // Verify the provider appears in the nav (dynamic data the test created).
  await expect(providerNav(page, data.name)).toBeVisible({ timeout: 15000 })
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
  // Provider names are edited inline on the detail page header.
  await clickProviderCard(page, providerName)
  await byTestId(page, 'llm-provider-header-edit-name-btn').click()
  await byTestId(page, 'llm-provider-header-name-input').waitFor({ state: 'visible', timeout: 30000 })
}

export async function updateProvider(
  page: Page,
  providerName: string,
  updates: Partial<ProviderFormData>
): Promise<void> {
  // Header inline edit only supports the name; rename via the header form.
  await openEditProviderDrawer(page, providerName)
  if (updates.name) {
    await byTestId(page, 'llm-provider-header-name-input').fill(updates.name)
  }
  const [resp] = await Promise.all([
    page.waitForResponse(
      // Provider update is POST /api/llm-providers/{id} (not PUT).
      r => /\/api\/llm-providers\/[0-9a-f-]+/.test(r.url()) && r.request().method() === 'POST',
      { timeout: 15000 }
    ),
    byTestId(page, 'llm-provider-header-save-name-btn').click(),
  ])
  expect(resp.ok()).toBeTruthy()
  void fillProviderForm
  void updateProviderForm
}

// =====================================================
// Provider Deletion
// =====================================================

export async function openDeleteProviderDialog(page: Page, providerName: string) {
  // Navigate to provider detail page first - delete button is in ProviderHeader.
  await clickProviderCard(page, providerName)
  await byTestId(page, 'llm-provider-delete-btn').click()
  // The kit Confirm renders its content with the delete-confirm testid.
  await byTestId(page, 'llm-provider-delete-confirm').waitFor({ state: 'visible', timeout: 5000 })
}

export async function confirmDeleteProvider(page: Page) {
  const [resp] = await Promise.all([
    page.waitForResponse(
      r => r.url().includes('/api/llm-providers') && r.request().method() === 'DELETE',
      { timeout: 15000 }
    ),
    byTestId(page, 'llm-provider-delete-confirm-confirm').click(),
  ])
  expect(resp.ok()).toBeTruthy()
}

export async function cancelDeleteProvider(page: Page) {
  await byTestId(page, 'llm-provider-delete-confirm-cancel').click()
}

export async function deleteProvider(page: Page, providerName: string): Promise<void> {
  await openDeleteProviderDialog(page, providerName)
  await confirmDeleteProvider(page)

  // Wait for navigation back to list page.
  await page.waitForLoadState('load')

  // Verify provider no longer in list.
  await expect(providerNav(page, providerName)).not.toBeVisible()
}

// =====================================================
// Provider Enable/Disable
// =====================================================

export async function toggleProviderStatus(page: Page, providerName: string): Promise<void> {
  await clickProviderCard(page, providerName)
  await headerSwitch(page).click()
  await page.waitForLoadState('load')
  await page.goBack()
  await page.waitForLoadState('load')
}

export async function enableProvider(page: Page, providerName: string): Promise<void> {
  await clickProviderCard(page, providerName)
  const toggle = headerSwitch(page)
  if ((await toggle.getAttribute('aria-checked')) === 'false') {
    await toggle.click()
    await page.waitForLoadState('load')
  }
  await page.goBack()
  await page.waitForLoadState('load')
}

export async function disableProvider(page: Page, providerName: string): Promise<void> {
  await clickProviderCard(page, providerName)
  const toggle = headerSwitch(page)
  if ((await toggle.getAttribute('aria-checked')) === 'true') {
    await toggle.click()
    await page.waitForLoadState('load')
  }
  await page.goBack()
  await page.waitForLoadState('load')
}

// =====================================================
// Provider Assertions
// =====================================================

export async function assertProviderExists(page: Page, providerName: string): Promise<void> {
  await expect(providerNav(page, providerName)).toBeVisible()
}

export async function assertProviderNotExists(page: Page, providerName: string): Promise<void> {
  await expect(providerNav(page, providerName)).not.toBeVisible()
}

export async function assertProviderEnabled(page: Page, providerName: string): Promise<void> {
  await clickProviderCard(page, providerName)
  await expect(headerSwitch(page)).toHaveAttribute('aria-checked', 'true')
  await page.goBack()
  await page.waitForLoadState('load')
}

export async function assertProviderDisabled(page: Page, providerName: string): Promise<void> {
  await clickProviderCard(page, providerName)
  await expect(headerSwitch(page)).toHaveAttribute('aria-checked', 'false')
  await page.goBack()
  await page.waitForLoadState('load')
}
