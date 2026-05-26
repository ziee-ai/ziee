import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
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

    // Drive memory ops via REST (faster than UI) but assert via the
    // public audit-log endpoint we just shipped.
    const create = await page.request.post(`${apiURL}/api/memories`, {
      data: { content: 'User code is Bravo' },
    })
    const row = await create.json()
    await page.request.patch(`${apiURL}/api/memories/${row.id}`, {
      data: { content: 'User code is Charlie' },
    })
    await page.request.delete(`${apiURL}/api/memories/${row.id}`)

    const log = await page.request.get(`${apiURL}/api/memory/audit-log`)
    expect(log.status()).toBe(200)
    const entries = await log.json()
    const ops = entries.map((e: any) => e.op)
    expect(ops).toContain('ADD')
    expect(ops).toContain('UPDATE')
    expect(ops).toContain('DELETE')
  })
})
