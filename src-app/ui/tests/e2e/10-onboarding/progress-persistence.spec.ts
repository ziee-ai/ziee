import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  loginExpectingOnboarding,
} from '../../common/auth-helpers'

/**
 * E2E — onboarding progress PERSISTS across a reload (the UI→API→DB→UI
 * round-trip).
 *
 * Audit gap (all-70778001ec32): `progress-api.spec.ts` already asserts the
 * step/guide complete POSTs *fire* from the wizard buttons, but nothing
 * proves the persisted progress actually survives — i.e. that a step
 * completed via the UI is stored server-side and re-hydrated on the next
 * load so the wizard RESUMES past it instead of restarting at Welcome.
 *
 * This advances one step through the wizard, reloads (wiping all in-memory
 * store state), and asserts BOTH:
 *   (a) GET /api/onboarding/progress reports the completed step id, and
 *   (b) the freshly-mounted wizard resumes at the next step (AI Providers),
 *       not back at Welcome — proving `OnboardingPage`'s resume-point
 *       computation re-hydrated from the persisted `completed_step_ids`.
 */

test.describe('Onboarding — progress persists across reload', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('a step completed via the wizard survives a reload and resumes the wizard', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `progpersist_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      ['profile::read', 'profile::edit'],
    )
    await loginExpectingOnboarding(page, baseURL, username, 'password123')

    // Fresh user starts at the Welcome step.
    await expect(page.getByRole('heading', { name: /Welcome/ })).toBeVisible()

    // Advance one step — "Next" marks the Welcome step complete server-side
    // and moves the wizard to AI Providers.
    const stepComplete = page.waitForResponse(
      r =>
        /\/api\/onboarding\/.+\/steps\/.+\/complete$/.test(r.url()) &&
        r.request().method() === 'POST',
      { timeout: 30000 },
    )
    await page.getByRole('button', { name: 'Next' }).click()
    expect((await stepComplete).status()).toBeLessThan(400)
    await expect(
      page.getByRole('heading', { name: 'AI Providers' }),
    ).toBeVisible()

    // (a) Server-side persistence: GET /api/onboarding/progress now reports a
    // completed step id (the Welcome step was durably stored, not just held
    // in the client store).
    const userToken = await page.evaluate(
      () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const progressRes = await fetch(`${apiURL}/api/onboarding/progress`, {
      headers: { Authorization: `Bearer ${userToken}` },
    })
    expect(progressRes.ok, `GET progress: ${progressRes.status}`).toBeTruthy()
    const progress = (await progressRes.json()) as {
      completed_step_ids: string[]
    }
    expect(
      progress.completed_step_ids.length,
      'a step completed via the wizard must be persisted server-side',
    ).toBeGreaterThan(0)

    // (b) Reload wipes all in-memory store state; the wizard must re-hydrate
    // from the persisted progress and RESUME at AI Providers (the next
    // incomplete step), never restarting at Welcome.
    await page.reload()
    await expect(
      page.getByRole('heading', { name: 'AI Providers' }),
    ).toBeVisible({ timeout: 30000 })
    await expect(
      page.getByRole('heading', { name: /Welcome/ }),
    ).toHaveCount(0)
  })
})
