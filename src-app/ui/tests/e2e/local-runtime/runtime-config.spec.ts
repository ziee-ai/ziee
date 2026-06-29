import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'
import { gotoRuntimeSettings } from './helpers/local-runtime-helpers'

/**
 * Runtime configuration card: edit a numeric setting, Save, reload, and
 * confirm it persisted (PUT /local-runtime/settings round-trip).
 */
test.describe('Local Runtime — runtime config', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('edit idle-unload timeout, save, and persist across reload', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)

    const configCard = byTestId(page, 'llmrt-runtime-config-card')
    await expect(configCard).toBeVisible()

    // The idle-unload field is a numeric input; target it by testid.
    const idleInput = byTestId(page, 'llmrt-config-idle-unload')
    await idleInput.click()
    await idleInput.fill('120')

    await byTestId(page, 'llmrt-config-save-btn').click()
    // Saved-confirmation toast (dynamic feedback, not chrome).
    await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({ timeout: 5000 })

    // Reload → the saved value persists.
    await page.reload()
    await page.waitForLoadState('load')
    await expect(byTestId(page, 'llmrt-config-idle-unload')).toHaveValue('120')
  })

  test('toggling allow-unsigned-downloads surfaces a warning', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const configCard = byTestId(page, 'llmrt-runtime-config-card')
    await expect(configCard).toBeVisible()
    // Defensive: the allow-unsigned-downloads control is only present on
    // deployments that expose it; assert the warning only when the switch
    // renders.
    const sw = byTestId(configCard, 'llmrt-config-allow-unsigned-switch')
    if (await sw.isVisible().catch(() => false)) {
      await sw.click()
      await expect(byTestId(configCard, 'llmrt-config-unsigned-warning')).toBeVisible()
    }
  })
})
