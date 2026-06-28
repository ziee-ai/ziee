/**
 * Permission-gating E2E for /settings/summarization-admin.
 *
 * The page + sidebar slot are gated on `summarization::settings::read`
 * (module.tsx route `permission` + settingsAdminPages slot `permission`).
 * admin-settings.spec.ts only ever logs in as the root admin (is_admin
 * bypass), so the NEGATIVE gate — a non-admin without the read perm — was
 * never asserted.
 *
 * Uses the `no-403` fixture so any accidental /api/* 403 during the member
 * flow fails loudly rather than masking a missing UI gate.
 */
import { test, expect } from './no-403'
import { loginAsMember, loginWithPerms } from './fixtures'
import { Permissions } from '../../../src/api-client/types'

test.describe('summarization-admin — permission gating', () => {
  test('member without summarization::settings::read: entry hidden + deep-link 403', async ({
import { test, expect } from './no-403'
import { loginAsMember } from './fixtures'

/**
 * Summarization admin page is gated by `SummarizationSettingsRead`. The
 * existing 14-summarization specs always log in as admin, so the GATE itself
 * (a non-admin being kept out) was never exercised. A basic member must not see
 * the "Summarization" settings entry and must hit the inline 403 on a deep-link.
 * Runs under the no-403 fixture, so any accidental admin-only API call also
 * fails the test.
 */
test.describe('summarization module — permission gating', () => {
  test('non-admin: entry hidden + deep-link renders 403', async ({
    page,
    testInfra,
  }) => {
    await loginAsMember(page, testInfra.baseURL, testInfra.apiURL)

    // The admin sidebar entry is gated → hidden.
    // The "Summarization" admin entry is absent from a non-admin's settings menu.
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(
      page.getByRole('menuitem', { name: /^Summarization$/ }),
    ).toHaveCount(0)

    // Deep-link → inline "Not authorized", URL preserved (route gate fires).
    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)
    await expect(page.getByText(/Not authorized/i)).toBeVisible({ timeout: 10_000 })
    expect(page.url()).toContain('/settings/summarization-admin')
  })

  test('reader with summarization::settings::read: page renders', async ({
    page,
    testInfra,
  }) => {
    await loginWithPerms(page, testInfra.baseURL, testInfra.apiURL, [
      Permissions.SummarizationSettingsRead,
    ])

    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)

    // The gated page renders for a holder of the read perm (no 403).
    await expect(
      page.getByText('Summarize after N tokens'),
    ).toBeVisible({ timeout: 10_000 })
    await expect(page.getByText(/Not authorized/i)).toHaveCount(0)
  })
    // Deep-link to the admin page → inline 403 (URL preserved).
    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)
    await expect(page.getByText(/Not authorized/i)).toBeVisible()
    expect(page.url()).toContain('/settings/summarization-admin')
  })
})
