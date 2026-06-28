import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { gotoRuntimeSettings } from './helpers/local-runtime-helpers'

/**
 * E2E — the Runtime configuration card's "Auto-start timeout" field.
 *
 * Audit gap: 02-runtime-config.spec only exercises idle_unload_secs; the
 * `auto_start_timeout_secs` field (RuntimeConfigCard.tsx) was untested. This
 * edits it, saves, reloads, and asserts it persisted (PUT /settings round-trip).
 */

test.describe('Local Runtime — auto-start timeout config', () => {
  test('edit auto-start timeout, save, persist across reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await gotoRuntimeSettings(page, baseURL)

    const card = page
      .locator('.ant-card')
      .filter({ hasText: /Runtime configuration/i })
      .first()
    await expect(card).toBeVisible({ timeout: 30000 })

    const field = card.getByLabel('Auto-start timeout (seconds)')
    await field.click()
    await field.press('ControlOrMeta+a')
    await field.fill('45')

    await card.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText(/saved|updated/i).first()).toBeVisible({
      timeout: 10000,
    })

    await page.reload()
    await page.waitForLoadState('load')
    await expect(
      page
        .locator('.ant-card')
        .filter({ hasText: /Runtime configuration/i })
        .first()
        .getByLabel('Auto-start timeout (seconds)'),
    ).toHaveValue('45', { timeout: 30000 })
  })
})
