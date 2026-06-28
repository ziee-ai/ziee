import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the Memory admin "Retention & extraction limits" card
 * (RetentionLimitsSection.tsx) — a form with two numeric inputs.
 *
 * Audit gap: this section (soft-delete grace days + daily extraction quota
 * → Stores.MemoryAdmin.update) had zero E2E. This edits both numeric fields
 * and saves, asserting the success toast (real store→PUT round-trip).
 */

test.describe('Memory — admin retention limits', () => {
  test('edits both numeric fields and saves', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/memory-admin`)

    const grace = page.getByLabel('Soft-delete grace days')
    await expect(grace).toBeVisible({ timeout: 30000 })

    const card = page.locator('.ant-card', {
      has: page.getByLabel('Soft-delete grace days'),
    })

    await grace.click()
    await grace.press('ControlOrMeta+a')
    await grace.fill('14')

    const quota = page.getByLabel('Daily extraction quota (per user)')
    await quota.click()
    await quota.press('ControlOrMeta+a')
    await quota.fill('25')

    await card.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByText('Retention & limits saved.')).toBeVisible({
      timeout: 30000,
    })
  })
})
