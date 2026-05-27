import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — manual memory add + list + delete on the /settings/memory page.
 *
 * Phase 1 plan §9: "MemoriesPage.tsx with manual add/list/edit/delete
 * (no embedding, no AI; pure text storage)". This spec exercises that
 * happy path against the live REST surface.
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

test.describe('Memory — manual add', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('add → list → delete', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const username = await memoryUser(apiURL, 'mem_add')
    await login(page, baseURL, username, 'password123')

    await page.goto(`${baseURL}/settings/memory`)
    // After the settings-page consolidation, "My memories" is a Card
    // section title (not a Typography heading), so anchor on the
    // section's unique CTA instead.
    await expect(
      page.getByRole('button', { name: /Add memory/ }),
    ).toBeVisible()

    // Add
    await page.getByRole('button', { name: /Add memory/ }).click()
    const dialog = page.getByRole('dialog')
    await expect(dialog).toBeVisible()
    await dialog.getByLabel('Content').fill('User prefers TypeScript over JavaScript')
    await dialog.getByRole('button', { name: /^Add$/ }).click()
    await expect(page.getByText('Memory added')).toBeVisible({ timeout: 5000 })

    // List row appears
    await expect(
      page.getByText('User prefers TypeScript over JavaScript'),
    ).toBeVisible()

    // Delete (Popconfirm) — okText="Delete" per codebase convention;
    // `exact: true` distinguishes the Popconfirm's "Delete" OK button
    // from the row's trash icon (aria-label = "Delete memory <id>").
    await page
      .getByRole('row', { name: /TypeScript/ })
      .getByRole('button')
      .last()
      .click()
    await page.getByRole('button', { name: 'Delete', exact: true }).click()
    await expect(page.getByText('Memory deleted')).toBeVisible({ timeout: 5000 })
  })
})
