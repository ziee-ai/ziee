import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
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

/**
 * The permission patterns are read from the SHIPPED manifest, not hard-coded —
 * so if someone edits `config/desired-state.yaml`'s `remove:` list, this spec
 * follows it (and a typo there fails here rather than silently un-hiding a
 * feature in production).
 */
function shippedRemovePatterns(): string[] {
  const manifest = fileURLToPath(
    new URL('../../../../../config/desired-state.yaml', import.meta.url),
  )
  const yaml = readFileSync(manifest, 'utf8')
  // The `remove:` list under the `Users` group entry: a flat list of
  // `- <pattern>` lines. No YAML parser is available in the e2e deps.
  const removeBlock = yaml.split(/^\s*remove:\s*$/m)[1] ?? ''
  const patterns = [...removeBlock.matchAll(/^\s*-\s*([^\s#]+)\s*$/gm)].map(
    m => m[1],
  )
  expect(
    patterns.length,
    'could not read the remove: list out of config/desired-state.yaml',
  ).toBeGreaterThan(0)
  return patterns
}

/** `hub::*` → the `hub` prefix; an exact permission → itself. */
function toPrefix(pattern: string): string {
  return pattern.endsWith('::*') ? pattern.slice(0, -3) : pattern
}

test.describe('desired-state — restricted (default-group) user', () => {
  test('[negative-perm] hidden features are absent for a user in the trimmed default group', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra

    // The admin must exist before we can create a user through the API.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Apply the trim the SHIPPED manifest declares, with the same matching
    // semantics the reconciler uses (`hub::*` strips `hub` and anything under
    // `hub::`). The backend integration suite (tests/desired_state/) proves the
    // reconciler WRITES this at boot; this spec is about what the UI then does.
    const patterns = shippedRemovePatterns()
    const prefixes = patterns.map(toPrefix)
    await sql(
      `UPDATE groups
          SET permissions = ARRAY(
                SELECT p FROM unnest(permissions) AS p
                 WHERE NOT (p = ANY($1::text[]))
                   AND NOT EXISTS (
                         SELECT 1 FROM unnest($1::text[]) AS pre
                          WHERE p LIKE pre || '::%'
                       )
              )
        WHERE name = 'Users' AND is_system = true AND is_default = true`,
      [prefixes],
    )

    // Sanity-check the fixture actually removed something, so a future rename of
    // the permissions can't make this spec vacuously green.
    const after = await sql(
      `SELECT permissions FROM groups WHERE name = 'Users' AND is_default = true`,
    )
    const perms: string[] = after.rows[0].permissions
    for (const prefix of prefixes) {
      expect(
        perms.some(p => p === prefix || p.startsWith(`${prefix}::`)),
        `${prefix} should have been stripped from the Users group`,
      ).toBe(false)
    }
    // The trim must be non-vacuous: hub:: + assistants:: ARE granted to the
    // default group by migration 27, so at least those must have been present.
    // (`projects::*` is granted only to Administrators, so its removal is a
    // documented no-op — see DEC-6.)
    expect(prefixes).toContain('hub')
    expect(prefixes).toContain('assistants')
    // …and the KEEP set survived.
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
