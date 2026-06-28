/**
 * E2E — the "all provider types taken" disabled Add button
 * (audit gap all-63169cd2dd9a).
 *
 * `AddProviderMenu.tsx` filters `PROVIDER_TEMPLATES` against the names
 * already configured; when EVERY template key is taken the trigger
 * button is `disabled` and its tooltip reads "All providers taken"
 * (AddProviderMenu.tsx:34-35,40). Migration 47 pre-seeds google /
 * microsoft / apple, so 3 of the 5 templates start taken and the Add
 * button is initially ENABLED. This test creates the remaining two
 * generic templates (named exactly `oidc-generic` / `oauth2-generic`
 * so they collide with the template keys the menu filters on), then
 * asserts the Add button flips to disabled — the previously-untested
 * empty-available-types state. No API mocks: real create + real delete
 * cleanup so suite isolation holds.
 */
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

const ADD_PROVIDER = 'Add authentication provider'

test.describe('Auth providers — Add button disabled when all types taken', () => {
  test('configuring every remaining template disables the Add menu', async ({
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — AddProviderMenu disables the "Add" button when every provider template
 * is already configured (AddProviderMenu.tsx:34-35, tooltip "All providers
 * taken"). The `available` filter matches each template's `key` against the
 * existing provider NAMES, so creating one provider per key exhausts the menu.
 */

const TEMPLATES: { name: string; provider_type: string }[] = [
  { name: 'google', provider_type: 'oidc' },
  { name: 'microsoft', provider_type: 'oidc' },
  { name: 'apple', provider_type: 'apple' },
  { name: 'oidc-generic', provider_type: 'oidc' },
  { name: 'oauth2-generic', provider_type: 'oauth2' },
]

test.describe('Auth providers — all templates taken', () => {
  test('the Add button is disabled once every template name exists', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(apiURL)

    const created: string[] = []
    const createProvider = async (
      name: string,
      providerType: string,
      config: Record<string, unknown>,
    ) => {
      const res = await fetch(`${apiURL}/api/admin/auth-providers`, {
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Create one provider per template key (apple may be pre-seeded by
    // migration 47 → a conflict there is fine, it's already "taken").
    for (const t of TEMPLATES) {
      await fetch(`${apiURL}/api/admin/auth-providers`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({
          name,
          provider_type: providerType,
          enabled: false,
          config,
        }),
      })
      expect(res.ok, `create ${name}: ${res.status}`).toBeTruthy()
      const body = await res.json()
      created.push(body.provider.id as string)
    }

    try {
      await loginAsAdmin(page, baseURL)
      await page.goto(`${baseURL}/settings/auth-providers`)

      // Pre-condition: with only the 3 seeded templates taken, two
      // generic templates remain → the Add button is ENABLED.
      const addBtn = page.getByRole('button', { name: ADD_PROVIDER })
      await expect(addBtn).toBeVisible({ timeout: 10_000 })
      await expect(addBtn).toBeEnabled()

      // Take the last two templates (names must equal the template keys
      // the menu filters on: `oidc-generic` / `oauth2-generic`).
      await createProvider('oidc-generic', 'oidc', {
        client_id: 'e2e-id',
        client_secret: 'e2e-secret',
        issuer_url: 'https://example.invalid',
        scopes: ['openid', 'email', 'profile'],
        attribute_mapping: { user_id: 'sub', username: 'preferred_username', email: 'email' },
      })
      await createProvider('oauth2-generic', 'oauth2', {
        client_id: 'e2e-id',
        client_secret: 'e2e-secret',
        authorization_url: 'https://example.invalid/authorize',
        token_url: 'https://example.invalid/token',
        userinfo_url: 'https://example.invalid/userinfo',
        scopes: ['email', 'profile'],
        attribute_mapping: { user_id: 'sub', username: 'username', email: 'email' },
      })

      // Reload so the provider list (and AddProviderMenu.existingNames)
      // re-hydrates with all five template keys now taken.
      await page.reload()

      // Both new rows are present (the create round-tripped).
      await expect(
        page.getByRole('switch', { name: 'Toggle oidc-generic' }),
      ).toBeVisible({ timeout: 10_000 })
      await expect(
        page.getByRole('switch', { name: 'Toggle oauth2-generic' }),
      ).toBeVisible()

      // All templates taken → the Add button is now DISABLED, and its
      // tooltip explains why.
      const disabledAdd = page.getByRole('button', { name: ADD_PROVIDER })
      await expect(disabledAdd).toBeDisabled()
      await disabledAdd.hover()
      await expect(
        page.getByText('All providers taken', { exact: true }),
      ).toBeVisible({ timeout: 5_000 })
    } finally {
      // Restore the empty-available-types state so the rest of the
      // suite sees the default seeded-only providers.
      for (const id of created) {
        await fetch(`${apiURL}/api/admin/auth-providers/${id}`, {
          method: 'DELETE',
          headers: { Authorization: `Bearer ${token}` },
        }).catch(() => {})
      }
    }
          name: t.name,
          provider_type: t.provider_type,
          config: {},
        }),
      }).catch(() => {})
    }

    await page.goto(`${baseURL}/settings/auth-providers`)
    await page.waitForLoadState('domcontentloaded')

    // The trigger button is disabled with the "All providers taken" tooltip.
    const addBtn = page.getByRole('button', {
      name: 'Add authentication provider',
    })
    await expect(addBtn).toBeVisible({ timeout: 30000 })
    await expect(addBtn).toBeDisabled()
  })
})
