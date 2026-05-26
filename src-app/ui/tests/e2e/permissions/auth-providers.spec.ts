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

test.describe('auth-providers — permission gating', () => {
  test('member: settings menu entry hidden + deep-link 403', async ({
    page,
    testInfra,
  }) => {
    await loginAsMember(page, testInfra.baseURL, testInfra.apiURL)

    // Menu entry hidden.
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(
      page.getByRole('menuitem', { name: /^Auth providers$/ }),
    ).toHaveCount(0)

    // Deep-link → inline 403, URL preserved.
    await page.goto(`${testInfra.baseURL}/settings/auth-providers`)
    await expect(page.getByText(/Not authorized/i)).toBeVisible()
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

    // The page title + table render — readers see the data.
    await expect(
      page.getByRole('heading', { name: /Auth providers/i }),
    ).toBeVisible({ timeout: 10_000 })

    // The pre-seeded google/microsoft/apple rows from migration 47 are
    // visible to readers (proof the GET list endpoint worked under
    // their read permission).
    await expect(
      page.getByRole('row').filter({ hasText: 'google' }).first(),
    ).toBeVisible()
    await expect(
      page.getByRole('row').filter({ hasText: 'microsoft' }).first(),
    ).toBeVisible()
    await expect(
      page.getByRole('row').filter({ hasText: 'apple' }).first(),
    ).toBeVisible()

    // All mutating + test controls hidden — every <Can permission=
    // Permissions.AuthProvidersManage> wrapper should render null.
    await expect(
      page.getByRole('button', { name: /add provider/i }),
    ).toHaveCount(0)
    await expect(page.getByRole('button', { name: /^edit$/i })).toHaveCount(0)
    await expect(page.getByRole('button', { name: /^delete$/i })).toHaveCount(0)
    await expect(page.getByRole('button', { name: /^test$/i })).toHaveCount(0)
    // The per-row Switch is also inside <Can> — Antd renders it as
    // a switch role.
    await expect(page.getByRole('switch')).toHaveCount(0)
  })

  test('manager: all controls visible', async ({ page, testInfra }) => {
    await loginAsAuthProvidersManager(
      page,
      testInfra.baseURL,
      testInfra.apiURL,
    )

    await page.goto(`${testInfra.baseURL}/settings/auth-providers`)
    await expect(
      page.getByRole('heading', { name: /Auth providers/i }),
    ).toBeVisible({ timeout: 10_000 })

    // Add Provider dropdown visible.
    await expect(
      page.getByRole('button', { name: /add provider/i }),
    ).toBeVisible()

    // Per-row controls — at least one of each on the pre-seeded rows.
    await expect(
      page.getByRole('button', { name: /^edit$/i }).first(),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: /^test$/i }).first(),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: /^delete$/i }).first(),
    ).toBeVisible()
    // 3 pre-seeded rows × 1 switch each = at least 3.
    expect(await page.getByRole('switch').count()).toBeGreaterThanOrEqual(3)
  })

  test('root admin (wildcard): all controls visible', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)

    await page.goto(`${testInfra.baseURL}/settings/auth-providers`)
    await expect(
      page.getByRole('heading', { name: /Auth providers/i }),
    ).toBeVisible({ timeout: 10_000 })

    await expect(
      page.getByRole('button', { name: /add provider/i }),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: /^edit$/i }).first(),
    ).toBeVisible()
  })
})
