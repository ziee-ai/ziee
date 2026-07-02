import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToProvidersPage,
  waitForProvidersPageLoad,
} from './helpers/navigation-helpers'
import { createLocalProvider } from './helpers/provider-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the LLM Providers settings sidebar with only the seeded built-in
 * providers present.
 *
 * BY DESIGN the deployment always ships 7 undeletable built-in providers
 * (OpenAI/Anthropic/Groq/Gemini/Mistral/DeepSeek/Local) — they back the
 * per-user-keys model, so the DELETE API rejects them (400) and the admin
 * sidebar is never empty. This test drives the real transition: it creates a
 * user provider through the UI, deletes it via the REST API (which succeeds),
 * confirms a built-in delete is refused, then reloads and asserts the sidebar
 * has collapsed back to exactly the built-ins + "Add Provider" — the
 * user-created row is gone but the built-ins remain.
 */

const BUILT_IN_NAMES = [
  'OpenAI',
  'Anthropic',
  'Groq',
  'Google Gemini',
  'Mistral AI',
  'DeepSeek',
  'Local',
]

async function tokenFromPage(page: import('@playwright/test').Page): Promise<string> {
  return page.evaluate(
    () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
  )
}

async function listProviders(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
): Promise<Array<{ id: string; name: string; built_in: boolean }>> {
  const res = await page.request.get(`${apiURL}/api/llm-providers?page=1&per_page=100`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  expect(res.ok(), `list providers: ${res.status()}`).toBeTruthy()
  const body = await res.json()
  return body.providers ?? []
}

test.describe('LLM Providers - built-ins always present', () => {
  test('a user provider can be deleted; the built-ins remain and are undeletable', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Create a user provider through the UI and confirm it renders in the
    // sidebar alongside the always-present built-ins.
    const providerName = `EmptyState_${Date.now().toString(36)}`
    await createLocalProvider(page, baseURL, providerName, 'empty-state probe')
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)
    await expect(
      page.locator('[data-testid^="llm-provider-nav-"]').filter({ hasText: providerName }),
    ).toBeVisible()
    // The seeded built-ins render too (sample two of them).
    await expect(
      page.locator('[data-testid^="llm-provider-nav-"]').filter({ hasText: 'OpenAI' }),
    ).toBeVisible()
    await expect(
      page.locator('[data-testid^="llm-provider-nav-"]').filter({ hasText: 'Anthropic' }),
    ).toBeVisible()

    const token = await tokenFromPage(page)
    const providers = await listProviders(page, apiURL, token)
    const builtIns = providers.filter(p => p.built_in)
    const userProviders = providers.filter(p => !p.built_in)

    // A built-in delete is refused by design (400); the row survives.
    expect(builtIns.length).toBeGreaterThan(0)
    const builtInDel = await page.request.delete(
      `${apiURL}/api/llm-providers/${builtIns[0].id}`,
      { headers: { Authorization: `Bearer ${token}` } },
    )
    expect(builtInDel.status(), 'built-in delete must be rejected').toBe(400)

    // User-created providers delete successfully.
    for (const p of userProviders) {
      const del = await page.request.delete(
        `${apiURL}/api/llm-providers/${p.id}`,
        { headers: { Authorization: `Bearer ${token}` } },
      )
      expect(del.ok(), `delete ${p.id}: ${del.status()}`).toBeTruthy()
    }

    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    // The user-created row is gone...
    await expect(
      page.locator('[data-testid^="llm-provider-nav-"]').filter({ hasText: providerName }),
    ).toHaveCount(0)

    // ...but every built-in still renders, plus the "Add Provider" affordance.
    for (const name of BUILT_IN_NAMES) {
      await expect(
        page.locator('[data-testid^="llm-provider-nav-"]').filter({ hasText: name }),
      ).toBeVisible()
    }
    await expect(byTestId(page, 'llm-provider-nav-add-provider')).toBeVisible()
  })
})
