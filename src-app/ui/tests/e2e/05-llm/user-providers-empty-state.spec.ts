import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

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

const EMPTY_TEXT = /No AI providers are available yet/i

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

    await expect(page.getByText(EMPTY_TEXT)).toBeVisible({ timeout: 15_000 })
    // The empty state guidance points the user at an administrator.
    await expect(
      page.getByText(/administrator needs to add a provider/i),
    ).toBeVisible()

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

    await page.reload()
    await expect(page.getByText(name)).toBeVisible({ timeout: 15_000 })
    await expect(page.getByText(EMPTY_TEXT)).toHaveCount(0)
  })
})
