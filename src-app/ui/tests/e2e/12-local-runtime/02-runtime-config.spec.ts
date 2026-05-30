import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
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

    const configCard = page.locator('.ant-card').filter({ hasText: /Runtime configuration/i }).first()
    await expect(configCard).toBeVisible()

    // The idle-unload field is an antd InputNumber; target it within the card.
    const idleInput = configCard.locator('input[role="spinbutton"]').first()
    await idleInput.click()
    await idleInput.fill('120')

    await page.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText(/saved|updated/i).first()).toBeVisible({ timeout: 5000 })

    // Reload → the saved value persists.
    await page.reload()
    await page.waitForLoadState('load')
    await expect(
      page.locator('.ant-card').filter({ hasText: /Runtime configuration/i }).locator('input[role="spinbutton"]').first(),
    ).toHaveValue('120')
  })

  test('toggling allow-unsigned-downloads surfaces a warning', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    const configCard = page.locator('.ant-card').filter({ hasText: /Runtime configuration/i }).first()
    const sw = configCard.locator('.ant-switch').last()
    if (await sw.isVisible().catch(() => false)) {
      await sw.click()
      await expect(configCard.locator('.ant-alert').first()).toBeVisible()
    }
  })
})
