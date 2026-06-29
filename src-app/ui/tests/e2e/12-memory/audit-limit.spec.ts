import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E — Memory "Audit log" limit selector (AuditLogSection.tsx).
 *
 * Audit gap: the "Show last N" InputNumber + Apply button
 * (handleSubmit → Stores.MemoryAudit.setLimit → re-fetch with the new
 * `limit`) was never exercised. This sets the limit and asserts the
 * audit-log GET re-fires with the chosen `limit` query param — the real
 * store→API round-trip, not just a render check.
 */

test.describe('Memory — audit log limit', () => {
  test('Apply re-fetches the audit log with the chosen limit', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/memory`)

    // The Audit log card + its "Show last" field render.
    await expect(byTestId(page, 'memory-audit-card')).toBeVisible({
      timeout: 30000,
    })
    const input = byTestId(page, 'memory-audit-limit-input')
    await input.click()
    await input.press('ControlOrMeta+a')
    await input.fill('7')

    const [resp] = await Promise.all([
      page.waitForResponse(
        r =>
          r.url().includes('/api/memory/audit-log') &&
          r.url().includes('limit=7'),
        { timeout: 30000 },
      ),
      byTestId(page, 'memory-audit-limit-apply').click(),
    ])
    expect(resp.status()).toBe(200)
  })
})
