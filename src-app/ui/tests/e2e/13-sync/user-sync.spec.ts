import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  navigateToUsers,
  navigateToUserGroups,
  openCreateUserDrawer,
  openCreateGroupDrawer,
} from '../02-users/helpers/user-navigation'
import { createUser } from '../02-users/helpers/user-actions'
import { createGroup } from '../02-users/helpers/group-actions'
import {
  assertUserExists,
  assertGroupExists,
} from '../02-users/helpers/user-assertions'

// Realtime sync for the admin USER and GROUP tables (permission-audience
// scoped). A row created by one admin device reaches another admin device
// live — both devices carry `users::read` / `groups::read` (the create
// handlers publish `Audience::perm::<UsersRead>()` / `<GroupsRead>()`).
//
// Run with --workers=1 (shared backend + DB).
//
// NOTE on `networkidle`: the realtime-sync SSE stream is a persistent
// connection that keeps the network perpetually "busy", so any helper that
// calls `waitForLoadState('networkidle')` HANGS. The navigation helpers used
// here (navigateToUsers / navigateToUserGroups) wait for `load` + a stable
// heading selector, so they are safe. The assertion helpers used here
// (assertUserExists / assertGroupExists) are pure reads with NO reload —
// they observe the SSE-driven store refetch directly.
test.describe('Realtime sync — admin user/group tables (perm-scoped)', () => {
  test('a user created on admin device A appears in admin device B users table without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Fresh per-test backend: onboard the admin FIRST, then land on the
    // users page on device A.
    await loginAsAdmin(page, baseURL)
    await navigateToUsers(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      // Load device B fully (its own admin session + users table) BEFORE
      // device A mutates, so the SSE stream is already subscribed.
      await loginAsAdmin(pageB, baseURL)
      await navigateToUsers(pageB, baseURL)

      const timestamp = Date.now()
      const userData = {
        username: `syncuser${timestamp}`,
        email: `syncuser${timestamp}@example.com`,
        password: 'password123',
        displayName: 'Sync User',
      }

      // Create the user on device A through the real admin UI drawer.
      await openCreateUserDrawer(page)
      await createUser(page, userData)

      // Sanity: device A (the originator) shows the new row.
      await assertUserExists(page, userData.username)

      // The point of the test: device B must show the new user WITHOUT a
      // manual reload — the SSE sync event makes the users store refetch.
      // assertUserExists has its own 5s wait; Playwright auto-retries the
      // expect, but bump the timeout to absorb SSE delivery latency.
      const userElementB = pageB.locator('.ant-typography.font-medium', {
        hasText: userData.username,
      })
      await expect(userElementB.first()).toBeVisible({ timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })

  test('a group created on admin device A appears in admin device B groups table without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await navigateToUserGroups(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await navigateToUserGroups(pageB, baseURL)

      const timestamp = Date.now()
      const groupData = {
        name: `SyncGroup${timestamp}`,
        description: 'Realtime-sync test group',
      }

      // Create the group on device A through the real admin UI drawer.
      await openCreateGroupDrawer(page)
      await createGroup(page, groupData)

      // Sanity: device A (the originator) shows the new row.
      await assertGroupExists(page, groupData.name)

      // The point of the test: device B must show the new group WITHOUT a
      // manual reload — the SSE sync event makes the groups store refetch.
      const groupElementB = pageB.locator('text=' + groupData.name)
      await expect(groupElementB.first()).toBeVisible({ timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })
})
