import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — CoreMemorySection assistant-picker → editor combo flow
 * (audit id all-ae04dc8511a3). The section (on /settings/memory) lets the user
 * pick an assistant from a Select, then renders CoreMemoryBlocksEditor for it.
 * Untested. Create an assistant, pick it, assert the per-assistant editor mounts.
 */

test.describe('Memory — per-assistant core memory section', () => {
  test.describe.configure({ retries: 2 })

  test('picking an assistant reveals its core-memory editor', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Create an assistant so the picker has an option.
    const aName = `Core Memory Bot ${Date.now()}`
    const res = await fetch(`${apiURL}/api/assistants`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ name: aName, is_template: false, enabled: true }),
    })
    if (!res.ok) throw new Error(`assistant create failed: ${res.status} ${await res.text()}`)

    await page.goto(`${baseURL}/settings/memory`)
    await page.waitForLoadState('load')

    // The per-assistant core-memory card + its picker render.
    await expect(page.getByText('Per-assistant core memory')).toBeVisible({
      timeout: 30000,
    })
    const picker = page.getByLabel('Pick an assistant')
    await expect(picker).toBeVisible({ timeout: 15000 })

    // Open the Select and choose the created assistant.
    await picker.click()
    await page.getByRole('option', { name: aName }).click()

    // The editor for that assistant mounts: an empty assistant shows the
    // "Add block" affordance + the "No blocks yet" empty state.
    await expect(page.getByRole('button', { name: 'Add block' })).toBeVisible({
      timeout: 10000,
    })
    await expect(page.getByText('No blocks yet')).toBeVisible()
  })
})
