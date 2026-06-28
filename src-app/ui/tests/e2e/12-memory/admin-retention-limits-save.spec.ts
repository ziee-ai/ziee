import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

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

    const card = page.locator(
      '.ant-card:has(.ant-card-head-title:has-text("Retention & extraction limits"))',
    )
    await expect(card).toBeVisible({ timeout: 30000 })

    const grace = card.getByLabel('Soft-delete grace days')
    const quota = card.getByLabel('Daily extraction quota (per user)')
    await setNumber(grace, 14)
    await setNumber(quota, 250)

    await card.getByRole('button', { name: 'Save', exact: true }).click()
    await expect(page.getByText('Retention & limits saved.')).toBeVisible({
      timeout: 10000,
    })

    // Reload → persisted values come back.
    await page.goto(`${baseURL}/settings/memory-admin`)
    const card2 = page.locator(
      '.ant-card:has(.ant-card-head-title:has-text("Retention & extraction limits"))',
    )
    await expect(card2.getByLabel('Soft-delete grace days')).toHaveValue('14', {
      timeout: 30000,
    })
    await expect(
      card2.getByLabel('Daily extraction quota (per user)'),
    ).toHaveValue('250')
  })
})
