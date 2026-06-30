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
import { byTestId } from '../testid.ts'

test.describe('Auth providers — Add button disabled when all types taken', () => {
  test('configuring every remaining template disables the Add menu', async ({
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
      const addBtn = byTestId(page, 'authprov-add-button')
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
        byTestId(page, 'authprov-toggle-switch-oidc-generic'),
      ).toBeVisible({ timeout: 10_000 })
      await expect(
        byTestId(page, 'authprov-toggle-switch-oauth2-generic'),
      ).toBeVisible()

      // All templates taken → the Add button is now DISABLED. (The explanatory
      // tooltip can't be asserted via hover: a disabled button carries
      // `pointer-events: none`, so it never receives the pointer events a
      // Radix tooltip needs — the disabled state itself is the behavior.)
      const disabledAdd = byTestId(page, 'authprov-add-button')
      await expect(disabledAdd).toBeDisabled()
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
  })
})
