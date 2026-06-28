import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'
import { createProviderViaAPI } from '../../common/provider-helpers'

/**
 * E2E — the onboarding wizard's API-key entry + save (ApiKeysStep.tsx +
 * registerBeforeNext → saveKey).
 *
 * Audit gap: onboarding-wizard.spec steps THROUGH the AI Providers step
 * without ever entering/saving a key. This seeds a provider visible to the
 * onboarding user, types a key, clicks Next, and asserts the
 * POST /api/user-llm-providers/api-keys save fires and the wizard advances.
 */

async function assignProviderToUsersGroup(
  apiURL: string,
  token: string,
  providerId: string,
) {
  const groupsRes = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  const data = await groupsRes.json()
  const groups = Array.isArray(data) ? data : data.groups || []
  const users = groups.find((g: { name: string }) => g.name === 'Users')
  if (!users) throw new Error('default Users group not found')
  const res = await fetch(`${apiURL}/api/groups/${users.id}/providers`, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ provider_ids: [providerId] }),
  })
  if (!res.ok) throw new Error(`assign to Users failed: ${res.status}`)
}

test.describe('Onboarding — API key entry + save', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('entering a key and clicking Next saves it via the user-api-keys endpoint', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    // A provider the onboarding (non-admin) user can see: assign it to Users.
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Onboarding OpenAI',
      'openai',
    )
    await assignProviderToUsersGroup(apiURL, adminToken, providerId)

    const username = `wizkey_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers.
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()
    await expect(
      page.getByRole('heading', { name: 'AI Providers' }),
    ).toBeVisible()

    // Enter a personal API key for the seeded provider.
    await page.getByLabel('Your API Key').fill('sk-onboarding-personal-key')

    // Next triggers registerBeforeNext → saveKey (POST user-api-keys).
    const saveResp = page.waitForResponse(
      r =>
        r.url().includes('/api/user-llm-providers/api-keys') &&
        r.request().method() === 'POST',
      { timeout: 30000 },
    )
    await page.getByRole('button', { name: 'Next' }).click()
    expect((await saveResp).status()).toBeLessThan(400)

    // The wizard advanced past the providers step.
    await expect(
      page.getByRole('heading', { name: 'MCP Servers' }),
    ).toBeVisible({ timeout: 30000 })
  })
})
