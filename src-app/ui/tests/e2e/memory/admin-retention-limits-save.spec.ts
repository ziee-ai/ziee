import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Memory admin RetentionLimitsSection (two numeric inputs + Save).
 *
 * The "Retention & extraction limits" card has `soft_delete_grace_days` and
 * `daily_extraction_quota` InputNumbers and a Save button that surfaces
 * "Retention & limits saved." This edits both and asserts the save + reload
 * persistence.
 */

async function setNumber(field: import('@playwright/test').Locator, value: number) {
  await field.click()
  await field.press('ControlOrMeta+a')
  await field.fill(String(value))
  await field.press('Enter')
}

test.describe('Memory — admin retention limits', () => {
  test('editing both numeric limits and saving persists across reload', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/memory-admin`)

    await expect(byTestId(page, 'memory-retention-card')).toBeVisible({
      timeout: 30000,
    })

    const grace = byTestId(page, 'memory-retention-grace-input')
    const quota = byTestId(page, 'memory-retention-quota-input')
    await setNumber(grace, 14)
    await setNumber(quota, 250)

    await byTestId(page, 'memory-retention-save-btn').click()
    await expect(page.locator('[data-sonner-toast]')).toContainText(
      'Retention & limits saved.',
      { timeout: 10000 },
    )

    // Reload → persisted values come back.
    await page.goto(`${baseURL}/settings/memory-admin`)
    await expect(byTestId(page, 'memory-retention-grace-input')).toHaveValue(
      '14',
      { timeout: 30000 },
    )
    await expect(byTestId(page, 'memory-retention-quota-input')).toHaveValue(
      '250',
    )
  })
})
