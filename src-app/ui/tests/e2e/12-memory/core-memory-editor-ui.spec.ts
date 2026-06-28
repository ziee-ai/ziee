import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the CoreMemorySection assistant picker + CoreMemoryBlocksEditor UI.
 *
 * Audit gap (all-d3edff7f208e): core-memory-persona.spec.ts only drives the
 * REST validation path (PUT /api/assistants/core-memory → 400 for a bad
 * slug). Neither CoreMemorySection.tsx (the "Pick an assistant" Select) nor
 * CoreMemoryBlocksEditor.tsx (the add/edit-block modal) had any UI test. This
 * drives the real page: pick an assistant, add a block through the modal,
 * assert the PUT fires, and assert the block persists across a reload.
 */

test.describe('Memory — core memory blocks editor UI', () => {
  test('add a block through the editor and it persists', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // A real assistant for the picker to select (admin holds memory::core::*
    // via the wildcard).
    const tag = Date.now().toString(36)
    const assistantName = `CoreMem Assistant ${tag}`
    const created = await fetch(`${apiURL}/api/assistants`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      body: JSON.stringify({
        name: assistantName,
        instructions: 'You are a test assistant.',
      }),
    })
    expect(created.status, 'create assistant').toBeLessThan(300)

    await page.goto(`${baseURL}/settings/memory`)

    // The "Per-assistant core memory" card holds the assistant picker.
    const picker = page.getByLabel('Pick an assistant')
    await expect(picker).toBeVisible({ timeout: 15000 })
    await picker.click()
    await page
      .locator('.ant-select-dropdown:visible')
      .getByText(assistantName, { exact: true })
      .click()

    // Picking the assistant mounts CoreMemoryBlocksEditor → "Add block".
    await page.getByRole('button', { name: 'Add block' }).click()

    const modal = page.getByRole('dialog')
    await expect(modal).toBeVisible({ timeout: 5000 })
    const blockLabel = `persona_${tag}`
    await modal.getByPlaceholder('persona').fill(blockLabel)
    await modal
      .getByPlaceholder(/Always-in-context content/i)
      .fill('Always greet the user by name and stay concise.')

    // The OK button ("Add") fires PUT /api/assistants/core-memory.
    const putReq = page.waitForRequest(
      req =>
        req.method() === 'PUT' &&
        req.url().includes('/api/assistants/core-memory'),
      { timeout: 10000 },
    )
    await modal.getByRole('button', { name: 'Add' }).click()
    await putReq

    // The new block renders in the editor's list.
    await expect(page.getByText(blockLabel)).toBeVisible({ timeout: 10000 })

    // Persistence: reload, re-pick the assistant, the block is still listed
    // (proves it was written server-side, not just optimistic state).
    await page.reload()
    const picker2 = page.getByLabel('Pick an assistant')
    await expect(picker2).toBeVisible({ timeout: 15000 })
    await picker2.click()
    await page
      .locator('.ant-select-dropdown:visible')
      .getByText(assistantName, { exact: true })
      .click()
    await expect(page.getByText(blockLabel)).toBeVisible({ timeout: 10000 })
  })
})
