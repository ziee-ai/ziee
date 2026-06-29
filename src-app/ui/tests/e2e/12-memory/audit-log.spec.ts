import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — memory audit log records ADD / UPDATE / DELETE / BULK_DELETE.
 *
 * Plan §11 PII mitigation: "audit log table (Phase 5)". This
 * exercises the GET /api/memory/audit-log endpoint via the UI flow.
 */

async function memoryUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(apiURL, adminToken, username, `${username}@ex.com`, 'password123', [
    'profile::read',
    'profile::edit',
    'memory::read',
    'memory::write',
  ])
  return username
}

test.describe('Memory — audit log', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('add+update+delete each record an audit entry', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'audit')
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)
    const authHeader = { Authorization: `Bearer ${userToken}` }

    // Drive memory ops via REST (faster than UI) but assert via the
    // public audit-log endpoint we just shipped.
    const create = await page.request.post(`${apiURL}/api/memories`, {
      headers: authHeader,
      data: { content: 'User code is Bravo' },
    })
    const row = await create.json()
    await page.request.patch(`${apiURL}/api/memories/${row.id}`, {
      headers: authHeader,
      data: { content: 'User code is Charlie' },
    })
    await page.request.delete(`${apiURL}/api/memories/${row.id}`, {
      headers: authHeader,
    })

    const log = await page.request.get(`${apiURL}/api/memory/audit-log`, {
      headers: authHeader,
    })
    expect(log.status()).toBe(200)
    const entries = await log.json()
    const ops = entries.map((e: any) => e.op)
    expect(ops).toContain('ADD')
    expect(ops).toContain('UPDATE')
    expect(ops).toContain('DELETE')
  })

  test('audit log table displays entries and the limit filter narrows them', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'auditui')
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)
    const authHeader = { Authorization: `Bearer ${userToken}` }

    // Seed ≥3 audit entries (add + update + delete on one memory).
    const create = await page.request.post(`${apiURL}/api/memories`, {
      headers: authHeader,
      data: { content: 'Audit UI memory' },
    })
    const row = await create.json()
    await page.request.patch(`${apiURL}/api/memories/${row.id}`, {
      headers: authHeader,
      data: { content: 'Audit UI memory v2' },
    })
    await page.request.delete(`${apiURL}/api/memories/${row.id}`, {
      headers: authHeader,
    })

    await page.goto(`${baseURL}/settings/memory`)
    const card = byTestId(page, 'memory-audit-card')
    await expect(card).toBeVisible({ timeout: 30000 })

    // The table renders the seeded entries (≥3 data rows). Kit Table emits
    // `${tableTestid}-row-${rowKey}` per row.
    const rows = byTestId(page, 'memory-audit-table').locator(
      '[data-testid^="memory-audit-table-row-"]',
    )
    await expect
      .poll(async () => await rows.count(), { timeout: 15000 })
      .toBeGreaterThanOrEqual(3)

    // Apply the "Show last" limit = 1 → the table narrows to a single row.
    const limit = byTestId(card, 'memory-audit-limit-input')
    await limit.click()
    await limit.press('ControlOrMeta+a')
    await limit.fill('1')
    await byTestId(card, 'memory-audit-limit-apply').click()
    await expect.poll(async () => await rows.count(), { timeout: 15000 }).toBe(1)
  })
})
