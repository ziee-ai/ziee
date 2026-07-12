import { test, expect } from './no-403'
import {
  createTestUser,
  getAdminToken,
  login,
  loginAsAdmin,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * TEST-14 — the user-visible EFFECT of the config-as-code group trim.
 *
 * The shipped `config/desired-state.yaml` removes `projects::*`, `hub::*` and
 * `assistants::*` from the default **Users** group. This spec proves what that
 * actually does to a real, non-admin user who is IN that group: the Projects and
 * Hub nav entries and the Settings→Assistants section must be GONE — nav entry,
 * settings tab AND route (they share one permission gate) — while General,
 * Profile, LLM Providers and MCP Servers stay.
 *
 * The seeded admin bypasses every permission check (`users.is_admin`), so the
 * hiding is only ever observable through a regular user — which is exactly why
 * the desired-state file seeds one.
 *
 * The trim itself is applied here with the same set semantics the reconciler
 * uses (the reconciler's own DB write is covered by the backend integration
 * suite, `tests/desired_state/`); this spec is about the UI consequence.
 */

const PASSWORD = 'password123'

const REMOVED_PREFIXES = ['projects::', 'hub::', 'assistants::']

test.describe('desired-state — restricted (default-group) user', () => {
  test('[negative-perm] hidden features are absent for a user in the trimmed default group', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra

    // The admin must exist before we can create a user through the API.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Apply the desired-state trim to the default group (what the reconciler
    // writes at boot).
    await sql(
      `UPDATE groups
          SET permissions = ARRAY(
                SELECT p FROM unnest(permissions) AS p
                 WHERE p NOT LIKE 'projects::%'
                   AND p NOT LIKE 'hub::%'
                   AND p NOT LIKE 'assistants::%'
              )
        WHERE name = 'Users' AND is_system = true AND is_default = true`,
    )

    // Sanity-check the fixture actually removed something, so a future rename of
    // the permissions can't make this spec vacuously green.
    const after = await sql(
      `SELECT permissions FROM groups WHERE name = 'Users' AND is_default = true`,
    )
    const perms: string[] = after.rows[0].permissions
    for (const prefix of REMOVED_PREFIXES) {
      expect(perms.some(p => p.startsWith(prefix))).toBe(false)
    }
    // …and that the KEEP set survived.
    expect(perms).toContain('mcp_servers::read')
    expect(perms).toContain('user_llm_providers::read')

    // A normal user — created through the real API, so it lands in the default
    // group exactly like the desired-state-seeded `user` account does.
    await createTestUser(
      apiURL,
      adminToken,
      'restricted_user',
      'restricted_user@test.local',
      PASSWORD,
      [],
    )

    await page.goto(baseURL)
    await page.evaluate(() => {
      localStorage.clear()
      sessionStorage.clear()
    })
    await page.context().clearCookies()
    await login(page, baseURL, 'restricted_user', PASSWORD)

    // ── Layer 1: the nav slots are gone ──
    await page.goto(`${baseURL}/`)
    await expect(
      byTestId(page, 'layout-sidebar-nav-menu-item-projects'),
    ).toHaveCount(0)
    await expect(
      byTestId(page, 'layout-sidebar-tools-menu-item-hub'),
    ).toHaveCount(0)

    // ── Layer 2: the routes refuse to render the feature ──
    await page.goto(`${baseURL}/projects`)
    await expect(byTestId(page, 'router-route-forbidden-result')).toBeVisible()
    expect(page.url()).toContain('/projects')

    await page.goto(`${baseURL}/hub`)
    await expect(byTestId(page, 'router-route-forbidden-result')).toBeVisible()
    expect(page.url()).toContain('/hub')

    // ── Layer 3: the Settings→Assistants section is gone (tab AND route) ──
    await page.goto(`${baseURL}/settings/general`)
    await expect(byTestId(page, 'settings-nav-menu')).toBeVisible()
    await expect(
      byTestId(page, 'settings-nav-menu-item-assistants'),
    ).toHaveCount(0)

    await page.goto(`${baseURL}/settings/assistants`)
    // A deep link renders the settings-level forbidden panel, not the feature.
    await expect(byTestId(page, 'settings-forbidden-result')).toBeVisible()

    // ── The KEEP set is still there ──
    await page.goto(`${baseURL}/settings/general`)
    for (const kept of ['general', 'profile', 'user-llm-providers', 'mcp-servers']) {
      await expect(
        byTestId(page, `settings-nav-menu-item-${kept}`),
      ).toBeVisible()
    }
  })
})
