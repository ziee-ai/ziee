import { test, expect } from '../../fixtures/test-context'
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
    page.getByRole('heading', { name: /^Settings$/ }),
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
      const usersMenu = pageB
        .getByRole('menuitem')
        .filter({ hasText: /^Users$/ })
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
      const usersMenu = pageB
        .getByRole('menuitem')
        .filter({ hasText: /^Users$/ })
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
