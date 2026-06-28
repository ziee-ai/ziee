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
