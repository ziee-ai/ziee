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
import { Permissions } from '../../../src/api-client/permissions'
import { byTestId } from '../testid'

test.describe('summarization-admin — permission gating', () => {
  test('member without summarization::settings::read: entry hidden + deep-link 403', async ({
    page,
    testInfra,
  }) => {
    await loginAsMember(page, testInfra.baseURL, testInfra.apiURL)

    // The admin sidebar entry is gated → hidden.
    await page.goto(`${testInfra.baseURL}/settings`)
    await expect(
      byTestId(page, 'settings-nav-menu-item-summarization-admin'),
    ).toHaveCount(0)

    // Deep-link → inline "Not authorized", URL preserved (route gate fires).
    await page.goto(`${testInfra.baseURL}/settings/summarization-admin`)
    await expect(byTestId(page, 'settings-forbidden-result')).toBeVisible({ timeout: 10_000 })
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
      byTestId(page, 'summ-after-tokens-input'),
    ).toBeVisible({ timeout: 10_000 })
    await expect(byTestId(page, 'settings-forbidden-result')).toHaveCount(0)
  })
})
