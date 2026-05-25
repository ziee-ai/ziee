import type { Page } from '@playwright/test'
import {
  createTestUser,
  getAdminToken,
  login,
  loginAsAdmin,
} from '../../common/auth-helpers'
import { Permissions } from '../../../src/api-client/types'

/**
 * Permission-scoped test-user helpers.
 *
 * The seeded admin (DEFAULT_ADMIN_CREDENTIALS) has `is_admin = true`
 * and bypasses every check. Non-admin fixtures are created on demand
 * via the admin API with explicit permission lists (no group
 * membership — the backend evaluates the union of user perms + active
 * group perms, so direct perms are sufficient for these tests).
 *
 * Each helper:
 *   1. Logs in as admin to get an API token.
 *   2. Creates the user via `POST /api/users` with the requested perms.
 *   3. Clears auth state on the page.
 *   4. Logs in as the new user (token injection + reload).
 */

const PASSWORD = 'password123'

async function createAndLoginAs(
  page: Page,
  baseURL: string,
  apiURL: string,
  username: string,
  permissions: string[],
) {
  await loginAsAdmin(page, baseURL)
  const adminToken = await getAdminToken(apiURL)
  try {
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@test.local`,
      PASSWORD,
      permissions,
    )
  } catch (e: any) {
    // If the user already exists from a prior test in this database
    // lifecycle, the login below will work regardless. Ignore.
    if (!/already exists/i.test(String(e?.message))) throw e
  }
  // Reset to a clean session before logging in as the new user.
  await page.evaluate(() => {
    localStorage.clear()
    sessionStorage.clear()
  })
  await page.context().clearCookies()
  await login(page, baseURL, username, PASSWORD)
}

/**
 * Member user with no special permissions (default `users` group only).
 * Should be denied access to every admin surface.
 */
export async function loginAsMember(
  page: Page,
  baseURL: string,
  apiURL: string,
) {
  return createAndLoginAs(page, baseURL, apiURL, 'member-test', [])
}

/**
 * Read-only users-module user. Verifies the read-vs-manage form
 * disable path: the user list is visible, but Create/Edit/Delete
 * controls aren't.
 */
export async function loginAsUsersReadOnly(
  page: Page,
  baseURL: string,
  apiURL: string,
) {
  return createAndLoginAs(page, baseURL, apiURL, 'users-readonly-test', [
    Permissions.UsersRead,
    Permissions.GroupsRead,
  ])
}

/**
 * Hub-MCP-only user. Verifies Hub's per-tab visibility — should see
 * the MCP Servers tab but not Models or Assistants.
 */
export async function loginAsHubMcpOnly(
  page: Page,
  baseURL: string,
  apiURL: string,
) {
  return createAndLoginAs(page, baseURL, apiURL, 'hub-mcp-only-test', [
    Permissions.HubMCPServersRead,
  ])
}
