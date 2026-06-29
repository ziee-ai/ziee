import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
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
 * pill's mode reflects the chosen value.
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

    // The pill defaults to 'inherit' (auto). It carries data-mode for
    // i18n-safe state assertions.
    const pill = byTestId(page, 'memory-status-pill')
    await expect(pill).toBeVisible({ timeout: 30000 })
    await expect(pill).toHaveAttribute('data-mode', 'inherit')

    // Open the dropdown and pick "Always retrieve memories" → mode = on.
    await pill.click()
    await byTestId(page, 'memory-status-pill-dropdown-item-on').click()

    // The pill relabels to mode = on.
    await expect(pill).toHaveAttribute('data-mode', 'on')

    // Server persisted the choice.
    const after = await page.request.get(
      `${apiURL}/api/conversations/${conversationId}/memory-mode`,
      { headers: authHeader },
    )
    expect((await after.json()).memory_mode).toBe('on')
  })
})
