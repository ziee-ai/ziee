/**
 * Admin CRUD E2E for /settings/auth-providers — exercises the real
 * backend (no API mocks). The Test endpoint will fail for the fake
 * URLs we use, but that's the POINT for this test: we're verifying
 * the UI handles the failure gracefully + persists the result.
 *
 * UI shape (post settings UX overhaul): providers render as a Card of
 * rows (NOT an AntD Table), each row carries a `Toggle <name>` switch
 * and `Test/Edit/Delete <name>` actions (per-row aria-labels). "Add
 * provider" is a `+` icon button (aria-label "Add authentication
 * provider") opening a dropdown of templates; templates whose name is
 * already taken (google/microsoft/apple seeded by migration 47) are
 * filtered OUT of the menu.
 *
 * Out of scope: the actual OAuth dance — covered by
 * `social-login-navikt.spec.ts` (parity test against real navikt).
 */
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

const ADD_PROVIDER = 'Add authentication provider'

async function createProvider(
  apiURL: string,
  token: string,
  name: string,
  providerType: 'oidc' | 'oauth2',
) {
  const res = await fetch(`${apiURL}/api/admin/auth-providers`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({
      name,
      provider_type: providerType,
      enabled: false,
      config: {
        client_id: 'x',
        client_secret: 'y',
        issuer_url: 'https://idp.invalid/oidc',
        authorization_url: 'https://idp.invalid/authorize',
        token_url: 'https://idp.invalid/token',
        scopes: ['openid'],
      },
    }),
  })
  if (!res.ok) throw new Error(`create ${name}: ${res.status} ${await res.text()}`)
}

test.describe('Auth providers — admin CRUD UI', () => {
  test('pre-seeded providers (google/microsoft/apple) show as disabled', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    // All three pre-seeded providers from migration 47 render as rows,
    // each with a `Toggle <name>` switch that starts OFF (disabled).
    for (const name of ['google', 'microsoft', 'apple']) {
      const toggle = page.getByRole('switch', { name: `Toggle ${name}` })
      await expect(toggle).toBeVisible({ timeout: 10_000 })
      await expect(toggle).not.toBeChecked()
    }

    // Each disabled provider shows the "(Disabled)" marker.
    const disabled = await page.getByText('(Disabled)').count()
    expect(disabled).toBeGreaterThanOrEqual(3)
  })

  test('Add Provider menu omits already-added templates, offers generic ones', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    await page.getByRole('button', { name: ADD_PROVIDER }).click()

    // Generic templates (no name collision) are offered.
    await expect(
      page.getByRole('menuitem', { name: /Generic OIDC/i }),
    ).toBeVisible({ timeout: 5_000 })
    await expect(
      page.getByRole('menuitem', { name: /Generic OAuth 2/i }),
    ).toBeVisible()

    // google/microsoft/apple are seeded (migration 47) → filtered OUT
    // of the menu entirely (the admin edits the existing row instead).
    await expect(
      page.getByRole('menuitem', { name: 'Google', exact: true }),
    ).toHaveCount(0)
    await expect(
      page.getByRole('menuitem', { name: 'Apple', exact: true }),
    ).toHaveCount(0)
  })

  test('create + delete a Generic OIDC provider', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    const providerName = `e2e-okta-${Date.now()}`

    // -------------------- CREATE --------------------
    await page.getByRole('button', { name: ADD_PROVIDER }).click()
    await page.getByRole('menuitem', { name: /Generic OIDC/i }).click()

    await expect(
      page.getByRole('button', { name: /^Create$/ }),
    ).toBeVisible({ timeout: 10_000 })

    await page.getByLabel(/Name \(URL slug\)/i).fill(providerName)
    await page.getByLabel(/Client ID/i).fill('e2e-client-id')
    await page.locator('input[type="password"]').first().fill('e2e-secret-value')
    await page
      .getByLabel(/Issuer URL/i)
      .fill('https://nonexistent.invalid/oidc')

    await page.getByRole('button', { name: /^Create$/ }).click()

    // New provider appears as a row (its `Toggle <name>` switch).
    await expect(
      page.getByRole('switch', { name: `Toggle ${providerName}` }),
    ).toBeVisible({ timeout: 10_000 })

    // -------------------- EDIT drawer briefly --------------------
    // Open via the per-row "Edit <name>" action; name field is
    // disabled in edit mode. Then close without saving.
    await page.getByRole('button', { name: `Edit ${providerName}` }).click()
    await expect(page.getByLabel(/Name \(URL slug\)/i)).toBeDisabled({
      timeout: 5_000,
    })
    await page.getByRole('button', { name: /^Cancel$/ }).click()

    // -------------------- DELETE --------------------
    // The per-row "Delete <name>" button opens an inline AntD
    // Popconfirm (not role=dialog) — confirm via its primary danger
    // button (stable `.ant-btn-primary` class).
    await page.getByRole('button', { name: `Delete ${providerName}` }).click()
    const popover = page.locator('.ant-popover:visible').last()
    await expect(popover).toBeVisible({ timeout: 5_000 })
    await popover.locator('.ant-btn-primary').click()

    // Row gone (generous timeout — delete includes DB write + reload).
    await expect(
      page.getByRole('switch', { name: `Toggle ${providerName}` }),
    ).toHaveCount(0, { timeout: 30_000 })
  })

  test('Test config button in the drawer surfaces inline result', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    // Open the drawer via "Add provider → Generic OIDC".
    await page.getByRole('button', { name: ADD_PROVIDER }).click()
    await page.getByRole('menuitem', { name: /Generic OIDC/i }).click()

    await page.getByLabel(/Name \(URL slug\)/i).fill(`e2e-test-config-${Date.now()}`)
    await page.getByLabel(/Client ID/i).fill('any-client')
    await page.locator('input[type="password"]').first().fill('any-secret')
    await page
      .getByLabel(/Issuer URL/i)
      .fill('https://nonexistent.invalid/oidc')

    // Click "Test config" — backend tries discovery, fails fast.
    await page.getByRole('button', { name: /Test config/i }).click()

    // Inline alert appears with the failure (we accept either the
    // "Configuration issues" Alert title or the underlying message).
    await expect(
      page.getByText(/Configuration (issues|OK)/i),
    ).toBeVisible({ timeout: 15_000 })

    // Drawer is still open — Test config doesn't close it.
    await expect(page.getByRole('button', { name: /^Create$/ })).toBeVisible()

    // Cleanup: close without saving.
    await page.getByRole('button', { name: /^Cancel$/ }).click()
  })

  // audit id c922bb2133d9 — the delete Popconfirm's cascade warning
  // ("Linked users lose this sign-in method; their accounts remain.",
  // AuthProvidersListSection.tsx:134-150) was never asserted; the existing
  // delete test confirms blindly without surfacing the cascade affordance.
  test('delete Popconfirm surfaces the user-link cascade warning', async ({
  test('Generic OAuth 2 template renders OAuth2-specific config fields', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    const providerName = `e2e-cascade-${Date.now()}`

    // Create a fresh provider to delete.
    await page.getByRole('button', { name: ADD_PROVIDER }).click()
    await page.getByRole('menuitem', { name: /Generic OIDC/i }).click()
    await expect(page.getByRole('button', { name: /^Create$/ })).toBeVisible({
      timeout: 10_000,
    })
    await page.getByLabel(/Name \(URL slug\)/i).fill(providerName)
    await page.getByLabel(/Client ID/i).fill('e2e-client-id')
    await page.locator('input[type="password"]').first().fill('e2e-secret-value')
    await page.getByLabel(/Issuer URL/i).fill('https://nonexistent.invalid/oidc')
    await page.getByRole('button', { name: /^Create$/ }).click()
    await expect(
      page.getByRole('switch', { name: `Toggle ${providerName}` }),
    ).toBeVisible({ timeout: 10_000 })

    // Open the per-row delete Popconfirm and assert the cascade-warning copy.
    await page.getByRole('button', { name: `Delete ${providerName}` }).click()
    const popover = page.locator('.ant-popover:visible').last()
    await expect(popover).toBeVisible({ timeout: 5_000 })
    await expect(
      popover.getByText(
        'Linked users lose this sign-in method; their accounts remain.',
      ),
    ).toBeVisible()

    // Confirm → the real delete (cascade of user_auth_links) runs and the row
    // disappears.
    await popover.locator('.ant-btn-primary').click()
    await expect(
      page.getByRole('switch', { name: `Toggle ${providerName}` }),
    ).toHaveCount(0, { timeout: 30_000 })
  })

  // audit id 60e73a973f45 — provider name collision: the backend enforces a
  // UNIQUE auth_providers.name (initial_schema). Creating a Generic OIDC whose
  // slug duplicates a seeded provider ("google") must surface an error to the
  // user (the catch → message.error path), not silently create a second row.
  test('creating a provider with a duplicate name shows an error', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    // The seeded "google" row exists (migration 47).
    await expect(
      page.getByRole('switch', { name: 'Toggle google' }),
    ).toBeVisible({ timeout: 10_000 })

    // Add a Generic OIDC and type the colliding slug "google".
    await page.getByRole('button', { name: ADD_PROVIDER }).click()
    await page.getByRole('menuitem', { name: /Generic OIDC/i }).click()
    await expect(page.getByRole('button', { name: /^Create$/ })).toBeVisible({
      timeout: 10_000,
    })
    await page.getByLabel(/Name \(URL slug\)/i).fill('google')
    await page.getByLabel(/Client ID/i).fill('e2e-client-id')
    await page.locator('input[type="password"]').first().fill('e2e-secret-value')
    await page.getByLabel(/Issuer URL/i).fill('https://nonexistent.invalid/oidc')
    await page.getByRole('button', { name: /^Create$/ }).click()

    // A duplicate-name error toast surfaces and the drawer stays open (create
    // did not succeed).
    await expect(page.locator('.ant-message-error')).toBeVisible({
      timeout: 10_000,
    })
    await expect(page.getByRole('button', { name: /^Create$/ })).toBeVisible()
    await page.getByRole('button', { name: ADD_PROVIDER }).click()
    await page.getByRole('menuitem', { name: /Generic OAuth 2/i }).click()

    await expect(
      page.getByRole('button', { name: /^Create$/ }),
    ).toBeVisible({ timeout: 10_000 })

    // OAuth2Fields() are the discriminating fields (OIDC has Issuer URL
    // instead). The three endpoint URLs + the button label are unique to it.
    await expect(page.getByLabel('Authorization URL')).toBeVisible()
    await expect(page.getByLabel('Token URL')).toBeVisible()
    await expect(page.getByLabel('UserInfo URL')).toBeVisible()
    await expect(page.getByLabel('Button label')).toBeVisible()
    // It is NOT an OIDC provider, so there's no Issuer URL field.
    await expect(page.getByLabel(/Issuer URL/i)).toHaveCount(0)

    await page.getByRole('button', { name: /^Cancel$/ }).click()
  })

  // audit id all-0b856b17fa63 — the name slug validation (AuthProviderEditDrawer
  // Form.Item name rule: pattern /^[a-z0-9-]+$/) was untested via the UI. An
  // invalid name (uppercase/spaces) must surface the inline rule error and block
  // Create (client-side, before any request).
  test('invalid provider name slug is rejected with an inline error', async ({
  test('editing the seeded Apple provider renders Apple-specific config fields', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    await page.getByRole('button', { name: ADD_PROVIDER }).click()
    await page.getByRole('menuitem', { name: /Generic OIDC/i }).click()
    await expect(page.getByRole('button', { name: /^Create$/ })).toBeVisible({
      timeout: 10_000,
    })

    // An invalid slug: uppercase + space + punctuation.
    await page.getByLabel(/Name \(URL slug\)/i).fill('Invalid Name!')
    await page.getByRole('button', { name: /^Create$/ }).click()

    // The antd Form rule fires inline; the drawer stays open (no row created).
    await expect(
      page.getByText(/Lowercase letters, digits, and hyphens only/i),
    ).toBeVisible({ timeout: 5_000 })
    await expect(page.getByRole('button', { name: /^Create$/ })).toBeVisible()

    await page.getByRole('button', { name: /^Cancel$/ }).click()
  })

  // audit id all-e452a4f689b8 — when EVERY provider template is already present
  // the "Add provider" button is disabled with an "All providers taken" tooltip
  // (AddProviderMenu.tsx:34-35). google/microsoft/apple are seeded; creating the
  // two generic-named providers takes the remaining templates → all 5 taken.
  test('Add provider button is disabled when every template is taken', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    // Occupy the two generic template keys (google/microsoft/apple are seeded).
    await createProvider(apiURL, token, 'oidc-generic', 'oidc')
    await createProvider(apiURL, token, 'oauth2-generic', 'oauth2')

    await page.goto(`${baseURL}/settings/auth-providers`)
    const addBtn = page.getByRole('button', { name: ADD_PROVIDER })
    await expect(addBtn).toBeVisible({ timeout: 30000 })
    await expect(addBtn).toBeDisabled()
  })
    // Apple is seeded (migration 47); open its edit drawer to reach AppleFields.
    await page.getByRole('button', { name: 'Edit apple' }).click()

    await expect(page.getByLabel('Team ID')).toBeVisible({ timeout: 10_000 })
    await expect(page.getByLabel('Services ID')).toBeVisible()
    await expect(page.getByLabel('Key ID')).toBeVisible()
    await expect(page.getByLabel('Private key path on disk')).toBeVisible()

    await page.getByRole('button', { name: /^Cancel$/ }).click()
  })
})
