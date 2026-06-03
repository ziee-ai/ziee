import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E for the first-run onboarding wizard.
 *
 * A freshly-created user has empty completed_onboarding_ids, so AuthGuard
 * redirects them to /onboarding. The "getting-started" guide's steps are all
 * skippable, so the happy path clicks straight through; the wizard's real
 * actions (saving keys / installing MCP) are covered by focused tests.
 */

const GUIDE = 'getting-started'

async function freshUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  // profile::read lets the user mark onboarding steps/guides complete.
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
  ])
  return { username, adminToken }
}

test.describe('Onboarding wizard', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    // Creates the admin (and completes its onboarding so it isn't trapped).
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('fresh user is redirected into the wizard', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const { username } = await freshUser(apiURL, 'redir')

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    await expect(page).toHaveURL(new RegExp(`/onboarding`))
    await expect(page.getByRole('heading', { name: 'Getting Started' })).toBeVisible()
    // First step renders.
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()
  })

  test('stepping through the wizard completes onboarding and lands on chat', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const { username } = await freshUser(apiURL, 'flow')

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()

    // AI Providers → MCP Servers
    await expect(page.getByRole('heading', { name: 'AI Providers' })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()

    // MCP Servers → Finish
    await expect(page.getByRole('heading', { name: 'MCP Servers' })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()

    // Finish → chat
    await expect(page.getByRole('heading', { name: /all set/i })).toBeVisible()
    await page.getByRole('button', { name: 'Start Chatting' }).click()

    await expect(page).toHaveURL(new RegExp(`/chat`), { timeout: 15000 })

    // AuthGuard release: navigating to / no longer bounces to /onboarding.
    await page.goto(`${baseURL}/`)
    await page.waitForLoadState('networkidle')
    expect(page.url()).not.toContain('/onboarding')
  })

  test('wizard resumes at the first incomplete step after a reload', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `resume_${Date.now().toString(36)}`
    await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
      'profile::read',
      'profile::edit',
    ])

    // Mark the first step (welcome) complete via the API before the user lands.
    const userToken = await getAdminToken(apiURL, { username, password: 'password123' })
    const res = await fetch(`${apiURL}/api/onboarding/${GUIDE}/steps/welcome/complete`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${userToken}` },
    })
    expect(res.ok).toBeTruthy()

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // With welcome done, the wizard opens at AI Providers, not Welcome.
    await expect(page.getByRole('heading', { name: 'AI Providers' })).toBeVisible()
    // Sanity: not still on the welcome step.
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeHidden()
  })

  test('the AI Providers step omits local providers from the key list', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${adminToken}` }

    // New users auto-join the default group; assign the seeded providers there
    // so the fresh user sees them in the AI Providers key step.
    const groupsRes = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, { headers: auth })
    const { groups } = await groupsRes.json()
    const defaultGroup =
      groups.find((g: any) => g.is_default) ?? groups.find((g: any) => g.name === 'Users')

    const suffix = Date.now().toString(36)
    const remoteName = `Remote Onb ${suffix}`
    const localName = `Local Onb ${suffix}`

    const seed = async (name: string, body: object) => {
      const r = await fetch(`${apiURL}/api/llm-providers`, {
        method: 'POST',
        headers: auth,
        body: JSON.stringify(body),
      })
      if (!r.ok) throw new Error(`create ${name} failed: ${r.status} ${await r.text()}`)
      const p = await r.json()
      const a = await fetch(`${apiURL}/api/llm-providers/${p.id}/groups`, {
        method: 'POST',
        headers: auth,
        body: JSON.stringify({ group_id: defaultGroup.id }),
      })
      if (!a.ok) throw new Error(`assign ${name} failed: ${a.status} ${await a.text()}`)
    }

    await seed(remoteName, {
      name: remoteName,
      provider_type: 'openai',
      enabled: true,
      api_key: 'sk-onb',
    })
    await seed(localName, { name: localName, provider_type: 'local', enabled: true })

    // Fresh user → onboarding.
    const { username } = await freshUser(apiURL, 'apikeys')
    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers.
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()
    await page.getByRole('button', { name: 'Next' }).click()
    await expect(page.getByRole('heading', { name: 'AI Providers' })).toBeVisible()

    // The remote provider is listed in the key step (its name renders in both
    // the menu and the detail header → use .first(); this also guarantees the
    // list has loaded); the local one is filtered out entirely.
    await expect(page.getByText(remoteName).first()).toBeVisible({ timeout: 15000 })
    await expect(page.getByText(localName, { exact: true })).toHaveCount(0)
  })
})
