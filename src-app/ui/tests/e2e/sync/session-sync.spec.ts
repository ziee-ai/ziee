import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  createTestUser,
  getAdminToken,
  loginAsAdmin,
  login,
} from '../../common/auth-helpers'

// Realtime sync for the `session` entity — the most security-critical
// surface in the sync vocabulary. When an admin edits a group's
// permissions (or assigns/removes a user from a group), the backend
// publishes `Session/Update` to every affected member via
// `publish_session_to_users`. Each member's Auth.store re-bootstraps
// /auth/me, refreshing their `permissions` snapshot WITHOUT a manual
// reload — so a granted capability immediately becomes available
// (and a revoked one immediately disappears).
//
// The integration test
// `group_permission_edit_fans_session_out_to_every_member` proves the
// fan-out emits to the right audience over the real HTTP path. This
// spec closes the browser side of the loop: a NEW permission granted
// on device A surfaces in the member's UI on device B as a sidebar
// menu entry appearing on its own, with NO `page.reload()` involved.
//
// Run with --workers=1 (shared backend + DB).
//
// CRITICAL: this suite NEVER calls waitForLoadState('networkidle').
// The realtime-sync SSE stream is a persistent connection — networkidle
// would never settle. We rely on stable selector waits instead.

async function gotoSettings(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings`)
  await page.waitForLoadState('load')
  // The Settings heading is always present once the page mounts — every
  // user (admin or not) sees at least the "General" entry.
  await expect(
    byTestId(page, 'settings-nav-menu'),
  ).toBeVisible({ timeout: 15_000 })
}

test.describe('Realtime sync — session (group-permission fan-out)', () => {
  test('granting users::read via group assignment surfaces the Users menu on the member device without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // ── Setup: admin + member ────────────────────────────────────────
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const timestamp = Date.now()
    const memberUsername = `syncmember${timestamp}`
    const memberPassword = 'password123'
    const memberId = await createTestUser(
      apiURL,
      adminToken,
      memberUsername,
      `${memberUsername}@example.com`,
      memberPassword,
      [], // no direct permissions — only the default group's baseline
    )

    // ── Device B: log in as the member, land on /settings ────────────
    // Open BEFORE the admin's grant so the SSE stream is subscribed
    // when the Session frame fires.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await login(pageB, baseURL, memberUsername, memberPassword, {
        completeOnboarding: true,
      })
      await gotoSettings(pageB, baseURL)

      // The "Users" menu entry is `<Can permission={Permissions.UsersRead}>`-
      // wrapped via the slot's permission field (modules/user/module.tsx).
      // The member has no users::read yet → entry must be absent.
      // The menuitem's accessible name includes the icon's alt ("user Users"
      // from `<img alt="user">` + "Users" text), so an exact `name: /^Users$/`
      // regex against the accessible name MISSES this entry. Filter by inner
      // text instead — that's "Users" alone, distinct from "User Groups" /
      // "MCP Servers" which share the "User" or "Servers" substring.
      const usersMenu = byTestId(pageB, 'settings-nav-menu-item-users')
      await expect(usersMenu).toHaveCount(0)

      // ── Device A: admin creates a group with users::read, then
      //    assigns the member to it ────────────────────────────────
      // Both calls are pure REST through the admin's session token —
      // the backend emits `group/create`, `group/update`, AND the
      // Session fan-out on assign. Only the Session frame matters
      // here; the others land on permission-scoped audiences the
      // member does NOT belong to.
      const groupName = `SyncSessionGroup${timestamp}`
      const groupRes = await page.request.post(
        `${baseURL}/api/groups`,
        {
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          data: {
            name: groupName,
            description: 'realtime-sync session test group',
            permissions: ['users::read'],
          },
        },
      )
      expect(groupRes.ok()).toBeTruthy()
      const group = await groupRes.json()
      const groupId = group.id as string

      const assignRes = await page.request.post(
        `${baseURL}/api/groups/assign`,
        {
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${adminToken}`,
          },
          data: { user_id: memberId, group_id: groupId },
        },
      )
      expect([200, 204]).toContain(assignRes.status())

      // ── The assertion: the "Users" menu entry appears on device B
      //    WITHOUT a manual reload. This is only true if:
      //      1. Backend published `Session/Update` to the member.
      //      2. Member's SSE stream delivered the frame.
      //      3. Auth.store re-bootstrapped /auth/me.
      //      4. permissions snapshot now includes 'users::read'.
      //      5. The slot/menu re-rendered with the updated <Can> gate.
      //    Any broken link in the chain leaves the entry absent and
      //    fails the test. Generous timeout to absorb SSE + refetch.
      await expect(usersMenu).toBeVisible({ timeout: 20_000 })
    } finally {
      await ctxB.close()
    }
  })

  test('revoking permission by removing member from group hides the Users menu on the member device without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // ── Setup: admin + member + a users::read group with member in it
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const timestamp = Date.now()
    const memberUsername = `syncrevoke${timestamp}`
    const memberPassword = 'password123'
    const memberId = await createTestUser(
      apiURL,
      adminToken,
      memberUsername,
      `${memberUsername}@example.com`,
      memberPassword,
      [],
    )

    const groupName = `SyncRevokeGroup${timestamp}`
    const groupRes = await page.request.post(
      `${baseURL}/api/groups`,
      {
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${adminToken}`,
        },
        data: {
          name: groupName,
          description: 'realtime-sync revoke test group',
          permissions: ['users::read'],
        },
      },
    )
    expect(groupRes.ok()).toBeTruthy()
    const groupId = (await groupRes.json()).id as string

    const assignRes = await page.request.post(
      `${baseURL}/api/groups/assign`,
      {
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${adminToken}`,
        },
        data: { user_id: memberId, group_id: groupId },
      },
    )
    expect([200, 204]).toContain(assignRes.status())

    // ── Device B: log in as the (now group-member) and confirm the
    //    Users menu is visible BEFORE we revoke it.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await login(pageB, baseURL, memberUsername, memberPassword, {
        completeOnboarding: true,
      })
      await gotoSettings(pageB, baseURL)

      // The menuitem's accessible name includes the icon's alt ("user Users"
      // from `<img alt="user">` + "Users" text), so an exact `name: /^Users$/`
      // regex against the accessible name MISSES this entry. Filter by inner
      // text instead — that's "Users" alone, distinct from "User Groups" /
      // "MCP Servers" which share the "User" or "Servers" substring.
      const usersMenu = byTestId(pageB, 'settings-nav-menu-item-users')
      await expect(usersMenu).toBeVisible({ timeout: 15_000 })

      // ── Admin removes the member from the group ─────────────────
      // The remove handler emits Session/Update to the affected
      // user (Owner-scoped, same as assign). Member's Auth.store
      // re-bootstraps /auth/me and the now-empty users::read makes
      // the slot's <Can> gate hide the entry.
      const removeRes = await page.request.delete(
        `${baseURL}/api/groups/${memberId}/${groupId}/remove`,
        {
          headers: { Authorization: `Bearer ${adminToken}` },
        },
      )
      expect([200, 204]).toContain(removeRes.status())

      // The menu entry must DISAPPEAR within the SSE delivery
      // window. `toHaveCount(0)` polls until satisfied.
      await expect(usersMenu).toHaveCount(0, { timeout: 20_000 })
    } finally {
      await ctxB.close()
    }
  })
})

// ── Cross-tab logout (the reported bug's second symptom) ────────────────────
//
// Reported: with an admin open in two tabs, logging out in tab 1 left tab 2
// "still logged in as admin ... as if nothing happened". Verified live before
// the fix: tab 2 kept rendering the whole authenticated admin shell
// indefinitely, because nothing told it anything had changed.
//
// The chain this closes: logout bumps `users.token_version` + publishes
// `Session/Update` → tab 2's Auth.store re-bootstraps /auth/me → 401
// (SESSION_REVOKED, its access token is now revoked) → the api-client
// interceptor refreshes → that 401s too (refresh tokens revoked) → the store
// tears the session down. No reload driven by this test.
test.describe('Realtime sync — session (cross-tab logout)', () => {
  test('logging out in one tab tears down the other tab on its own', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // TAB 2 — the SAME browser context, so it shares localStorage + cookies.
    // That is what "two browser tabs" means; browser.newContext() would model a
    // separate DEVICE and miss the shared-storage dimension entirely.
    const tab2 = await page.context().newPage()
    try {
      await tab2.goto(`${baseURL}/`)
      await tab2.waitForLoadState('load')
      // Precondition: tab 2 is authenticated (the sidebar profile widget only
      // renders for an authenticated user).
      await expect(byTestId(tab2, 'user-profile-widget')).toBeVisible({
        timeout: 30_000,
      })

      // TAB 1 — log out through the real dropdown click-path.
      await byTestId(page, 'user-profile-widget').click()
      await byTestId(page, 'userprofile-menu-dropdown-item-logout').click()
      await expect(byTestId(page, 'auth-login-username')).toBeVisible({
        timeout: 15_000,
      })

      // TAB 2 — must tear itself down on its own, with no reload driven by the
      // TEST (the app's own teardown does reload the document). Before the fix
      // it sat on the admin shell indefinitely.
      await expect(byTestId(tab2, 'auth-login-username')).toBeVisible({
        timeout: 30_000,
      })
      await expect(byTestId(tab2, 'user-profile-widget')).toHaveCount(0)
    } finally {
      await tab2.close()
    }
  })
})
