import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — MyMemoriesSection full CRUD lifecycle (audit gap r2-a1f0a0412c81).
 *
 * The other 12-memory specs cover the read-side surfaces — search
 * (`memories-search`), the Kind filter (`memories-filter-toolbar`),
 * pagination (`memories-pagination`), and export (`memories-export`) —
 * but none drives an individual memory through CREATE → EDIT → DELETE
 * via the UI. This walks that lifecycle end-to-end:
 *   - the "Add memory" drawer  → real `POST   /api/memories`,
 *   - the per-row Edit drawer   → real `PUT    /api/memories/{id}`,
 *   - the per-row Delete confirm → real `DELETE /api/memories/{id}`,
 * asserting on the rendered list + the real network calls at each step
 * (nothing mocked — `Stores.Memories.{create,update,remove}`).
 */

async function memoryUser(apiURL: string, name: string) {
  const adminToken = await getAdminToken(apiURL)
  const username = `${name}_${Date.now().toString(36)}`
  await createTestUser(
    apiURL,
    adminToken,
    username,
    `${username}@ex.com`,
    'password123',
    ['profile::read', 'profile::edit', 'memory::read', 'memory::write'],
  )
  return username
}

test.describe('Memory — My memories CRUD lifecycle', () => {
  test('create → edit → delete an individual memory via the UI', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const username = await memoryUser(apiURL, 'mem_life')
    await login(page, baseURL, username, 'password123')

    await page.goto(`${baseURL}/settings/memory`)

    const original = `User favors the ORYXPAD keyboard ${Date.now().toString(36)}`
    const updated = `User now favors the GNUBOARD keyboard instead`

    // ---------------------------- CREATE ----------------------------
    // The card's "Add memory" affordance opens the create drawer.
    await page.getByRole('button', { name: 'Add memory' }).click()
    const createDrawer = page.getByRole('dialog')
    await expect(createDrawer.getByText('Add memory')).toBeVisible()
    await createDrawer.getByLabel('Content').fill(original)

    const createResp = page.waitForResponse(
      (r) =>
        r.url().endsWith('/api/memories') &&
        r.request().method() === 'POST' &&
        r.status() === 201,
    )
    await createDrawer.getByRole('button', { name: 'Add', exact: true }).click()
    await createResp

    // The newly-created memory renders as a row carrying its content.
    const createdRow = page.locator('[data-memory-id]', { hasText: original })
    await expect(createdRow).toBeVisible({ timeout: 10_000 })

    // ----------------------------- EDIT -----------------------------
    await createdRow.getByRole('button', { name: 'Edit memory' }).click()
    const editDrawer = page.getByRole('dialog')
    await expect(editDrawer.getByText('Edit memory')).toBeVisible()
    // The edit form is pre-filled with the persisted content.
    await expect(editDrawer.getByLabel('Content')).toHaveValue(original)

    await editDrawer.getByLabel('Content').fill(updated)
    const editResp = page.waitForResponse(
      (r) =>
        /\/api\/memories\/[0-9a-f-]+$/.test(r.url()) &&
        r.request().method() === 'PUT' &&
        r.ok(),
    )
    await editDrawer.getByRole('button', { name: 'Save' }).click()
    await editResp

    // The row now shows the updated content; the original is gone.
    await expect(
      page.locator('[data-memory-id]', { hasText: updated }),
    ).toBeVisible({ timeout: 10_000 })
    await expect(
      page.locator('[data-memory-id]', { hasText: original }),
    ).toHaveCount(0)

    // ---------------------------- DELETE ----------------------------
    const editedRow = page.locator('[data-memory-id]', { hasText: updated })
    await editedRow.getByRole('button', { name: /^Delete memory/ }).click()
    // The per-row delete opens an antd Popconfirm; confirm via its
    // "Delete" danger button.
    const deleteResp = page.waitForResponse(
      (r) =>
        /\/api\/memories\/[0-9a-f-]+$/.test(r.url()) &&
        r.request().method() === 'DELETE' &&
        r.ok(),
    )
    await page
      .locator('.ant-popover:visible, .ant-popconfirm:visible')
      .getByRole('button', { name: 'Delete' })
      .click()
    await deleteResp

    // The memory is gone from the list.
    await expect(
      page.locator('[data-memory-id]', { hasText: updated }),
    ).toHaveCount(0, { timeout: 10_000 })
  })
})
