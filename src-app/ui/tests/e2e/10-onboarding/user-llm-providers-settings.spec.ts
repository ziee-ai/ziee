import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E for the per-user LLM-provider API-key settings page
 * (/settings/user-llm-providers). Built-in providers are disabled by default,
 * so the test seeds one enabled provider via the admin API, then drives the
 * real save/delete against the test backend.
 */

// A provider is visible to a user only if it's enabled AND assigned to a group
// the user belongs to. The setup-created admin is a member of both the
// "Administrators" and "Users" groups, so we assign the seeded provider to
// "Users".
async function seedEnabledProvider(apiURL: string, token: string, name: string) {
  const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }

  const createRes = await fetch(`${apiURL}/api/llm-providers`, {
    method: 'POST',
    headers: auth,
    body: JSON.stringify({ name, provider_type: 'openai', enabled: true, api_key: 'sk-admin-seed' }),
  })
  if (!createRes.ok) throw new Error(`create provider failed: ${createRes.status} ${await createRes.text()}`)
  const provider = await createRes.json()

  const groupsRes = await fetch(`${apiURL}/api/groups`, { headers: auth })
  if (!groupsRes.ok) throw new Error(`list groups failed: ${groupsRes.status}`)
  const { groups } = await groupsRes.json()
  const usersGroup = groups.find((g: any) => g.name === 'Users')
  if (!usersGroup) throw new Error('Users group not found')

  const assignRes = await fetch(`${apiURL}/api/llm-providers/${provider.id}/groups`, {
    method: 'POST',
    headers: auth,
    body: JSON.stringify({ group_id: usersGroup.id }),
  })
  if (!assignRes.ok) throw new Error(`assign to group failed: ${assignRes.status} ${await assignRes.text()}`)
}

test.describe('User LLM-provider key settings', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    const token = await getAdminToken(testInfra.apiURL)
    await seedEnabledProvider(testInfra.apiURL, token, 'E2E Provider')
  })

  test('save and remove a personal API key', async ({ page, testInfra }) => {
    await page.goto(`${testInfra.baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    // The seeded provider is auto-selected; its form is shown.
    await expect(page.getByRole('heading', { name: 'E2E Provider' })).toBeVisible({ timeout: 15000 })

    // Save a personal key.
    await page.locator('input[type="password"]').fill('sk-my-personal-key')
    await page.getByRole('button', { name: 'Save Key' }).click()

    // The "Your key configured" tag + Remove Key button appear.
    await expect(page.getByText('Your key configured')).toBeVisible({ timeout: 10000 })
    await expect(page.getByRole('button', { name: 'Remove Key' })).toBeVisible()

    // Remove it.
    await page.getByRole('button', { name: 'Remove Key' }).click()

    // Tag reverts (no longer the user's key).
    await expect(page.getByText('Your key configured')).toBeHidden({ timeout: 10000 })
    await expect(page.getByRole('button', { name: 'Save Key' })).toBeVisible()
  })

  test('local providers are not listed on the personal-keys page', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }

    // Seed an enabled LOCAL provider assigned to "Users" (beforeEach already
    // seeded the remote "E2E Provider"). Local providers authenticate via an
    // internal proxy token, so they must not appear on the personal-key page.
    const localRes = await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ name: 'Local Only Provider', provider_type: 'local', enabled: true }),
    })
    if (!localRes.ok) throw new Error(`local create failed: ${localRes.status} ${await localRes.text()}`)
    const local = await localRes.json()

    const groupsRes = await fetch(`${apiURL}/api/groups`, { headers: auth })
    const { groups } = await groupsRes.json()
    const usersGroup = groups.find((g: any) => g.name === 'Users')
    const assignRes = await fetch(`${apiURL}/api/llm-providers/${local.id}/groups`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ group_id: usersGroup.id }),
    })
    if (!assignRes.ok) throw new Error(`assign failed: ${assignRes.status} ${await assignRes.text()}`)

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    // The remote provider is listed (its name renders in both the menu and the
    // detail header → use .first()); the local one is filtered out entirely.
    await expect(page.getByText('E2E Provider').first()).toBeVisible({ timeout: 15000 })
    await expect(page.getByText('Local Only Provider', { exact: true })).toHaveCount(0)
  })
})
