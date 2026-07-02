import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { assignProviderToAdministratorsGroup } from '../../common/provider-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the USER-facing LLM providers page "No AI providers available" empty
 * state (`UserLlmProvidersPage.tsx:115-129`, route `/settings/user-llm-providers`).
 *
 * Distinct from the ADMIN providers page empty state (`providers-empty-state.spec.ts`):
 * this is the per-user page that lists the providers a user can configure keys
 * for, which renders an antd `<Empty>` with "No AI providers are available yet."
 * when `providers.length === 0`.
 *
 * Each E2E test gets its own fresh database, so a deployment with no provider
 * created deterministically shows zero accessible providers. We assert the empty
 * state, then create a provider via the admin API + reload as a positive control
 * proving the empty state is conditional (not always rendered).
 */

test.describe('User LLM Providers — empty state', () => {
  test('shows "No AI providers available" when the user has no accessible providers', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Fresh DB → no providers created → the page's `providers.length === 0`
    // branch renders the empty state.
    await page.goto(`${baseURL}/settings/user-llm-providers`)

    const empty = byTestId(page, 'ullm-no-providers-empty')
    await expect(empty).toBeVisible({ timeout: 15_000 })
    // The empty state guidance points the user at an administrator.
    await expect(empty).toContainText(/administrator needs to add a provider/i)

    // --- Positive control: once a provider exists, the empty state is gone. ---
    const adminToken = await getAdminToken(apiURL)
    const name = `EmptyStateProbe_${Date.now().toString(36)}`
    const res = await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      // `custom` + no api_key is the only enabled-no-key combination the
      // backend accepts (mirrors user-llm-providers.spec.ts).
      body: JSON.stringify({ name, provider_type: 'custom', enabled: true }),
    })
    expect(res.ok, `create provider: ${res.status}`).toBeTruthy()
    const created = await res.json()

    // The user-facing list only surfaces providers assigned to a group the
    // user belongs to (INNER JOIN user_group_llm_providers), so assign the new
    // provider to the admin's Administrators group before it can appear.
    await assignProviderToAdministratorsGroup(apiURL, adminToken, created.id)

    await page.reload()
    // The provider now appears as a menu item and the empty state is gone.
    await expect(byTestId(page, `ullm-provider-menu-item-${created.id}`)).toBeVisible({ timeout: 15_000 })
    await expect(byTestId(page, 'ullm-no-providers-empty')).toHaveCount(0)
  })
})
