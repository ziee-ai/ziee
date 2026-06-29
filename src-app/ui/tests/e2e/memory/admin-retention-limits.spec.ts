import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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

    const grace = byTestId(page, 'memory-retention-grace-input')
    await expect(grace).toBeVisible({ timeout: 30000 })

    await grace.click()
    await grace.press('ControlOrMeta+a')
    await grace.fill('14')

    const quota = byTestId(page, 'memory-retention-quota-input')
    await quota.click()
    await quota.press('ControlOrMeta+a')
    await quota.fill('25')

    await byTestId(page, 'memory-retention-save-btn').click()
    await expect(page.locator('[data-sonner-toast]')).toContainText(
      'Retention & limits saved.',
      { timeout: 30000 },
    )
  })
})
