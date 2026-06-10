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
      page.getByRole('menuitem', { name: /^Auth Providers$/ }),
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

    // The pre-seeded google/microsoft/apple entries from migration 47
    // are visible to readers (proof the GET list endpoint worked under
    // their read permission). The list renders each provider as a
    // <Card> with the name inside an antd `<Text>` (span.ant-typography)
    // — not a `<tr>`. We anchor on the typography span specifically
    // because the row ALSO renders the provider_type as an antd <Tag>
    // (span.ant-tag), and for the Apple OIDC seed the type string is
    // ALSO "apple" — a bare `getByText('apple', { exact: true })`
    // hits strict-mode (2 matches: name span + tag span).
    const providerName = (name: string) =>
      page.locator('span.ant-typography').getByText(name, { exact: true })
    await expect(providerName('google')).toBeVisible()
    await expect(providerName('microsoft')).toBeVisible()
    await expect(providerName('apple')).toBeVisible()

    // All mutating + test controls hidden — every <Can permission=
    // Permissions.AuthProvidersManage> wrapper should render null.
    // Names come from the live DOM:
    //   Add button:      aria-label="Add authentication provider"
    //   Per-row Switch:  aria-label="Toggle <name>"
    //   Per-row Buttons: aria-label "Test <name>" / "Edit <name>" /
    //                    "Delete <name>" — the aria-label overrides the
    //                    visible "Test"/"Edit"/"Delete" text, so match the
    //                    "<verb> " prefix.
    await expect(
      page.getByRole('button', { name: /^Add authentication provider$/i }),
    ).toHaveCount(0)
    await expect(page.getByRole('button', { name: /^Edit /i })).toHaveCount(0)
    await expect(page.getByRole('button', { name: /^Delete /i })).toHaveCount(0)
    await expect(page.getByRole('button', { name: /^Test /i })).toHaveCount(0)
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
      page.getByRole('button', { name: /^Add authentication provider$/i }),
    ).toBeVisible()

    // Per-row controls — at least one of each on the pre-seeded rows.
    // The buttons carry aria-label "Edit <name>" / "Test <name>" /
    // "Delete <name>" (which overrides the visible verb text), so match
    // the "<verb> " prefix rather than the bare verb.
    await expect(
      page.getByRole('button', { name: /^Edit / }).first(),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: /^Test / }).first(),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: /^Delete / }).first(),
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
      page.getByRole('button', { name: /^Add authentication provider$/i }),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: /^Edit / }).first(),
    ).toBeVisible()
  })
})
