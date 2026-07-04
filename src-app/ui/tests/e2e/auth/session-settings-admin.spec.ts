import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the admin "Sessions" settings page (/settings/sessions): renders
 * the token-lifetime card, saves an edit, and the value persists across
 * a reload (round-trips through PUT /api/auth/session-settings).
 */

test.describe('Auth — session settings admin page', () => {
  test('admin edits + persists the session length', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/sessions`, {
      waitUntil: 'domcontentloaded',
    })
    await expect(page.getByTestId('session-settings-card')).toBeVisible({
      timeout: 30_000,
    })

    // Defaults from the config seed (24h / 30d).
    const accessInput = page.getByTestId('session-settings-access-hours')
    const daysInput = page.getByTestId('session-settings-session-days')
    await expect(accessInput).toHaveValue(/24/)
    await expect(daysInput).toHaveValue(/30/)

    // Edit the session length + save.
    await daysInput.fill('14')
    const save = page.getByTestId('session-settings-save')
    await expect(save).toBeEnabled()
    await save.click()

    // Persisted server-side: a hard reload re-fetches the row.
    await page.reload({ waitUntil: 'domcontentloaded' })
    await expect(page.getByTestId('session-settings-card')).toBeVisible({
      timeout: 30_000,
    })
    await expect(
      page.getByTestId('session-settings-session-days'),
    ).toHaveValue(/14/, { timeout: 15_000 })
  })
})
