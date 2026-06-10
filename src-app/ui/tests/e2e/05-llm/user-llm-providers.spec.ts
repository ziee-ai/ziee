import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

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
 *
 * `apiKey: null` produces a `custom` provider with no system key — the only
 * combination the backend accepts for an enabled provider with no key. This
 * is what the orange "No admin key" tag covers in the UI.
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
 * Click a provider in the page's provider menu (desktop) or dropdown (mobile).
 * Uses getByRole('menuitem') which covers both Ant Design Menu (desktop) and
 * Dropdown (mobile) rendering.
 */
async function selectProviderInMenu(page: import('@playwright/test').Page, providerName: string) {
  // Desktop: provider sits in an Ant Menu rendered in the left sidebar.
  // Mobile: provider sits in an Ant Dropdown.  Both render menu items with role="menuitem".
  await page.getByRole('menuitem', { name: providerName }).first().click()
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

    await selectProviderInMenu(page, providerName)

    // The page should now show the provider's detail panel with the orange "No admin key" tag.
    await expect(
      page.getByRole('heading', { level: 4, name: providerName }),
    ).toBeVisible()
    await expect(page.getByText('No admin key')).toBeVisible()
    await expect(page.getByRole('button', { name: 'Save Key' })).toBeVisible()
  })

  test('displays provider with admin key as blue tag', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // loginAsAdmin creates the admin via the setup form on first run; getAdminToken
    // requires the admin to already exist, so this order matters.
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

    await selectProviderInMenu(page, providerName)

    await expect(page.getByText('Admin key configured')).toBeVisible()
  })

  test('saves a user API key end-to-end', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // loginAsAdmin creates the admin via the setup form on first run; getAdminToken
    // requires the admin to already exist, so this order matters.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-save-${Date.now()}`

    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, providerName)

    // Fill the password input (Ant Design Input.Password — no native label, use placeholder)
    await page
      .getByPlaceholder('Enter your API key (e.g. sk-...)')
      .fill('sk-my-personal-key')

    // Save button becomes enabled when input has content.
    const saveBtn = page.getByRole('button', { name: 'Save Key' })
    await expect(saveBtn).toBeEnabled()
    await saveBtn.click()

    // After save, tag flips to green "Your key configured", and the Update/Remove pair appears.
    await expect(page.getByText('Your key configured')).toBeVisible()
    await expect(page.getByRole('button', { name: 'Update Key' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Remove Key' })).toBeVisible()
  })

  test('save button is disabled while input is empty', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // loginAsAdmin creates the admin via the setup form on first run; getAdminToken
    // requires the admin to already exist, so this order matters.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-empty-${Date.now()}`

    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, providerName)

    // No text in the input → Save button is disabled.
    await expect(page.getByRole('button', { name: 'Save Key' })).toBeDisabled()

    // Whitespace-only input → still disabled (handleSave trims).
    await page
      .getByPlaceholder('Enter your API key (e.g. sk-...)')
      .fill('   ')
    await expect(page.getByRole('button', { name: 'Save Key' })).toBeDisabled()
  })

  test('updates an existing user API key', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // loginAsAdmin creates the admin via the setup form on first run; getAdminToken
    // requires the admin to already exist, so this order matters.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-update-${Date.now()}`

    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, providerName)

    // Initial save.
    await page
      .getByPlaceholder('Enter your API key (e.g. sk-...)')
      .fill('sk-first')
    await page.getByRole('button', { name: 'Save Key' }).click()
    await expect(page.getByText('Your key configured')).toBeVisible()

    // Update: input now shows the masked placeholder; focusing must clear it
    // and entering a new key + clicking "Update Key" must succeed.
    const updateInput = page.getByPlaceholder('Enter new key to replace')
    await updateInput.click() // focus → clears the display placeholder
    await updateInput.fill('sk-second')

    await page.getByRole('button', { name: 'Update Key' }).click()

    // Still green; still in the "Your key configured" state.
    await expect(page.getByText('Your key configured')).toBeVisible()
    await expect(page.getByRole('button', { name: 'Update Key' })).toBeVisible()
  })

  test('deletes a user API key and reverts tag', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // loginAsAdmin creates the admin via the setup form on first run; getAdminToken
    // requires the admin to already exist, so this order matters.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerName = `e2e-delete-${Date.now()}`

    const provider = await createProviderViaApi(apiURL, adminToken, providerName, null)
    await assignProviderToDefaultGroup(apiURL, adminToken, provider.id)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    await selectProviderInMenu(page, providerName)

    // Save a key first.
    await page
      .getByPlaceholder('Enter your API key (e.g. sk-...)')
      .fill('sk-delete-me')
    await page.getByRole('button', { name: 'Save Key' }).click()
    await expect(page.getByText('Your key configured')).toBeVisible()

    // Delete it.
    await page.getByRole('button', { name: 'Remove Key' }).click()

    // Tag reverts to the orange "No admin key" state (this provider has no system key)
    // and the Save Key button reappears.
    await expect(page.getByText('No admin key')).toBeVisible()
    await expect(page.getByRole('button', { name: 'Save Key' })).toBeVisible()
    // Update/Remove buttons must be gone.
    await expect(page.getByRole('button', { name: 'Update Key' })).not.toBeVisible()
    await expect(page.getByRole('button', { name: 'Remove Key' })).not.toBeVisible()
  })
})
