import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  login,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E for the first-run onboarding wizard.
 *
 * A freshly-created user has no onboarding progress (the onboarding store
 * fetches it from GET /api/onboarding/progress after login), so the redirect
 * sends them to /onboarding. The "getting-started" guide's steps are all
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
    await expect(byTestId(page, 'onboarding-guide-title')).toBeVisible()
    // First step renders.
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
  })

  test('stepping through the wizard completes onboarding and lands on chat', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const { username } = await freshUser(apiURL, 'flow')

    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()

    // AI Providers → MCP Servers
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()

    // MCP Servers → Memory
    await expect(byTestId(page, 'onboarding-step-mcp-servers')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()

    // Memory → Finish (the memory module injects a 'memory-setup' step)
    await expect(byTestId(page, 'onboarding-step-memory-setup')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()

    // Finish → chat
    await expect(byTestId(page, 'onboarding-step-finish')).toBeVisible()

    // FinishStep summary (FinishStep.tsx): with no keys entered and no MCP
    // servers selected during this walk-through, both summary lines render their
    // empty-path text. This verifies the finish-step summary that the wizard
    // tests never asserted.
    await expect(byTestId(page, 'onboarding-finish-apikeys-summary')).toContainText(/No API keys added/i)
    await expect(byTestId(page, 'onboarding-finish-mcp-summary')).toContainText(/No MCP servers selected/i)

    await byTestId(page, 'onboarding-page-next-button').click()

    await expect(page).toHaveURL(new RegExp(`/chat`), { timeout: 15000 })

    // AuthGuard release: navigating to / no longer bounces to /onboarding.
    await page.goto(`${baseURL}/`)
    await page.waitForLoadState('load')
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
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()
    // Sanity: not still on the welcome step.
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeHidden()
  })

  test('a user who already completed onboarding is NOT redirected to the wizard', async ({ page, testInfra }) => {
    // Negative case (bypass): once the guide is complete, AuthGuard must land
    // the user in the app and NOT trap them at /onboarding — even on a direct
    // navigation to /onboarding. Guards the OnboardingRedirect early-return.
    const { baseURL, apiURL } = testInfra
    const { username } = await freshUser(apiURL, 'bypass')

    // `login` marks the guide complete via the API, then navigates home.
    await login(page, baseURL, username, 'password123')

    // Landed in the app, not bounced to the wizard.
    await expect(page).not.toHaveURL(/\/onboarding/)
    await expect(byTestId(page, 'chat-input-send-btn')).toBeVisible({
      timeout: 15000,
    })

    // Direct navigation to /onboarding must redirect AWAY (completed guide is
    // not re-entered), not strand the user on the wizard.
    await page.goto(`${baseURL}/onboarding`)
    await page.waitForLoadState('load')
    await expect(page).not.toHaveURL(/\/onboarding/)
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
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()

    // The remote provider is listed in the key step (its name renders in both
    // the menu and the detail header → use .first(); this also guarantees the
    // list has loaded); the local one is filtered out entirely.
    await expect(byTestId(page, 'onboarding-step-api-keys')).toContainText(remoteName, { timeout: 15000 })
    await expect(byTestId(page, 'onboarding-step-api-keys')).not.toContainText(localName)
  })

  test('entering an API key in the AI Providers step saves it', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${adminToken}` }

    const groupsRes = await fetch(`${apiURL}/api/groups?page=1&per_page=100`, { headers: auth })
    const { groups } = await groupsRes.json()
    const defaultGroup =
      groups.find((g: any) => g.is_default) ?? groups.find((g: any) => g.name === 'Users')

    // A remote provider with NO admin key → the user enters their own ("sk-..."
    // placeholder), exercising the onboarding key-save path.
    const remoteName = `KeySave ${Date.now().toString(36)}`
    const created = await fetch(`${apiURL}/api/llm-providers`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ name: remoteName, provider_type: 'openai', enabled: true }),
    })
    const provider = await created.json()
    await fetch(`${apiURL}/api/llm-providers/${provider.id}/groups`, {
      method: 'POST',
      headers: auth,
      body: JSON.stringify({ group_id: defaultGroup.id }),
    })

    const { username } = await freshUser(apiURL, 'keysave')
    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Welcome → AI Providers.
    await expect(byTestId(page, 'onboarding-step-welcome')).toBeVisible()
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-step-api-keys')).toBeVisible()
    await expect(byTestId(page, 'onboarding-step-api-keys')).toContainText(remoteName, { timeout: 15000 })

    // Enter a key, advance → the step's onNext saves it (saveUserApiKey) and the
    // store surfaces an "API key saved" success toast.
    await byTestId(page, 'onboarding-apikeys-password-input').fill('sk-onboarding-user-key-123')
    await byTestId(page, 'onboarding-page-next-button').click()
    await expect(byTestId(page, 'onboarding-apikeys-key-status-tag')).toBeVisible({ timeout: 15000 })
  })

  // Negative coverage for OnboardingRedirect's `if (user.is_admin === true)
  // return` bypass (OnboardingRedirect.tsx:41): an admin is NEVER forced into
  // the wizard. Contrast the "fresh user is redirected into the wizard" test
  // above — a non-admin with incomplete onboarding bounces to /onboarding,
  // while the admin (logged in via the beforeEach) lands on the app shell.
  test('admin is not redirected into the onboarding wizard', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Navigate to a normal in-app route; the redirect, if it fired, would
    // replace the URL with /onboarding before the shell renders.
    await page.goto(`${baseURL}/`)
    await expect(byTestId(page, 'chat-history-new-chat-btn')).toBeVisible({
      timeout: 20000,
    })
    await expect(page).not.toHaveURL(/\/onboarding/)
  })
})

/**
 * Regression for the first-run setup hang: after creating the admin account on
 * the /setup page, the app redirected to home but stuck on the AuthGuard
 * spinner (isInitializing never cleared) until a manual reload. This describe
 * has NO loginAsAdmin beforeEach — it drives the real setup form on the fresh
 * needs_setup=true backend and asserts the app loads WITHOUT any reload.
 */
test.describe('First-run admin setup', () => {
  test('creating the admin lands in the app without a manual reload', async ({ page, testInfra }) => {
    const { baseURL } = testInfra

    // Fresh backend (no admin) → AuthGuard sends us to /setup.
    await page.goto(`${baseURL}/`)
    // First page load of a fresh worker can 504 on a Vite re-bundle; reload
    // once if the setup form doesn't render (mirrors loginAsAdmin). This is a
    // PRE-submit retry, so it doesn't mask the post-submit bug under test.
    try {
      await page.waitForSelector('#setup-form_username', { timeout: 8000 })
    } catch {
      await page.reload({ waitUntil: 'load' })
      await page.waitForSelector('#setup-form_username', { timeout: 30000 })
    }

    const suffix = Date.now().toString(36)
    await page.fill('#setup-form_username', `admin_${suffix}`)
    await page.fill('#setup-form_email', `admin_${suffix}@ex.com`)
    await page.fill('#setup-form_password', 'password123')
    await page.fill('#setup-form_confirm_password', 'password123')
    await byTestId(page, 'app-setup-submit-button').click()

    // CRITICAL: no reload / goto here. Before the Auth.store fix the AuthGuard
    // spinner (isInitializing stuck true) never cleared, so home hung on the
    // loader until a manual reload. The chat composer only renders once
    // AuthGuard releases — its presence proves the post-setup bootstrap
    // completed and the home page actually loaded (without a reload).
    await expect(byTestId(page, 'chat-input-send-btn')).toBeVisible({
      timeout: 20000,
    })
    // Landed on the app home, not bounced back to /setup, with no stuck spinner.
    await expect(page).not.toHaveURL(/\/setup/)
    await expect(byTestId(page, 'chat-input-send-btn')).toBeVisible()
  })
})
