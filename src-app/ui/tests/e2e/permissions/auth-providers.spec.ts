/**
 * Permission-gating E2E for /settings/auth-providers.
 *
 * Four personas:
 *   1. Member (no auth_providers perms) — menu entry hidden + deep-link 403
 *   2. Reader (auth_providers::read only) — page + list visible,
 *      NO Add / Edit / Switch / Delete / Test controls
 *   3. Manager (auth_providers::read + ::manage) — all controls present
 *   4. Admin (root, is_admin) — all controls present via wildcard
 *
 * Uses the `no-403` fixture so any accidental /api/* 403 during the
 * Reader test fails loudly (catches missing UI gates the developer
 * forgot to wire — exactly the regression class
 * `.claude/PERMISSION_GATING.md` was designed to prevent).
 */
import { test, expect } from './no-403'
import {
  loginAsAuthProvidersManager,
  loginAsAuthProvidersReader,
  loginAsMember,
} from './fixtures'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

test.describe('auth-providers — permission gating', () => {
  test('member: settings menu entry hidden + deep-link 403', async ({
    page,
    testInfra,
  }) => {
    await loginAsMember(page, testInfra.baseURL, testInfra.apiURL)

    // Menu entry hidden.
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(
      byTestId(page, 'settings-nav-menu-item-auth-providers'),
    ).toHaveCount(0)

    // Deep-link → inline 403, URL preserved.
    await page.goto(`${testInfra.baseURL}/settings/auth-providers`)
    await expect(byTestId(page, 'settings-forbidden-result')).toBeVisible()
    expect(page.url()).toContain('/settings/auth-providers')
  })

  test('reader: list visible, NO Add/Edit/Switch/Delete/Test', async ({
    page,
    testInfra,
  }) => {
    await loginAsAuthProvidersReader(
      page,
      testInfra.baseURL,
      testInfra.apiURL,
    )

    await page.goto(`${testInfra.baseURL}/settings/auth-providers`)

    // The page + list card render — readers see the data.
    await expect(byTestId(page, 'authprov-list-card')).toBeVisible({ timeout: 10_000 })

    // The pre-seeded google/microsoft/apple entries from migration 47 are
    // visible to readers (proof the GET list endpoint worked under their read
    // permission). Provider names are dynamic seed data — asserted inside the
    // list-card testid scope.
    const listCard = byTestId(page, 'authprov-list-card')
    await expect(listCard).toContainText('google')
    await expect(listCard).toContainText('microsoft')
    await expect(listCard).toContainText('apple')

    // All mutating + test controls hidden — every <Can permission=
    // Permissions.AuthProvidersManage> wrapper should render null.
    // Names come from the live DOM:
    //   Add button:      aria-label="Add authentication provider"
    //   Per-row Switch:  aria-label="Toggle <name>"
    //   Per-row Buttons: aria-label "Test <name>" / "Edit <name>" /
    //                    "Delete <name>" — the aria-label overrides the
    //                    visible "Test"/"Edit"/"Delete" text, so match the
    //                    "<verb> " prefix.
    await expect(byTestId(page, 'authprov-add-button')).toHaveCount(0)
    await expect(page.getByTestId(/^authprov-edit-button-/)).toHaveCount(0)
    await expect(page.getByTestId(/^authprov-delete-button-/)).toHaveCount(0)
    await expect(page.getByTestId(/^authprov-test-button-/)).toHaveCount(0)
    await expect(page.getByTestId(/^authprov-toggle-switch-/)).toHaveCount(0)
  })

  test('manager: all controls visible', async ({ page, testInfra }) => {
    await loginAsAuthProvidersManager(
      page,
      testInfra.baseURL,
      testInfra.apiURL,
    )

    await page.goto(`${testInfra.baseURL}/settings/auth-providers`)
    await expect(byTestId(page, 'authprov-list-card')).toBeVisible({ timeout: 10_000 })

    // Add Provider dropdown visible.
    await expect(byTestId(page, 'authprov-add-button')).toBeVisible()

    // Per-row controls — at least one of each on the pre-seeded rows
    // (derived `authprov-{edit,test,delete}-button-<id>` / toggle-switch ids).
    await expect(page.getByTestId(/^authprov-edit-button-/).first()).toBeVisible()
    await expect(page.getByTestId(/^authprov-test-button-/).first()).toBeVisible()
    await expect(page.getByTestId(/^authprov-delete-button-/).first()).toBeVisible()
    // 3 pre-seeded rows × 1 switch each = at least 3.
    expect(await page.getByTestId(/^authprov-toggle-switch-/).count()).toBeGreaterThanOrEqual(3)
  })

  test('root admin (wildcard): all controls visible', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    await page.goto(`${testInfra.baseURL}/settings/auth-providers`)
    await expect(byTestId(page, 'authprov-list-card')).toBeVisible({ timeout: 10_000 })

    await expect(byTestId(page, 'authprov-add-button')).toBeVisible()
    await expect(page.getByTestId(/^authprov-edit-button-/).first()).toBeVisible()
  })
})
