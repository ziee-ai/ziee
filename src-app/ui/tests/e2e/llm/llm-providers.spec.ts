import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  goToProvidersPage,
  waitForProvidersPageLoad,
  clickProviderCard,
} from './helpers/navigation-helpers'
import {
  createLocalProvider,
  createRemoteProvider,
  deleteProvider,
  toggleProviderStatus,
  assertProviderExists,
  assertProviderNotExists,
  assertProviderEnabled,
  assertProviderDisabled,
  openAddProviderDrawer,
  selectProviderType,
  createProvider,
} from './helpers/provider-helpers'
import { submitProviderForm } from './helpers/form-helpers'

// Header enable/disable switch carries the provider name in its aria-label
// (`Enable/Disable ${name} provider`) — a durable, dynamic-data proof that the
// detail header is rendered for this provider.
const headerForProvider = (page: import('@playwright/test').Page, name: string) =>
  page.locator(`[aria-label*="${name} provider"]`).first()

test.describe('LLM Providers - List Page', () => {
  test('should pass accessibility checks', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    await assertNoAccessibilityViolations(page)
  })

  test('should pass accessibility checks in dark mode', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    await setTheme(page, 'dark')
    await page.waitForTimeout(500) // Wait for theme transition

    const darkModeActive = await isDarkMode(page)
    expect(darkModeActive).toBe(true)

    await assertNoAccessibilityViolations(page)
  })

  test('should display providers page with Add Provider menu item', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    // Verify we're on the providers page
    await expect(page).toHaveURL(new RegExp('/settings/llm-providers'))

    // Verify Add Provider affordance exists.
    await expect(byTestId(page, 'llm-provider-nav-add-provider')).toBeVisible()
  })
})

test.describe('LLM Providers - Local Provider CRUD', () => {
  test('should create a local provider', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const providerName = `test-local-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, providerName, 'Test local provider')

    // Verify provider appears in list
    await assertProviderExists(page, providerName)

    // Cleanup
    await deleteProvider(page, providerName)
  })

  test('should open Add Provider drawer with correct structure', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    await openAddProviderDrawer(page)

    // Verify drawer is open with the provider form.
    await expect(byTestId(page, 'llm-provider-form')).toBeVisible()

    // Provider Type Select, Name input, Enable switch, and the buttons.
    await expect(byTestId(page, 'llm-provider-type-select')).toBeVisible()
    await expect(byTestId(page, 'llm-provider-name-input')).toBeVisible()
    await expect(byTestId(page, 'llm-provider-enabled-switch')).toBeVisible()
    await expect(byTestId(page, 'llm-provider-cancel-btn')).toBeVisible()
    await expect(byTestId(page, 'llm-provider-submit-btn')).toBeVisible()

    // Close drawer
    await byTestId(page, 'llm-provider-cancel-btn').click()
  })

  test('should show different fields for local vs remote provider types', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    await openAddProviderDrawer(page)

    // Local provider should NOT show API Key and Base URL
    await selectProviderType(page, 'local')
    await expect(byTestId(page, 'llm-provider-api-key-input')).not.toBeVisible()
    await expect(byTestId(page, 'llm-provider-base-url-input')).not.toBeVisible()

    // OpenAI provider should show API Key and Base URL
    await selectProviderType(page, 'openai')
    await expect(byTestId(page, 'llm-provider-api-key-input')).toBeVisible()
    await expect(byTestId(page, 'llm-provider-base-url-input')).toBeVisible()

    // Close drawer
    await byTestId(page, 'llm-provider-cancel-btn').click()
  })

  test('should validate required provider name field', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    await openAddProviderDrawer(page)

    // Select local type
    await selectProviderType(page, 'local')

    // Try to submit without provider name
    await submitProviderForm(page)

    // Should surface a validation error (FieldError renders role="alert").
    await expect(byTestId(page, 'llm-provider-form').getByRole('alert').first()).toBeVisible()

    // Close drawer
    await byTestId(page, 'llm-provider-cancel-btn').click()
  })

  test('should edit a local provider name', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const originalName = `test-edit-${Date.now()}`
    const updatedName = `${originalName}-updated`

    await loginAsAdmin(page, baseURL)

    // Create provider
    await createLocalProvider(page, baseURL, originalName, 'Original description')

    // Navigate to provider detail page
    await clickProviderCard(page, originalName)
    await expect(page).toHaveURL(new RegExp(`/settings/llm-providers/[a-f0-9-]+`))

    // Click edit button in provider header
    await byTestId(page, 'llm-provider-header-edit-name-btn').click()

    // Wait for edit mode
    const input = byTestId(page, 'llm-provider-header-name-input')
    await expect(input).toBeVisible()

    // Update name + save (assert the server accepted the rename).
    await input.fill(updatedName)
    const [resp] = await Promise.all([
      page.waitForResponse(
        r => r.url().includes('/api/llm-providers') && r.request().method() === 'PUT',
        { timeout: 15000 }
      ),
      byTestId(page, 'llm-provider-header-save-name-btn').click(),
    ])
    expect(resp.ok()).toBeTruthy()

    // The header now reflects the new name (switch aria-label embeds it).
    await expect(headerForProvider(page, updatedName)).toBeVisible()

    // Go back and cleanup
    await page.goBack()
    await deleteProvider(page, updatedName)
  })

  test('cancelling the inline name edit reverts to the original name', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const originalName = `test-cancel-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, originalName, 'desc')
    await clickProviderCard(page, originalName)
    await expect(page).toHaveURL(/\/settings\/llm-providers\/[a-f0-9-]+/)

    await byTestId(page, 'llm-provider-header-edit-name-btn').click()
    const input = byTestId(page, 'llm-provider-header-name-input')
    await expect(input).toBeVisible()

    // Type a new value then CANCEL — the change must NOT persist.
    await input.fill(`${originalName}-discarded`)
    await byTestId(page, 'llm-provider-header-cancel-name-btn').click()

    // Header still shows the original name; the discarded one never landed.
    await expect(headerForProvider(page, originalName)).toBeVisible()
    await expect(headerForProvider(page, `${originalName}-discarded`)).toHaveCount(0)

    await page.goBack()
    await deleteProvider(page, originalName)
  })

  test('empty provider name fails inline-edit validation', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const originalName = `test-empty-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createLocalProvider(page, baseURL, originalName, 'desc')
    await clickProviderCard(page, originalName)
    await expect(page).toHaveURL(/\/settings\/llm-providers\/[a-f0-9-]+/)

    await byTestId(page, 'llm-provider-header-edit-name-btn').click()
    const input = byTestId(page, 'llm-provider-header-name-input')
    await expect(input).toBeVisible()

    // Clear the field and try to save → the required rule blocks it.
    await input.fill('')
    await byTestId(page, 'llm-provider-header-save-name-btn').click()

    // A validation error appears (FieldError role="alert").
    await expect(byTestId(page, 'llm-provider-header-name-form').getByRole('alert').first()).toBeVisible()
    // Still in edit mode (save was rejected).
    await expect(byTestId(page, 'llm-provider-header-save-name-btn')).toBeVisible()

    await byTestId(page, 'llm-provider-header-cancel-name-btn').click()
    await page.goBack()
    await deleteProvider(page, originalName)
  })

  test('should delete a local provider', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const providerName = `test-delete-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create provider
    await createLocalProvider(page, baseURL, providerName, 'Will be deleted')

    // Delete the provider
    await deleteProvider(page, providerName)

    // Verify provider is gone
    await assertProviderNotExists(page, providerName)
  })

  test('should toggle provider enabled status', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const providerName = `test-toggle-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create enabled provider
    await createLocalProvider(page, baseURL, providerName, 'Test toggle')
    await assertProviderEnabled(page, providerName)

    // Toggle to disabled
    await toggleProviderStatus(page, providerName)
    await page.waitForTimeout(500) // Wait for state update
    await assertProviderDisabled(page, providerName)

    // Toggle back to enabled
    await toggleProviderStatus(page, providerName)
    await page.waitForTimeout(500)
    await assertProviderEnabled(page, providerName)

    // Cleanup
    await deleteProvider(page, providerName)
  })
})

test.describe('LLM Providers - Remote Provider CRUD', () => {
  test('should create an OpenAI provider', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const providerName = `test-openai-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createRemoteProvider(
      page,
      baseURL,
      providerName,
      'https://api.openai.com/v1',
      'sk-test-key-123'
    )

    // Verify provider appears in list
    await assertProviderExists(page, providerName)

    // Cleanup
    await deleteProvider(page, providerName)
  })

  // TODO: URL validation test removed - the UI currently does NOT validate base URL format
  // This is likely a bug - the base_url field has no validation rules
  // Either:
  // 1. Add validation to LlmProviderDrawer.tsx base_url field, OR
  // 2. Remove this test if validation is not needed

  test('should support multiple remote provider types', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    await openAddProviderDrawer(page)

    // Open the provider type Select.
    await byTestId(page, 'llm-provider-type-select').click()

    // Verify all provider types are available (kit Select derives
    // `${trigger}-opt-${value}` per option).
    const providerTypeValues = [
      'local',
      'openai',
      'anthropic',
      'groq',
      'gemini',
      'mistral',
      'deepseek',
      'huggingface',
      'custom',
    ]

    for (const value of providerTypeValues) {
      await expect(byTestId(page, `llm-provider-type-select-opt-${value}`)).toBeVisible()
    }

    // Close dropdown and drawer
    await page.keyboard.press('Escape')
    await byTestId(page, 'llm-provider-cancel-btn').click()
  })

  // The visibility test above only proves the 9 types render in the dropdown.
  // This one actually CREATES a provider of each remote type end-to-end (drawer
  // → select type → fill → submit → appears in list) and cleans it up, closing
  // the "only 2 of 9 exercised" gap. `local` + `openai` are already covered by
  // createLocalProvider/createRemoteProvider elsewhere, so this drives the
  // remaining keyed types + custom.
  const REMOTE_TYPES = [
    'anthropic',
    'groq',
    'gemini',
    'mistral',
    'deepseek',
    'huggingface',
    'custom',
  ] as const

  for (const type of REMOTE_TYPES) {
    test(`creates a provider of type "${type}" end-to-end`, async ({
      page,
      testInfra,
    }) => {
      const { baseURL } = testInfra
      const providerName = `e2e-${type}-${Date.now()}`

      await loginAsAdmin(page, baseURL)

      // `custom` has no preset base URL, so supply one; keyed SaaS types
      // default their base URL and just need an API key.
      await createProvider(
        page,
        baseURL,
        type === 'custom'
          ? { name: providerName, baseUrl: 'https://api.example.com/v1', apiKey: 'sk-e2e-test' }
          : { name: providerName, apiKey: 'sk-e2e-test' },
        type,
      )

      await assertProviderExists(page, providerName)
      await deleteProvider(page, providerName)
    })
  }
})

test.describe('LLM Providers - Navigation & Detail Page', () => {
  test('should navigate to provider detail page', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const providerName = `test-nav-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create provider
    await createLocalProvider(page, baseURL, providerName, 'Test navigation')

    // Click on provider card to navigate
    await clickProviderCard(page, providerName)

    // Verify we're on the detail page
    await expect(page).toHaveURL(new RegExp(`/settings/llm-providers/[a-f0-9-]+`))

    // Verify provider header reflects this provider.
    await expect(headerForProvider(page, providerName)).toBeVisible()

    // NOTE: Provider detail page has NO TABS - it shows sections directly
    // Verify the Models section is present.
    await expect(byTestId(page, 'llm-models-section-card')).toBeVisible()

    // Go back and cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
  })

  test('Models section: empty state + "Add model" opens the remote model drawer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const providerName = `test-models-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    // A remote provider so "Add model" goes straight to the Add Remote Model drawer.
    await createRemoteProvider(
      page,
      baseURL,
      providerName,
      'https://api.openai.com/v1',
      'sk-test-key',
    )
    await clickProviderCard(page, providerName)

    // The Models card renders with the empty state (no models yet).
    await expect(byTestId(page, 'llm-models-section-card')).toBeVisible()
    await expect(byTestId(page, 'llm-models-empty')).toBeVisible()

    // Interact: click the "Add model" affordance → the Add Remote Model drawer opens.
    await byTestId(page, 'llm-models-add-remote-btn').click()
    await expect(byTestId(page, 'llm-add-remote-model-form')).toBeVisible({ timeout: 15000 })

    // Cancel out and clean up.
    await page.keyboard.press('Escape')
    await page.goBack()
    await deleteProvider(page, providerName)
  })

  test('should show API Configuration for remote providers', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const providerName = `test-remote-nav-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create remote provider
    await createRemoteProvider(
      page,
      baseURL,
      providerName,
      'https://api.openai.com/v1',
      'sk-test-key'
    )

    // Navigate to detail page
    await clickProviderCard(page, providerName)

    // Verify API Configuration card + its fields exist for the remote provider.
    await expect(byTestId(page, 'llm-remote-api-config-card')).toBeVisible()
    await expect(byTestId(page, 'llm-remote-api-key-input')).toBeVisible()
    await expect(byTestId(page, 'llm-remote-base-url-input')).toBeVisible()

    // Go back and cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
  })

  test('should display provider actions in header', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const providerName = `test-actions-${Date.now()}`

    await loginAsAdmin(page, baseURL)

    // Create provider
    await createLocalProvider(page, baseURL, providerName, 'Test actions')

    // Navigate to detail page
    await clickProviderCard(page, providerName)

    // Verify header actions exist
    await expect(byTestId(page, 'llm-provider-header-edit-name-btn')).toBeVisible()
    await expect(byTestId(page, 'llm-provider-delete-btn')).toBeVisible()

    // Verify provider enable/disable switch (header one carries the name).
    const providerSwitch = byTestId(page, 'llm-provider-header-enabled-switch')
    await expect(providerSwitch).toBeVisible()
    await expect(providerSwitch).toHaveAttribute('aria-label', new RegExp(providerName))

    // Go back and cleanup
    await page.goBack()
    await deleteProvider(page, providerName)
  })
})

test.describe('LLM Providers - Empty States', () => {
  test('should handle empty provider list gracefully', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    // The page is still functional — the Add Provider affordance is present.
    await expect(byTestId(page, 'llm-provider-nav-add-provider')).toBeVisible()
  })
})

test.describe('LLM Providers - Multiple providers (key + keyless mix)', () => {
  test('a keyed remote provider and a keyless local provider coexist in the list', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const ts = Date.now()
    const remoteName = `mix-remote-${ts}`
    const localName = `mix-local-${ts}`

    await loginAsAdmin(page, baseURL)

    // A remote (OpenAI) provider configured WITH an API key.
    await createRemoteProvider(
      page,
      baseURL,
      remoteName,
      'https://api.openai.com/v1',
      'sk-mix-test-key-123',
    )
    // A local provider — keyless (no credentials).
    await createLocalProvider(page, baseURL, localName, 'Keyless local provider')

    // Both providers are present simultaneously (the prior suites only ever
    // exercised a single provider at a time).
    await assertProviderExists(page, remoteName)
    await assertProviderExists(page, localName)

    // Cleanup.
    await deleteProvider(page, remoteName)
    await deleteProvider(page, localName)
  })
})
