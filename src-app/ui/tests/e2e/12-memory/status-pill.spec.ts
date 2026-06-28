import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — the per-conversation MemoryStatusPill dropdown (composer
 * `toolbar_status` slot, MemoryStatusPill.tsx).
 *
 * The pill only renders when (a) there is an active conversation and
 * (b) memory is not globally disabled by the admin. We enable memory
 * deployment-wide via the admin-settings PUT (no embedding model is
 * required to flip `enabled`), create a real conversation, navigate to
 * it, then drive the pill's dropdown UI end-to-end and assert the
 * pill's label reflects the chosen mode.
 */

test.describe('Memory — composer status pill', () => {
  test('dropdown switches the per-conversation memory mode', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const authHeader = { Authorization: `Bearer ${token}` }

    // Enable memory deployment-wide so the pill is allowed to render.
    const enable = await page.request.put(
      `${apiURL}/api/memory/admin-settings`,
      { headers: authHeader, data: { enabled: true } },
    )
    expect(enable.ok()).toBe(true)

    // Create a real conversation and open it so the pill has a
    // conversation id to bind to.
    const conv = await page.request.post(`${apiURL}/api/conversations`, {
      headers: authHeader,
      data: { title: 'memory-pill-test' },
    })
    const conversationId: string = (await conv.json()).id

    await page.goto(`${baseURL}/chat/${conversationId}`)

    // The pill defaults to 'auto' (inherit). Find it by its aria-label.
    const pill = page.getByRole('button', { name: /Memory mode:/ })
    await expect(pill).toBeVisible({ timeout: 30000 })
    await expect(pill).toHaveText(/Memory: auto/)

    // Open the dropdown and pick "Always retrieve memories" → mode = on.
    await pill.click()
    await page.getByRole('menuitem', { name: 'Always retrieve memories' }).click()

    // Success feedback + the pill relabels to "Memory: on".
    await expect(page.getByText('Memory: on for this conversation')).toBeVisible()
    await expect(
      page.getByRole('button', { name: 'Memory mode: Memory: on' }),
    ).toBeVisible()

    // Server persisted the choice.
    const after = await page.request.get(
      `${apiURL}/api/conversations/${conversationId}/memory-mode`,
      { headers: authHeader },
    )
    expect((await after.json()).memory_mode).toBe('on')
  })
})
