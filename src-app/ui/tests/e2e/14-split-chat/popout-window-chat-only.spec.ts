import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — desktop pop-out renders the CHAT INTERFACE ONLY (TEST-79,
 * ITEM-52, FB-12). The desktop pop-out opens a dedicated `/chat-window/:id` route
 * whose route has NO layout (blank), so it renders the conversation (header +
 * messages + composer) with the app SHELL absent — no `app-sidebar`, no
 * sidebar-toggle / nav. This RUNS the real render in a real browser and asserts
 * the DOM (not a code-read): the control `/chat/:id` route DOES show the shell;
 * the pop-out route does NOT. (Web still opens `/chat/:id` for pop-out; this route
 * is what the desktop `WebviewWindow` loads — see TEST-75.)
 */
test.describe('Split chat — pop-out window renders chat-only (blank layout)', () => {
  test.describe.configure({ retries: 1 })

  test('the /chat-window/:id route renders the conversation with NO app shell', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(90000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }
    const conv = await (
      await page.request.post(`${apiURL}/api/conversations`, {
        headers: auth,
        data: { title: 'Popout Render' },
      })
    ).json()

    // CONTROL: the normal /chat/:id route DOES render the app shell (sidebar).
    await page.goto(`${baseURL}/chat/${conv.id}`)
    await expect(byTestId(page, 'app-sidebar').first()).toBeVisible({ timeout: 15000 })
    await expect(
      page.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 15000 })

    // POP-OUT ROUTE: renders the conversation interface, but the app shell is ABSENT.
    await page.goto(`${baseURL}/chat-window/${conv.id}`)
    // The chat interface itself is present (the composer + the conversation title).
    await expect(
      page.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'conversation-title').first()).toBeVisible({
      timeout: 15000,
    })
    // The app SHELL is gone — no sidebar container, no sidebar toggle / nav.
    await expect(byTestId(page, 'app-sidebar')).toHaveCount(0)
    await expect(byTestId(page, 'layout-sidebar-toggle-button')).toHaveCount(0)
    await expect(byTestId(page, 'layout-sidebar-nav-menu')).toHaveCount(0)
  })
})
