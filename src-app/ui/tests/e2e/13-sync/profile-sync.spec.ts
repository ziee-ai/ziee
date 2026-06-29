import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  createTestUser,
  getAdminToken,
  loginAsAdmin,
  login,
} from '../../common/auth-helpers'

// Realtime sync for the `profile` entity (Owner-scoped). When an admin
// edits a user's profile fields (e.g. display_name), the backend emits
// `Profile/Update` to that user's own connections so their other open
// devices re-bootstrap /auth/me and reflect the new values WITHOUT a
// manual reload.
//
// The integration test
// `update_user_emits_profile_to_the_edited_user_only` proves the
// frame fires on the right audience; this spec closes the browser
// side of the loop: device A (admin) mutates via REST, device B (the
// edited user, on /settings/profile) shows the new display_name in
// the form input on its own.
//
// Run with --workers=1.
//
// CRITICAL: this suite never calls waitForLoadState('networkidle') —
// the sync SSE stream is a persistent connection that keeps the
// network busy and would hang the wait.

async function gotoProfile(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/profile`)
  await page.waitForLoadState('load')
  await expect(
    byTestId(page, 'profile-account-descriptions'),
  ).toBeVisible({ timeout: 15_000 })
}

test.describe('Realtime sync — profile (owner-scoped)', () => {
  test("an admin's edit of a user's display_name reflects on the user's other device without reload", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const timestamp = Date.now()
    const username = `syncprofile${timestamp}`
    const password = 'password123'
    const targetId = await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@example.com`,
      password,
      [],
    )

    // The target user logs in on device B and lands on their profile
    // page. The display_name form input is the stable, observable
    // signal — it's re-seeded by an effect whenever `user` updates,
    // which is exactly what `Auth.store.refreshCurrentUser()` does in
    // response to `sync:profile`.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await login(pageB, baseURL, username, password, {
        completeOnboarding: true,
      })
      await gotoProfile(pageB, baseURL)

      // The form's "Display Name" Form.Item label is "Display name"
      // (per ProfileSettingsPage); getByLabel matches the antd label
      // text and resolves to the underlying <input>.
      const displayInput = byTestId(pageB, 'profile-display-name-input')
      await expect(displayInput).toBeVisible({ timeout: 15_000 })
      // Sanity: createTestUser did not pass display_name, so the
      // backend stores null and the form seeds with empty string.
      // We don't assert on the initial value — only on the change.

      const newDisplayName = `Renamed Live ${timestamp}`
      const res = await page.request.post(
        `${baseURL}/api/users/${targetId}`,
        {
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          data: { display_name: newDisplayName },
        },
      )
      expect(res.ok()).toBeTruthy()

      // The form must reflect the new display_name within the SSE
      // delivery + Auth refetch + form-seed window. Only true if:
      //   1. Profile/Update was published to the target's Owner audience
      //   2. SSE delivered the frame to device B
      //   3. Auth.store re-bootstrapped /auth/me
      //   4. The form's useEffect re-seeded the input from the new
      //      user.display_name
      await expect(displayInput).toHaveValue(newDisplayName, {
        timeout: 20_000,
      })
    } finally {
      await ctxB.close()
    }
  })

  test('a profile edit is NOT delivered to a DIFFERENT user (owner isolation)', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const timestamp = Date.now()
    const targetUsername = `syncptarget${timestamp}`
    const targetId = await createTestUser(
      apiURL,
      adminToken,
      targetUsername,
      `${targetUsername}@example.com`,
      'password123',
      [],
    )

    const bystanderUsername = `syncpbystander${timestamp}`
    await createTestUser(
      apiURL,
      adminToken,
      bystanderUsername,
      `${bystanderUsername}@example.com`,
      'password123',
      [],
    )

    // The BYSTANDER opens their profile page on device B.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await login(pageB, baseURL, bystanderUsername, 'password123', {
        completeOnboarding: true,
      })
      await gotoProfile(pageB, baseURL)

      const displayInput = byTestId(pageB, 'profile-display-name-input')
      await expect(displayInput).toBeVisible({ timeout: 15_000 })
      const bystanderInitialVal = await displayInput.inputValue()

      // Admin edits the OTHER user (not the bystander) — this is the
      // Owner-scoped Profile/Update path; the bystander's stream
      // must NOT receive it (a leak here would mean Owner is broken).
      const res = await page.request.post(
        `${baseURL}/api/users/${targetId}`,
        {
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          data: { display_name: `Should Not Reach Bystander ${timestamp}` },
        },
      )
      expect(res.ok()).toBeTruthy()

      // The bystander's display name field must NOT change. Wait long
      // enough that any in-flight stray frame would have arrived; if
      // the value changes within the window, Owner-scoping is broken.
      // toHaveValue with `inputValue` after a sleep is a more robust
      // anti-assertion than relying solely on Playwright's auto-retry.
      await pageB.waitForTimeout(3_000)
      await expect(displayInput).toHaveValue(bystanderInitialVal)
    } finally {
      await ctxB.close()
    }
  })

  test('two devices of the SAME user converge on bidirectional profile edits', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const ts = Date.now()
    const username = `concur${ts}`
    const password = 'password123'
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@example.com`,
      password,
      ['profile::read', 'profile::edit'],
    )

    // The same user opens TWO devices, both on the profile page.
    const ctxA = await browser.newContext()
    const ctxB = await browser.newContext()
    const a = await ctxA.newPage()
    const b = await ctxB.newPage()
    try {
      await login(a, baseURL, username, password, { completeOnboarding: true })
      await gotoProfile(a, baseURL)
      await login(b, baseURL, username, password, { completeOnboarding: true })
      await gotoProfile(b, baseURL)

      const inputA = byTestId(a, 'profile-display-name-input')
      const inputB = byTestId(b, 'profile-display-name-input')

      // Device A edits + saves → device B reflects it via sync (no reload).
      const nameFromA = `From A ${ts}`
      await inputA.fill(nameFromA)
      const _saved_a = a.waitForResponse(r => /\/api\/auth\/profile$/.test(r.url()) && r.request().method() === 'POST')
      await byTestId(a, 'profile-save-button').click()
      await _saved_a
      await expect(inputB).toHaveValue(nameFromA, { timeout: 20_000 })

      // Now device B edits + saves → device A converges on B's value.
      const nameFromB = `From B ${ts}`
      await inputB.fill(nameFromB)
      const _saved_b = b.waitForResponse(r => /\/api\/auth\/profile$/.test(r.url()) && r.request().method() === 'POST')
      await byTestId(b, 'profile-save-button').click()
      await _saved_b
      await expect(inputA).toHaveValue(nameFromB, { timeout: 20_000 })
    } finally {
      await ctxA.close()
      await ctxB.close()
    }
  })
})
