import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// audit id all-e8a9d734fadf — the user-facing providers page was only tested
// with a single (openai/custom) provider. This seeds MULTIPLE real chat
// provider TYPES (openai + anthropic + gemini) and asserts the page lists all
// of them together — the realistic multi-provider configuration a user sees.
async function createTypedProvider(
  apiURL: string,
  adminToken: string,
  name: string,
  providerType: 'openai' | 'anthropic' | 'gemini',
): Promise<string> {
  const res = await fetch(`${apiURL}/api/llm-providers`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${adminToken}` },
    body: JSON.stringify({ name, provider_type: providerType, enabled: true, api_key: 'sk-dummy-key' }),
  })
  if (!res.ok) throw new Error(`create ${providerType}: ${res.status} ${await res.text()}`)
  return (await res.json()).id as string
}

async function assignToDefaultGroup(apiURL: string, adminToken: string, providerId: string) {
  const groups = await (await fetch(`${apiURL}/api/groups`, {
    headers: { Authorization: `Bearer ${adminToken}` },
  })).json()
  const def = groups.groups.find((g: { is_default?: boolean }) => g.is_default)
  const r = await fetch(`${apiURL}/api/llm-providers/${providerId}/groups`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${adminToken}` },
    body: JSON.stringify({ group_id: def.id }),
  })
  if (!r.ok) throw new Error(`assign: ${r.status} ${await r.text()}`)
}

test.describe('User providers page — multiple chat-provider combinations', () => {
  test('lists openai + anthropic + gemini providers together', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const stamp = Date.now()
    const names = {
      openai: `e2e-openai-${stamp}`,
      anthropic: `e2e-anthropic-${stamp}`,
      gemini: `e2e-gemini-${stamp}`,
    }
    for (const [type, name] of Object.entries(names)) {
      const id = await createTypedProvider(apiURL, token, name, type as 'openai' | 'anthropic' | 'gemini')
      await assignToDefaultGroup(apiURL, token, id)
    }

    await page.goto(`${baseURL}/settings/user-llm-providers`)
    await page.waitForLoadState('load')

    // All three provider types appear in the page's provider menu.
    for (const name of Object.values(names)) {
      await expect(
        page.getByRole('menuitem', { name }).first(),
      ).toBeVisible({ timeout: 30000 })
    }
  })
})
