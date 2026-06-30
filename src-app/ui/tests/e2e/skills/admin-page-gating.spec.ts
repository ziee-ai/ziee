import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/types'
import { byTestId } from '../testid.ts'
import {
  assertAdminSkillsEmptyState,
  goToAdminSkillsPage,
} from './helpers/skill-helpers'

test.describe('Skills - Admin page gating', () => {
  test('admin can view the System Skills page', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToAdminSkillsPage(page, baseURL)

    await expect(byTestId(page, 'skills-admin-page')).toBeVisible()

    // Fresh DB → no system skills installed.
    await assertAdminSkillsEmptyState(page)

    // Exclude the pre-existing shell-wide empty-antd-Menu sidebar violation
    // (aria-required-children) — confirmed identical on the existing
    // projects a11y test; not from this feature. See list-page-renders.
    await assertNoAccessibilityViolations(page, {
      disabledRules: ['aria-required-children'],
    })
  })

  test('non-admin without skills::manage_system is blocked', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    // A user holding only skills::read (the user-list perm) lacks
    // skills::manage_system, which gates the /settings/skills-admin
    // route. The router's RoutePermissionGate renders an inline antd
    // <Result status="403" title="Not authorized"> instead of the page
    // (URL preserved). Mirrors tests/e2e/permissions/users.spec.ts.
    await loginWithPerms(page, baseURL, apiURL, [Permissions.SkillsRead])

    await page.goto(`${baseURL}/settings/skills-admin`)

    // `/settings/*` renders inside SettingsPage, so a forbidden section shows
    // the SETTINGS-level Result, not the router-level gate.
    await expect(byTestId(page, 'settings-forbidden-result')).toBeVisible()
    await expect(byTestId(page, 'skills-admin-page')).toHaveCount(0)
  })
})
