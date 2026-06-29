import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
  clearAuthState,
} from '../../common/auth-helpers'

/**
 * E2E — the full "template clone on signup" journey.
 *
 * Backend (assistant/event_handlers.rs) clones enabled DEFAULT template
 * assistants into every newly-created user. The backend test covers the
 * repo path; this asserts the user-visible end of the journey: an admin
 * creates a default template, a NEW user is created (which triggers the
 * clone), and that user sees the cloned assistant on their own assistants
 * page (a real user assistant they can then select in chat).
 */

test.describe('Assistants — template clone on signup', () => {
  test('a new user receives a clone of the default template', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Admin creates an ENABLED, DEFAULT template — the clone source.
    const tag = Date.now().toString(36)
    const templateName = `Onboarding Helper ${tag}`
    const createRes = await fetch(`${apiURL}/api/assistant-templates`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      body: JSON.stringify({
        name: templateName,
        instructions: 'You are the default onboarding helper.',
        is_default: true,
      }),
    })
    expect(createRes.ok, `create template: ${createRes.status}`).toBeTruthy()

    // A brand-new user — creation fires the clone-on-user-created hook.
    const uname = `clonee_${tag}`
    await createTestUser(
      apiURL,
      adminToken,
      uname,
      `${uname}@example.com`,
      'password123',
      ['profile::read', 'profile::edit', 'assistants::read'],
    )

    // The user logs in and sees the cloned assistant on THEIR assistants page.
    await clearAuthState(page)
    await login(page, baseURL, uname, 'password123')
    await page.goto(`${baseURL}/settings/assistants`)

    // The clone shows up as one of the user's own assistant rows.
    await expect(
      page
        .locator(`[data-test-assistant-id^="user-assistant-"]`, {
          hasText: templateName,
        })
        .first(),
    ).toBeVisible({ timeout: 30000 })

    // Sanity: it is a real USER assistant (a clone), not the template itself —
    // the user can read it via their own (non-admin) assistants list.
    const userToken = await page.evaluate(
      () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const listRes = await fetch(
      `${apiURL}/api/assistants?page=1&limit=100`,
      { headers: { Authorization: `Bearer ${userToken}` } },
    )
    const body = await listRes.json()
    const clone = (body.assistants as Array<{ name: string; is_template: boolean }>).find(
      a => a.name === templateName,
    )
    expect(clone, 'the cloned assistant must be in the user list').toBeTruthy()
    expect(clone!.is_template).toBe(false)
  })
})
