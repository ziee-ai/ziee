import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
import { byTestId } from '../testid'

/**
 * A10 [negative-perm] — the Background sub-agent surfaces are gated, and a user
 * who lacks the grants sees NONE of them (with an admin positive control that
 * proves the surfaces EXIST + are permission-gated, so the absence below is a
 * real gate and not a "module not loaded" vacuous pass).
 *
 * Two distinct surfaces + perms are involved (both correct — verified against
 * `background/module.tsx` + `notification/module.tsx`):
 *   • "Background tasks"  → /background-tasks         → gated `background::use`
 *   • "Background results"→ /notifications/background → gated `notifications::read`
 *     (the agent-inbox is a filtered VIEW over notifications, so it rides the
 *      notifications read-perm, not `background::use`).
 *
 * `background::use` is granted to the default `users` group, so a normal user
 * HAS it — this spec therefore uses `loginWithPerms(..., [])`, which creates a
 * user and REMOVES it from the default group (see permissions/fixtures.ts), so
 * the subject holds ONLY profile perms and lacks BOTH `background::use` and
 * `notifications::read`. Absence is then asserted at every gating layer:
 *
 *   1. slot   — neither the "Background tasks" nor the "Background results" left-
 *               sidebar nav entry renders (permission-filtered out of the menu).
 *   2. route  — a direct hit on /background-tasks AND /notifications/background
 *               each renders a 403 gate, never the page itself.
 */
test.describe('Background sub-agent surface — permission gating (negative-perm)', () => {
  // Left-sidebar nav items derive `<menu-testid>-item-<id>` from the kit Menu
  // (`layout-sidebar-nav-menu`); the background module registers id
  // `background-tasks`, the notification module id `agent-inbox`.
  const BACKGROUND_TASKS_NAV = 'layout-sidebar-nav-menu-item-background-tasks'
  const BACKGROUND_RESULTS_NAV = 'layout-sidebar-nav-menu-item-agent-inbox'

  const appShell = (page: import('@playwright/test').Page) =>
    page
      .getByRole('button', { name: /New Chat/ })
      .or(byTestId(page, 'layout-sidebar-toggle-button'))
      .first()

  test('admin sees both Background nav entries; a user without the grants sees no Background surface', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // ── Positive control: an admin (holds `*`) sees BOTH nav entries. ───────
    await loginAsAdmin(page, baseURL)
    await expect(appShell(page)).toBeVisible({ timeout: 45000 })
    await expect(byTestId(page, BACKGROUND_TASKS_NAV)).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, BACKGROUND_RESULTS_NAV)).toBeVisible({
      timeout: 30000,
    })

    // ── Negative subject: no group perms, only profile::{read,edit}. Lacks ──
    // background::use AND notifications::read. loginWithPerms creates the user,
    // strips the default group, clears state, and lands on the app shell (chat).
    await loginWithPerms(page, baseURL, apiURL, [], 'bg-noperm')
    await expect(appShell(page)).toBeVisible({ timeout: 45000 })

    // Layer 1 (slot): neither background nav entry renders.
    await expect(byTestId(page, BACKGROUND_TASKS_NAV)).toHaveCount(0)
    await expect(byTestId(page, BACKGROUND_RESULTS_NAV)).toHaveCount(0)

    // Layer 2 (route): /background-tasks is 403-gated (background::use).
    await page.goto(`${baseURL}/background-tasks`)
    await expect(
      byTestId(page, 'router-route-forbidden-result'),
    ).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'background-tasks-page')).toHaveCount(0)

    // Layer 2 (route): /notifications/background is 403-gated
    // (notifications::read).
    await page.goto(`${baseURL}/notifications/background`)
    await expect(
      byTestId(page, 'router-route-forbidden-result'),
    ).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'agent-inbox-page')).toHaveCount(0)
  })
})
