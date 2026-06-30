import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — API key saving during onboarding (ApiKeysStep.tsx:38-44 registerBeforeNext
 * → ApiKeysStep.store.saveKey → POST user API key). The onboarding-wizard happy
 * path clicks through WITHOUT entering a key (FinishStep shows "No API keys
 * added"); the actual save-on-Next path was untested.
 */

async function freshUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
  ])
  return username
}

/**
 * Provision an enabled remote provider (no admin key) and assign it to the
 * default group so a fresh user sees it on the AI-Providers step and can enter
 * their OWN key. The seeded defaults are all enabled=false, so without this the
 * step shows "No AI providers enabled".
 */
async function seedEnabledProvider(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const auth = {
    'Content-Type': 'application/json',
    Authorization: `Bearer ${adminToken}`,
  }
  const groupsRes = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, {
    headers: auth,
  })
  const { groups } = await groupsRes.json()
  const defaultGroup =
    groups.find((g: any) => g.is_default) ??
    groups.find((g: any) => g.name === 'Users')
  const created = await fetch(`${apiURL}/api/llm-providers`, {
    method: 'POST',
    headers: auth,
    body: JSON.stringify({ name, provider_type: 'openai', enabled: true }),
  })
  if (!created.ok)
    throw new Error(`seed provider failed: ${created.status} ${await created.text()}`)
  const provider = await created.json()
  await fetch(`${apiURL}/api/llm-providers/${provider.id}/groups`, {
    method: 'POST',
    headers: auth,
    body: JSON.stringify({ group_id: defaultGroup.id }),
  })
}

test.describe('Onboarding — API key save', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('entering a key on the AI Providers step saves it (Finish shows it added)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await seedEnabledProvider(apiURL, `ApiKeySave ${Date.now().toString(36)}`)
    const username = await freshUser(apiURL, 'apikey')
    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers.
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()

    // Enter a key into the (selected provider's) Password input.
    const keyInput = page.locator('input[type="password"]').first()
    await expect(keyInput).toBeVisible({ timeout: 10000 })
    await keyInput.fill('sk-onboarding-test-key-123')

    // Next runs the beforeNext hook → saves the key. Walk to Finish.
    // The save raises an "API key saved" success toast that renders over the
    // bottom-right Next CTA and (when hovered by a click-retry) stays expanded,
    // intercepting pointer events. Activate Next via the keyboard from here on —
    // focus+Enter doesn't pointer-hit-test, so the toast can't block it.
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-mcp-servers')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').press('Enter')
    await expect(byTestId(page, 'onboarding-step-memory-setup')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').press('Enter')

    // FinishStep summary reflects the saved key (not "No API keys added").
    await expect(byTestId(page, 'onboarding-step-finish')).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'onboarding-finish-apikeys-summary')).toContainText(/API key.*saved/i)
    await expect(byTestId(page, 'onboarding-finish-apikeys-summary')).not.toContainText(/No API keys added/i)
  })
})
