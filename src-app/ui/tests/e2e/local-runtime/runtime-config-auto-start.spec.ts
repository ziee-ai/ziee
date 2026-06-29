import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'
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

    const card = byTestId(page, 'llmrt-runtime-config-card')
    await expect(card).toBeVisible({ timeout: 30000 })

    const field = byTestId(card, 'llmrt-config-autostart-timeout')
    await field.click()
    await field.press('ControlOrMeta+a')
    await field.fill('45')

    await byTestId(card, 'llmrt-config-save-btn').click()
    await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({
      timeout: 10000,
    })

    await page.reload()
    await page.waitForLoadState('load')
    await expect(
      byTestId(byTestId(page, 'llmrt-runtime-config-card'), 'llmrt-config-autostart-timeout'),
    ).toHaveValue('45', { timeout: 30000 })
  })
})
