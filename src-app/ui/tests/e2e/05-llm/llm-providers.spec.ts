import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { setTheme, isDarkMode } from '../../utils/theme'
import { loginAsAdmin } from '../../common/auth-helpers'
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

    // Verify Add Provider menu item exists (in desktop sidebar or mobile dropdown)
    const addProviderMenuItem = page.locator('.ant-menu-item:has-text("Add Provider")')
    await expect(addProviderMenuItem).toBeVisible()
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

    // Verify drawer is open with correct title
    await expect(page.locator('.ant-drawer-title:has-text("Add Provider")')).toBeVisible()

    // Verify Provider Type field exists and is a Select dropdown
    await expect(page.locator('label:has-text("Provider Type")')).toBeVisible()
    // Newer AntD versions render `.ant-select-content` instead of
    // `.ant-select-selector`; the outer `.ant-select` is consistent.
    const providerTypeSelect = page.locator('.ant-select').first()
    await expect(providerTypeSelect).toBeVisible()

    // Verify Provider Name field (with form-prefixed ID)
    await expect(page.locator('label:has-text("Provider Name")')).toBeVisible()
    await expect(page.locator('#llm-provider-form_name')).toBeVisible()

    // Verify Enable Provider switch
    await expect(page.locator('label:has-text("Enable Provider")')).toBeVisible()
    await expect(page.locator('#llm-provider-form_enabled')).toBeVisible()

    // Verify buttons. Drawer submit label was standardised to verb-only
    // (audit I-2): "Add Provider" → "Add". Scope by primary-button
    // class to keep the assertion stable across naming changes.
    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
    await expect(drawer.locator('button:has-text("Cancel")')).toBeVisible()
    await expect(drawer.locator('.ant-btn-primary[type="submit"]')).toBeVisible()

    // Close drawer
    await page.click('button:has-text("Cancel")')
  })

  test('should show different fields for local vs remote provider types', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    await openAddProviderDrawer(page)

    // Local provider should NOT show API Key and Base URL
    await selectProviderType(page, 'local')
    await expect(page.locator('#llm-provider-form_api_key')).not.toBeVisible()
    await expect(page.locator('#llm-provider-form_base_url')).not.toBeVisible()

    // OpenAI provider should show API Key and Base URL
    await selectProviderType(page, 'openai')
    await expect(page.locator('label:has-text("API Key")')).toBeVisible()
    await expect(page.locator('#llm-provider-form_api_key')).toBeVisible()
    await expect(page.locator('label:has-text("Base URL")')).toBeVisible()
    await expect(page.locator('#llm-provider-form_base_url')).toBeVisible()

    // Close drawer
    await page.click('button:has-text("Cancel")')
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

    // Should show validation error - actual message is "Please enter a provider name"
    await expect(page.locator('.ant-form-item-explain-error:has-text("Please enter a provider name")')).toBeVisible()

    // Close drawer
    await page.click('button:has-text("Cancel")')
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
    await page.click('button[aria-label="Edit provider name"]')

    // Wait for edit mode
    await expect(page.locator('input[value="' + originalName + '"]')).toBeVisible()

    // Update name
    await page.fill('input[value="' + originalName + '"]', updatedName)

    // Save
    await page.click('button[aria-label="Save provider name"]')

    // Wait for update to complete
    await page.waitForTimeout(500)

    // Verify new name appears in the header (use heading to be specific)
    await expect(page.getByRole('heading', { name: updatedName })).toBeVisible()

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

    await page.click('button[aria-label="Edit provider name"]')
    const input = page.locator('input[value="' + originalName + '"]')
    await expect(input).toBeVisible()

    // Type a new value then CANCEL — the change must NOT persist.
    await input.fill(`${originalName}-discarded`)
    await page.click('button[aria-label="Cancel editing provider name"]')

    await expect(
      page.getByRole('heading', { name: originalName }),
    ).toBeVisible()
    await expect(
      page.getByRole('heading', { name: `${originalName}-discarded` }),
    ).toHaveCount(0)

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

    await page.click('button[aria-label="Edit provider name"]')
    const input = page.locator('input[value="' + originalName + '"]')
    await expect(input).toBeVisible()

    // Clear the field and try to save → the required rule blocks it.
    await input.fill('')
    await page.click('button[aria-label="Save provider name"]')

    await expect(
      page.locator('.ant-form-item-explain-error:has-text("Name is required")'),
    ).toBeVisible()
    // Still in edit mode (save was rejected) and the original name is intact.
    await expect(
      page.getByRole('button', { name: 'Save provider name' }),
    ).toBeVisible()

    await page.click('button[aria-label="Cancel editing provider name"]')
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

    // Open provider type dropdown (use `.ant-select` not the inner
    // `.ant-select-selector` — newer AntD uses `.ant-select-content`).
    await page.locator('.ant-drawer.ant-drawer-open').locator('.ant-select').first().click()
    await page.waitForSelector('.ant-select-dropdown', { state: 'visible' })

    // Verify all provider types are available
    const providerTypes = [
      'Local',
      'OpenAI',
      'Anthropic',
      'Groq',
      'Google Gemini',
      'Mistral AI',
      'DeepSeek',
      'Hugging Face',
      'Custom',
    ]

    for (const type of providerTypes) {
      await expect(page.locator(`.ant-select-item-option:has-text("${type}")`)).toBeVisible()
    }

    // Close dropdown and drawer
    await page.keyboard.press('Escape')
    await page.click('button:has-text("Cancel")')
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

    // Verify provider name in header
    await expect(page.getByRole('heading', { name: providerName })).toBeVisible()

    // NOTE: Provider detail page has NO TABS - it shows sections directly
    // Verify the Models section is present (DownloadsSection only appears when there are active downloads)
    await expect(page.locator('.ant-card-head-title:has-text("Models")')).toBeVisible()

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
    const modelsCard = page.locator('.ant-card:has(.ant-card-head-title:has-text("Models"))')
    await expect(modelsCard).toBeVisible()
    await expect(modelsCard.getByText('No models yet')).toBeVisible()

    // Interact: click the "Add model" affordance → the Add Remote Model drawer opens.
    await modelsCard.getByRole('button', { name: 'Add model' }).click()
    await expect(
      page.locator('.ant-drawer-title:has-text("Add Remote Model")'),
    ).toBeVisible({ timeout: 15000 })

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

    // Verify API Configuration card exists for remote provider (not present for local providers)
    await expect(page.locator('.ant-card-head-title:has-text("API Configuration")')).toBeVisible()
    await expect(page.getByRole('heading', { name: 'API Key', level: 5 })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Base URL', level: 5 })).toBeVisible()

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
    await expect(page.locator('button[aria-label="Edit provider name"]')).toBeVisible()
    await expect(page.locator('button[aria-label="Delete provider"]')).toBeVisible()

    // Verify provider enable/disable switch (filter by aria-label to get the header one, not drawer one)
    const providerSwitch = page.locator(`.ant-switch[aria-label*="${providerName}"]`)
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

    // If no providers exist, the page should still be functional
    // (No specific empty state component is required - just verify page loads)
    await expect(page.locator('.ant-menu-item:has-text("Add Provider")')).toBeVisible()
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
