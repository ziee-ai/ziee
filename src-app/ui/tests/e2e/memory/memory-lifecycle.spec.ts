import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — MyMemoriesSection full CRUD lifecycle (audit gap r2-a1f0a0412c81).
 *
 * The other memory specs cover the read-side surfaces — search
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
    await byTestId(page, 'memory-add-btn').click()
    const createDrawer = page.getByRole('dialog')
    await expect(byTestId(createDrawer, 'memory-create-form')).toBeVisible()
    await byTestId(createDrawer, 'memory-create-content-input').fill(original)

    const createResp = page.waitForResponse(
      (r) =>
        r.url().endsWith('/api/memories') &&
        r.request().method() === 'POST' &&
        r.status() === 201,
    )
    await byTestId(page, 'memory-create-submit-btn').click()
    await createResp

    // The newly-created memory renders as a row carrying its content.
    const createdRow = page.locator('[data-memory-id]', { hasText: original })
    await expect(createdRow).toBeVisible({ timeout: 10_000 })
    const id = await createdRow.getAttribute('data-memory-id')

    // ----------------------------- EDIT -----------------------------
    await byTestId(createdRow, `memory-row-edit-btn-${id}`).click()
    const editDrawer = page.getByRole('dialog')
    await expect(byTestId(editDrawer, 'memory-edit-form')).toBeVisible()
    // The edit form is pre-filled with the persisted content.
    await expect(byTestId(editDrawer, 'memory-edit-content-input')).toHaveValue(
      original,
    )

    await byTestId(editDrawer, 'memory-edit-content-input').fill(updated)
    const editResp = page.waitForResponse(
      (r) =>
        /\/api\/memories\/[0-9a-f-]+$/.test(r.url()) &&
        // The memory-row edit endpoint is PATCH /api/memories/{id}
        // (Memory.update), not PUT — the app sends PATCH correctly.
        r.request().method() === 'PATCH' &&
        r.ok(),
    )
    await byTestId(page, 'memory-edit-submit-btn').click()
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
    await byTestId(editedRow, `memory-row-delete-btn-${id}`).click()
    // The per-row delete opens a confirm dialog; confirm via its danger button.
    const deleteResp = page.waitForResponse(
      (r) =>
        /\/api\/memories\/[0-9a-f-]+$/.test(r.url()) &&
        r.request().method() === 'DELETE' &&
        r.ok(),
    )
    await byTestId(page, `memory-row-delete-confirm-${id}-confirm`).click()
    await deleteResp

    // The memory is gone from the list.
    await expect(
      page.locator('[data-memory-id]', { hasText: updated }),
    ).toHaveCount(0, { timeout: 10_000 })
  })
})
