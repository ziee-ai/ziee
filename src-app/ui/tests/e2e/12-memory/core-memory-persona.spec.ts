import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  getCurrentUserToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'

/**
 * E2E — assistant core memory editor (Plan §9 Phase 6).
 *
 * Exercises the CRUD on /api/assistants/{id}/core-memory. The actual
 * block-injection behavior in the chat path is tested in the backend
 * integration suite (memory module retriever tests).
 */

test.describe('Memory — assistant core memory blocks', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('block_label validation rejects bad slugs', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)
    const username = `core_${Date.now().toString(36)}`
    await createTestUser(
      apiURL,
      adminToken,
      username,
      `${username}@ex.com`,
      'password123',
      [
        'profile::read',
        'profile::edit',
        'memory::core::read',
        'memory::core::write',
      ],
    )
    await login(page, baseURL, username, 'password123')
    const userToken = await getCurrentUserToken(page)

    // Random UUID — the FK to assistants(id) will reject, but the
    // 400 we expect for the bad slug fires BEFORE the FK check
    // because validation happens in the handler.
    const assistantId = '00000000-0000-0000-0000-000000000001'
    const res = await page.request.put(`${apiURL}/api/assistants/core-memory`, {
      headers: { Authorization: `Bearer ${userToken}` },
      data: {
        assistant_id: assistantId,
        block_label: 'NOT A VALID SLUG',
        content: 'hi',
        char_limit: 100,
      },
    })
    expect(res.status()).toBe(400)
  })

  test('editor UI: pick assistant → add block → it lists → delete it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const adminToken = await getAdminToken(apiURL)

    // Seed an assistant so the per-assistant picker has an option.
    const assistantName = `Core Persona ${Date.now()}`
    const created = await page.request.post(`${apiURL}/api/assistants`, {
      headers: { Authorization: `Bearer ${adminToken}` },
      data: {
        name: assistantName,
        description: 'core-memory editor e2e',
        instructions: 'You are a test assistant.',
        is_template: false,
      },
    })
    expect(created.ok()).toBe(true)

    await page.goto(`${baseURL}/settings/memory`)
    const card = page
      .locator('.ant-card')
      .filter({ hasText: 'Per-assistant core memory' })
    await expect(card).toBeVisible({ timeout: 20000 })

    // Pick the seeded assistant — the blocks editor renders for it.
    await card.getByRole('combobox', { name: 'Pick an assistant' }).click()
    await page.getByRole('option', { name: assistantName }).click()

    // Add a core-memory block via the editor modal.
    await card.getByRole('button', { name: 'Add block' }).click()
    const modal = page.getByRole('dialog', { name: 'Add core memory block' })
    await expect(modal).toBeVisible()
    await modal.getByLabel('Label').fill('persona')
    await modal.getByLabel('Content').fill('Always answer concisely.')
    await modal.getByRole('button', { name: 'Add' }).click()

    await expect(page.getByText('Block added')).toBeVisible()
    // The new block lists under the assistant's editor.
    await expect(card.getByText('persona')).toBeVisible()

    // Delete it via the per-row Popconfirm.
    await card.getByRole('button', { name: 'Delete block persona' }).click()
    const popconfirm = page.locator('.ant-popconfirm:visible')
    await expect(popconfirm.getByText('Delete this block?')).toBeVisible()
    await popconfirm.locator('.ant-btn-primary').click()
    await expect(page.getByText('Block deleted')).toBeVisible()
    await expect(card.getByText('persona')).toHaveCount(0)
  })
})
