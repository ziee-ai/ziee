import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E tests for the user-llm-providers settings page
 * (path: /settings/user-llm-providers).
 *
 * The page lists every LLM provider the user can access and lets them save,
 * update, or delete a personal API key per provider. A green "Your key
 * configured" tag, blue "Admin key configured" tag, or orange "No admin key"
 * tag reflects the current state.
 *
 * Each test creates its own provider via the admin API (so the page has
 * deterministic content) and then drives the UI for the actual flow.
 */

// ---------- helpers ----------

interface CreatedProvider {
  id: string
  name: string
}

/**
 * Create a provider directly via the admin API. Keeps test fixtures
 * decoupled from the admin-provider UI.
 */
async function createProviderViaApi(
  apiURL: string,
  adminToken: string,
  name: string,
  apiKey: string | null,
): Promise<CreatedProvider> {
  const provider_type = apiKey === null ? 'custom' : 'openai'
  const body: Record<string, unknown> = {
    name,
    provider_type,
    enabled: true,
  }
  if (apiKey !== null) body.api_key = apiKey

  const response = await fetch(`${apiURL}/api/llm-providers`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${adminToken}`,
    },
    body: JSON.stringify(body),
  })
  if (!response.ok) {
    throw new Error(
      `Failed to create provider ${name}: ${response.status} ${await response.text()}`,
    )
  }
  const data = await response.json()
  return { id: data.id, name: data.name }
}

/**
 * Assign a provider to the default Users group so every authenticated user
 * can see it via /api/user-llm-providers.
 */
async function assignProviderToDefaultGroup(
  apiURL: string,
  adminToken: string,
  providerId: string,
): Promise<void> {
  // List groups, find the default one
  const groupsResp = await fetch(`${apiURL}/api/groups`, {
    headers: { Authorization: `Bearer ${adminToken}` },
  })
  const groupsBody = await groupsResp.json()
  const defaultGroup = groupsBody.groups.find((g: { is_default?: boolean }) => g.is_default)
  if (!defaultGroup) throw new Error('No default group found')

  const assignResp = await fetch(
    `${apiURL}/api/llm-providers/${providerId}/groups`,
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      body: JSON.stringify({ group_id: defaultGroup.id }),
    },
  )
  if (!assignResp.ok) {
    throw new Error(
      `Failed to assign provider to default group: ${assignResp.status} ${await assignResp.text()}`,
    )
  }
}

/**
 * Click a provider in the page's provider menu. Menu items derive their id
 * from the provider id (`ullm-provider-menu-item-${id}`).
 */
async function selectProviderInMenu(page: import('@playwright/test').Page, providerId: string) {
  await byTestId(page, `ullm-provider-menu-item-${providerId}`).first().click()
}

// ---------- tests ----------

test.describe('User LLM Providers settings page', () => {
  test('displays provider with no admin key as orange tag', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // loginAsAdmin creates the admin via the setup form on first run; getAdminToken
    // requires the admin to already exist, so this order matters.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-no-key-${Date.now()}`

    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, provider.id)

    // The provider's detail panel renders with the orange "No admin key" tag.
    await expect(byTestId(page, 'ullm-key-form')).toBeVisible()
    await expect(byTestId(page, 'ullm-key-status-tag')).toContainText('No admin key')
    await expect(byTestId(page, 'ullm-save-key-button')).toContainText('Save Key')
  })

  test('displays provider with admin key as blue tag', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-admin-key-${Date.now()}`

    const provider = await createProviderViaApi(
      apiURL,
      adminToken,
      providerName,
      'sk-admin-test',
    )
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, provider.id)

    await expect(byTestId(page, 'ullm-key-status-tag')).toContainText('Admin key configured')
  })

  test('saves a user API key end-to-end', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-save-${Date.now()}`

    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, provider.id)

    // Fill the key input.
    await byTestId(page, 'ullm-key-password-input').fill('sk-my-personal-key')

    // Save button becomes enabled when input has content.
    const saveBtn = byTestId(page, 'ullm-save-key-button')
    await expect(saveBtn).toBeEnabled()
    await saveBtn.click()

    // After save, tag flips to green "Your key configured", button becomes
    // "Update Key", and the Remove pair appears.
    await expect(byTestId(page, 'ullm-key-status-tag')).toContainText('Your key configured')
    await expect(byTestId(page, 'ullm-save-key-button')).toContainText('Update Key')
    await expect(byTestId(page, 'ullm-remove-key-button')).toBeVisible()
  })

  test('save button is disabled while input is empty', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-empty-${Date.now()}`

    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, provider.id)

    // No text in the input → Save button is disabled.
    await expect(byTestId(page, 'ullm-save-key-button')).toBeDisabled()

    // Whitespace-only input → still disabled (handleSave trims).
    await byTestId(page, 'ullm-key-password-input').fill('   ')
    await expect(byTestId(page, 'ullm-save-key-button')).toBeDisabled()
  })

  test('updates an existing user API key', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-update-${Date.now()}`

    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, provider.id)

    // Initial save.
    await byTestId(page, 'ullm-key-password-input').fill('sk-first')
    await byTestId(page, 'ullm-save-key-button').click()
    await expect(byTestId(page, 'ullm-key-status-tag')).toContainText('Your key configured')

    // Update: input now shows the masked placeholder; focusing must clear it
    // and entering a new key + clicking "Update Key" must succeed.
    const updateInput = byTestId(page, 'ullm-key-password-input')
    await updateInput.click() // focus → clears the display placeholder
    await updateInput.fill('sk-second')

    await byTestId(page, 'ullm-save-key-button').click()

    // Still green; still in the "Your key configured" state.
    await expect(byTestId(page, 'ullm-key-status-tag')).toContainText('Your key configured')
    await expect(byTestId(page, 'ullm-save-key-button')).toContainText('Update Key')
  })

  test('deletes a user API key and reverts tag', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-delete-${Date.now()}`

    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, provider.id)

    // Save a key first.
    await byTestId(page, 'ullm-key-password-input').fill('sk-delete-me')
    await byTestId(page, 'ullm-save-key-button').click()
    await expect(byTestId(page, 'ullm-key-status-tag')).toContainText('Your key configured')

    // Delete it.
    await byTestId(page, 'ullm-remove-key-button').click()

    // Tag reverts to the orange "No admin key" state (this provider has no
    // system key) and the Save Key button reappears.
    await expect(byTestId(page, 'ullm-key-status-tag')).toContainText('No admin key')
    await expect(byTestId(page, 'ullm-save-key-button')).toContainText('Save Key')
    // The Remove button must be gone (no user key).
    await expect(byTestId(page, 'ullm-remove-key-button')).not.toBeVisible()
  })

  // audit id c7c6a450bf5d — the page's error branch (UserLlmProvidersPage.tsx
  // :110-112, the `if (error) return <Alert type="error" .../>`) was untested.
  test('shows an error alert when the providers list fails to load', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Fail the providers fetch → the store sets `error` → the page renders the
    // error Alert instead of the provider UI.
    await page.route(/\/api\/user-llm-providers(\?.*)?$/, async route =>
      route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error_code: 'INTERNAL', error: 'provider load exploded' }),
      }),
    )

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await expect(byTestId(page, 'ullm-error-alert')).toBeVisible({ timeout: 15000 })
  })

  // audit id 9b9a621f318b — the mobile (sub-`sm`) responsive layout swaps the
  // desktop sidebar for a Dropdown provider selector (UserLlmProvidersPage.tsx
  // :220-246); that responsive branch + dropdown selection was untested.
  test('mobile viewport uses the dropdown provider selector', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-mobile-${Date.now()}`
    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    // Mobile width → below the `sm` breakpoint.
    await page.setViewportSize({ width: 375, height: 800 })
    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    // The desktop sidebar provider menu is hidden on mobile…
    await expect(byTestId(page, 'ullm-provider-menu')).toHaveCount(0)
    // …and the mobile Dropdown trigger is present. Open it and pick the provider.
    await byTestId(page, 'ullm-provider-dropdown-trigger').first().click()
    await byTestId(page, `ullm-provider-dropdown-item-${provider.id}`).first().click()

    // The provider detail panel renders (key form visible).
    await expect(byTestId(page, 'ullm-key-form')).toBeVisible({ timeout: 10000 })
  })

  // audit id all-eab883dfd4e1 — the "No AI providers available" empty state
  // (UserLlmProvidersPage.tsx:115-129, the `providers.length === 0` Empty) had
  // no E2E. Mock the providers list to return an empty set so the page renders
  // the Empty guidance instead of provider cards.
  test('shows the empty state when no AI providers are available', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.route(/\/api\/user-llm-providers(\?.*)?$/, async route =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ providers: [] }),
      }),
    )

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await expect(byTestId(page, 'ullm-no-providers-empty')).toBeVisible({ timeout: 15000 })
  })
})
