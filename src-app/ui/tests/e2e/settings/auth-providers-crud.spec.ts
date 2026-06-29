/**
 * Admin CRUD E2E for /settings/auth-providers — exercises the real
 * backend (no API mocks). The Test endpoint will fail for the fake
 * URLs we use, but that's the POINT for this test: we're verifying
 * the UI handles the failure gracefully + persists the result.
 *
 * UI shape (post settings UX overhaul): providers render as a Card of
 * rows, each row carries a per-row toggle Switch + Test/Edit/Delete
 * actions (testids keyed by the provider slug). "Add provider" is a `+`
 * icon button opening a dropdown of templates; templates whose name is
 * already taken (google/microsoft/apple seeded by migration 47) are
 * filtered OUT of the menu.
 *
 * Out of scope: the actual OAuth dance — covered by
 * `social-login-navikt.spec.ts` (parity test against real navikt).
 */
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

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
    // each with a toggle Switch that starts OFF (disabled), and each
    // shows its "(Disabled)" marker.
    for (const name of ['google', 'microsoft', 'apple']) {
      const toggle = byTestId(page, `authprov-toggle-switch-${name}`)
      await expect(toggle).toBeVisible({ timeout: 10_000 })
      await expect(toggle).not.toBeChecked()
      await expect(
        byTestId(page, `authprov-disabled-marker-${name}`),
      ).toBeVisible()
    }
  })

  test('Add Provider menu omits already-added templates, offers generic ones', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    await byTestId(page, 'authprov-add-button').click()

    // Generic templates (no name collision) are offered.
    await expect(
      byTestId(page, 'authprov-add-dropdown-item-oidc-generic'),
    ).toBeVisible({ timeout: 5_000 })
    await expect(
      byTestId(page, 'authprov-add-dropdown-item-oauth2-generic'),
    ).toBeVisible()

    // google/microsoft/apple are seeded (migration 47) → filtered OUT
    // of the menu entirely (the admin edits the existing row instead).
    await expect(
      byTestId(page, 'authprov-add-dropdown-item-google'),
    ).toHaveCount(0)
    await expect(
      byTestId(page, 'authprov-add-dropdown-item-apple'),
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
    await byTestId(page, 'authprov-add-button').click()
    await byTestId(page, 'authprov-add-dropdown-item-oidc-generic').click()

    await expect(byTestId(page, 'authprov-drawer-save-button')).toBeVisible({
      timeout: 10_000,
    })

    await byTestId(page, 'authprov-name-input').fill(providerName)
    await byTestId(page, 'authprov-oidc-client-id-input').fill('e2e-client-id')
    await byTestId(page, 'authprov-oidc-client-secret-input').fill('e2e-secret-value')
    await byTestId(page, 'authprov-oidc-issuer-url-input').fill(
      'https://nonexistent.invalid/oidc',
    )

    await byTestId(page, 'authprov-drawer-save-button').click()

    // New provider appears as a row.
    await expect(
      byTestId(page, `authprov-toggle-switch-${providerName}`),
    ).toBeVisible({ timeout: 10_000 })

    // -------------------- EDIT drawer briefly --------------------
    // Open via the per-row Edit action; name field is disabled in edit
    // mode. Then close without saving.
    await byTestId(page, `authprov-edit-button-${providerName}`).click()
    await expect(byTestId(page, 'authprov-name-input')).toBeDisabled({
      timeout: 5_000,
    })
    await byTestId(page, 'authprov-drawer-cancel-button').click()

    // -------------------- DELETE --------------------
    await byTestId(page, `authprov-delete-button-${providerName}`).click()
    await byTestId(
      page,
      `authprov-delete-confirm-${providerName}-confirm`,
    ).click()

    // Row gone (generous timeout — delete includes DB write + reload).
    await expect(
      byTestId(page, `authprov-toggle-switch-${providerName}`),
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
    await byTestId(page, 'authprov-add-button').click()
    await byTestId(page, 'authprov-add-dropdown-item-oidc-generic').click()

    await byTestId(page, 'authprov-name-input').fill(`e2e-test-config-${Date.now()}`)
    await byTestId(page, 'authprov-oidc-client-id-input').fill('any-client')
    await byTestId(page, 'authprov-oidc-client-secret-input').fill('any-secret')
    await byTestId(page, 'authprov-oidc-issuer-url-input').fill(
      'https://nonexistent.invalid/oidc',
    )

    // Click "Test config" — backend tries discovery, fails fast.
    await byTestId(page, 'authprov-test-config-button').click()

    // Inline test-result alert appears (either "Configuration issues" or OK).
    await expect(
      byTestId(page, 'authprov-drawer-testresult-alert'),
    ).toBeVisible({ timeout: 15_000 })

    // Drawer is still open — Test config doesn't close it.
    await expect(byTestId(page, 'authprov-drawer-save-button')).toBeVisible()

    // Cleanup: close without saving.
    await byTestId(page, 'authprov-drawer-cancel-button').click()
  })

  // audit id c922bb2133d9 — the delete confirm's cascade warning
  // ("Linked users lose this sign-in method; their accounts remain.")
  // was never asserted; the existing delete test confirms blindly.
  test('delete confirm surfaces the user-link cascade warning', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    const providerName = `e2e-cascade-${Date.now()}`

    // Create a fresh provider to delete.
    await byTestId(page, 'authprov-add-button').click()
    await byTestId(page, 'authprov-add-dropdown-item-oidc-generic').click()
    await expect(byTestId(page, 'authprov-drawer-save-button')).toBeVisible({
      timeout: 10_000,
    })
    await byTestId(page, 'authprov-name-input').fill(providerName)
    await byTestId(page, 'authprov-oidc-client-id-input').fill('e2e-client-id')
    await byTestId(page, 'authprov-oidc-client-secret-input').fill('e2e-secret-value')
    await byTestId(page, 'authprov-oidc-issuer-url-input').fill(
      'https://nonexistent.invalid/oidc',
    )
    await byTestId(page, 'authprov-drawer-save-button').click()
    await expect(
      byTestId(page, `authprov-toggle-switch-${providerName}`),
    ).toBeVisible({ timeout: 10_000 })

    // Open the per-row delete confirm and assert the cascade-warning dialog.
    await byTestId(page, `authprov-delete-button-${providerName}`).click()
    const confirm = byTestId(page, `authprov-delete-confirm-${providerName}`)
    await expect(confirm).toBeVisible({ timeout: 5_000 })
    await expect(
      confirm.getByText(
        'Linked users lose this sign-in method; their accounts remain.',
      ),
    ).toBeVisible()

    // Confirm → the real delete (cascade of user_auth_links) runs and the row
    // disappears.
    await byTestId(
      page,
      `authprov-delete-confirm-${providerName}-confirm`,
    ).click()
    await expect(
      byTestId(page, `authprov-toggle-switch-${providerName}`),
    ).toHaveCount(0, { timeout: 30_000 })
  })

  // audit id 60e73a973f45 — provider name collision: the backend enforces a
  // UNIQUE auth_providers.name. Creating a Generic OIDC whose slug duplicates a
  // seeded provider ("google") must surface an error (the catch → toast path),
  // not silently create a second row.
  test('creating a provider with a duplicate name shows an error', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    // The seeded "google" row exists (migration 47).
    await expect(
      byTestId(page, 'authprov-toggle-switch-google'),
    ).toBeVisible({ timeout: 10_000 })

    // Add a Generic OIDC and type the colliding slug "google".
    await byTestId(page, 'authprov-add-button').click()
    await byTestId(page, 'authprov-add-dropdown-item-oidc-generic').click()
    await expect(byTestId(page, 'authprov-drawer-save-button')).toBeVisible({
      timeout: 10_000,
    })
    await byTestId(page, 'authprov-name-input').fill('google')
    await byTestId(page, 'authprov-oidc-client-id-input').fill('e2e-client-id')
    await byTestId(page, 'authprov-oidc-client-secret-input').fill('e2e-secret-value')
    await byTestId(page, 'authprov-oidc-issuer-url-input').fill(
      'https://nonexistent.invalid/oidc',
    )
    await byTestId(page, 'authprov-drawer-save-button').click()

    // A duplicate-name error toast surfaces and the drawer stays open (create
    // did not succeed).
    await expect(
      page.locator('[data-sonner-toast][data-type="error"]'),
    ).toBeVisible({ timeout: 10_000 })
    await expect(byTestId(page, 'authprov-drawer-save-button')).toBeVisible()

    await byTestId(page, 'authprov-drawer-cancel-button').click()
  })

  // audit id all-0b856b17fa63 — the name slug validation (pattern /^[a-z0-9-]+$/)
  // was untested via the UI. An invalid name (uppercase/spaces) must surface the
  // inline rule error and block Create (client-side, before any request).
  test('invalid provider name slug is rejected with an inline error', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    await byTestId(page, 'authprov-add-button').click()
    await byTestId(page, 'authprov-add-dropdown-item-oidc-generic').click()
    await expect(byTestId(page, 'authprov-drawer-save-button')).toBeVisible({
      timeout: 10_000,
    })

    // An invalid slug: uppercase + space + punctuation.
    await byTestId(page, 'authprov-name-input').fill('Invalid Name!')
    await byTestId(page, 'authprov-drawer-save-button').click()

    // The Form rule fires inline (role="alert" FieldError); the drawer stays
    // open (no row created).
    await expect(
      byTestId(page, 'authprov-drawer-form').getByRole('alert').first(),
    ).toBeVisible({ timeout: 5_000 })
    await expect(byTestId(page, 'authprov-drawer-save-button')).toBeVisible()

    await byTestId(page, 'authprov-drawer-cancel-button').click()
  })

  // audit id all-e452a4f689b8 — when EVERY provider template is already present
  // the "Add provider" button is disabled with an "All providers taken" tooltip.
  // google/microsoft/apple are seeded; creating the two generic-named providers
  // takes the remaining templates → all 5 taken.
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
    const addBtn = byTestId(page, 'authprov-add-button')
    await expect(addBtn).toBeVisible({ timeout: 30000 })
    await expect(addBtn).toBeDisabled()
  })
})
