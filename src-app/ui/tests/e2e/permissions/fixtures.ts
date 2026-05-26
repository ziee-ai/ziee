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
  // Always include profile::edit. Removing the user from the default
  // group (below) strips this away, and login()'s
  // completeOnboarding() POST requires it — without it the wizard
  // POST 403s and login() throws before the test ever runs. Adding
  // it back as a direct perm doesn't affect any test premise (the
  // tests gate hub/admin surfaces, not profile editing).
  if (!permissions.includes(Permissions.ProfileEdit)) {
    permissions = [...permissions, Permissions.ProfileEdit]
  }
  // Get an admin token. Try the direct API call first (works when the
  // spec's beforeEach already set the admin up); fall back to the
  // full setup UI flow if no admin exists yet.
  //
  // The earlier unconditional `loginAsAdmin` call was costly when an
  // admin already existed: it navigated to /auth and tried to
  // re-login through the UI form, which routinely timed out under
  // cold-cache Vite re-optimization. The API path is ~1 round trip.
  let adminToken: string
  try {
    adminToken = await getAdminToken(apiURL)
  } catch {
    await loginAsAdmin(page, baseURL)
    adminToken = await getAdminToken(apiURL)
  }

  let userId: string | undefined
  try {
    userId = await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@test.local`,
      PASSWORD,
      permissions,
    )
    console.log(`[loginWithPerms] created user ${username} → ${userId}`)
  } catch (e: any) {
    // If the user already exists from a prior test in this database
    // lifecycle, the login below will work regardless. Ignore.
    if (!/already exists/i.test(String(e?.message))) throw e
    console.log(`[loginWithPerms] user ${username} already exists`)
  }

  // Remove the new user from the default `users` group. The backend
  // auto-assigns every new user to the default group
  // (UserService::assign_to_default_group), and that group is seeded
  // with hub::assistants::create, hub::mcp_servers::create,
  // assistants::create, conversations::*, messages::*, etc. (see
  // migrations 27+). Permission checks union direct perms with
  // active-group perms, so a test user with explicit
  // `[HubAssistantsRead]` would still effectively hold
  // `hub::assistants::create` via the group — breaking any
  // "should prevent creation without required perms" test premise.
  // Removing here isolates the user to its direct perms only.
  if (userId) {
    try {
      const groupsRes = await page.request.get(`${apiURL}/api/groups`, {
        headers: { Authorization: `Bearer ${adminToken}` },
      })
      const groupsBody = await groupsRes.json()
      const groups: Array<{ id: string; is_default?: boolean; name: string }> =
        Array.isArray(groupsBody) ? groupsBody : (groupsBody.groups ?? [])
      console.log(`[loginWithPerms] groups: ${groups.map(g => `${g.name}(default=${g.is_default})`).join(', ')}`)
      const defaultGroup =
        groups.find(g => g.is_default) ?? groups.find(g => g.name === 'users')
      console.log(`[loginWithPerms] defaultGroup: ${defaultGroup?.id} (${defaultGroup?.name})`)
      if (defaultGroup) {
        // Backend endpoint shape: DELETE /api/groups/{user_id}/{group_id}/remove
        // (per UserGroup.removeUser in api-client/types.ts).
        const res = await page.request.delete(
          `${apiURL}/api/groups/${userId}/${defaultGroup.id}/remove`,
          { headers: { Authorization: `Bearer ${adminToken}` } },
        )
        if (!res.ok()) {
          console.warn(
            `[loginWithPerms] failed to remove ${username} from default group: ${res.status()} ${await res.text()}`,
          )
        }
      }
    } catch (e: any) {
      console.warn(`[loginWithPerms] removeFromDefaultGroup error: ${e?.message}`)
    }
  }

  // Reset to a clean session before logging in as the new user.
  await page.evaluate(() => {
    localStorage.clear()
    sessionStorage.clear()
  })
  await page.context().clearCookies()
  await login(page, baseURL, username, PASSWORD)

  // Debug: log the effective permissions the backend reports for this
  // user via /api/auth/me. Read the bearer token out of localStorage
  // (where the auth store persists it) — page.request doesn't share
  // the page's cookies/localStorage with the request context.
  try {
    const token = await page.evaluate(() => {
      const raw = localStorage.getItem('auth-storage')
      return raw ? JSON.parse(raw).state?.token : null
    })
    const meRes = await page.request.get(`${apiURL}/api/auth/me`, {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
    })
    const meBody = await meRes.json()
    console.log(
      `[loginWithPerms] ${username} → status=${meRes.status()} user.perms=${JSON.stringify(meBody.user?.permissions)} effective=${JSON.stringify(meBody.permissions)} groups=${JSON.stringify((meBody.groups ?? []).map((g: any) => g.name))}`,
    )
  } catch (e: any) {
    console.warn(`[loginWithPerms] /api/auth/me failed: ${e?.message}`)
  }
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
  // Hub MCP tab loads server cards which fetch per-server version
  // detail (hub::mcp_servers::read_version). Granting only ::read
  // makes the tab body render the inline "Missing required
  // permission" error instead of the catalog. The fixture name is
  // descriptive of intent ("can see the MCP tab"), so include both.
  return createAndLoginAs(page, baseURL, apiURL, 'hub-mcp-only-test', [
    Permissions.HubMCPServersRead,
    Permissions.HubMCPServersVersionRead,
  ])
}

/**
 * Read-only auth-providers user. Sees `/settings/auth-providers`
 * (list rendered) but no Add/Edit/Switch/Delete/Test controls.
 * Tests the gate-by-Can wrapping pattern in
 * `modules/auth-providers/components/AuthProvidersListSection.tsx`.
 */
export async function loginAsAuthProvidersReader(
  page: Page,
  baseURL: string,
  apiURL: string,
) {
  return createAndLoginAs(
    page,
    baseURL,
    apiURL,
    'auth-providers-reader-test',
    [Permissions.AuthProvidersRead],
  )
}

/**
 * Full-manage auth-providers user. All admin surfaces visible:
 * Add Provider, Edit drawer, Test config, Switch, Delete, etc.
 */
export async function loginAsAuthProvidersManager(
  page: Page,
  baseURL: string,
  apiURL: string,
) {
  return createAndLoginAs(
    page,
    baseURL,
    apiURL,
    'auth-providers-manager-test',
    [Permissions.AuthProvidersRead, Permissions.AuthProvidersManage],
  )
}

/**
 * Generic helper: create + log in as a user with an arbitrary explicit
 * permission set. Useful for one-off tests where the named fixtures
 * above don't fit (e.g. hub permission-prevention tests that need
 * read-but-not-create).
 */
export async function loginWithPerms(
  page: Page,
  baseURL: string,
  apiURL: string,
  permissions: string[],
  usernameSuffix?: string,
) {
  const suffix = usernameSuffix ?? Math.random().toString(36).slice(2, 8)
  return createAndLoginAs(
    page,
    baseURL,
    apiURL,
    `perm-${suffix}`,
    permissions,
  )
}
