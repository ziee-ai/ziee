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
    const assistant = await created.json()

    await page.goto(`${baseURL}/settings/memory`)
    const card = byTestId(page, 'memory-core-card')
    await expect(card).toBeVisible({ timeout: 20000 })

    // Pick the seeded assistant — the blocks editor renders for it.
    await byTestId(page, 'memory-core-assistant-combobox').click()
    await byTestId(
      page,
      `memory-core-assistant-combobox-opt-${assistant.id}`,
    ).click()

    // Add a core-memory block via the editor modal.
    await byTestId(page, 'memory-core-add-block-btn').click()
    const modal = byTestId(page, 'memory-core-block-create-dialog')
    await expect(modal).toBeVisible()
    await byTestId(modal, 'memory-core-block-label-input').fill('persona')
    await byTestId(modal, 'memory-core-block-content-input').fill(
      'Always answer concisely.',
    )
    await byTestId(modal, 'memory-core-block-form-submit-btn').click()

    // The new block lists under the assistant's editor ("persona" is the
    // dynamic label we just created).
    const blockCard = page.locator('[data-testid^="memory-core-block-card-"]')
    await expect(blockCard).toHaveCount(1)
    await expect(blockCard).toContainText('persona')

    // Delete it via the per-row Confirm dialog.
    await blockCard
      .locator('[data-testid^="memory-core-block-delete-btn-"]')
      .click()
    await page
      .locator(
        '[data-testid^="memory-core-block-delete-confirm-"][data-testid$="-confirm"]',
      )
      .click()
    await expect(blockCard).toHaveCount(0)
  })
})
