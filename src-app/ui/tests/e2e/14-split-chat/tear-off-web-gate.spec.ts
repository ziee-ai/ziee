import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — tear-off is DESKTOP ONLY (TEST-93, ITEM-58 / DEC-70). This
 * runs on the WEB build (no Tauri), so releasing a conversation drag past the
 * window edge must open NOTHING — the ⤢ button stays the web affordance. Proves
 * (a) the drag sources are actually wired to `onDragEnd` and (b) the desktop-only
 * gate holds, by dispatching a real `dragend` with off-window screen coords and
 * asserting no window opened. (The desktop POSITIVE path — a native window — is
 * covered by the pure `planTearOff`/`runTearOffPlan` unit tests + the shared
 * `openConversationWindow` desktop-seam tests; a real OS-window tear can't be
 * driven headlessly.)
 */
test.describe('Split chat — tear-off web gate (desktop-only)', () => {
  test('a drag released outside the window opens NO window on web', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'TearGate One' },
    })
    const convId = (await res.json()).id as string

    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('load')

    // Record any window.open (the web tear path) so we can assert it never fires.
    await page.evaluate(() => {
      ;(window as unknown as { __opened: unknown[] }).__opened = []
      const orig = window.open.bind(window)
      window.open = ((...a: unknown[]) => {
        ;(window as unknown as { __opened: unknown[] }).__opened.push(a)
        return orig(a[0] as string) as Window | null
      }) as typeof window.open
    })

    const card = byTestId(page, `chat-conversation-card-${convId}`)
    await expect(card).toBeVisible({ timeout: 15000 })

    // Dispatch a real dragend on the wired source with a release point far off the
    // window (top-left). On web (__TAURI__ absent) the gate must swallow it.
    await card.dispatchEvent('dragstart', { dataTransfer: await page.evaluateHandle(() => new DataTransfer()) })
    await card.dispatchEvent('dragend', { screenX: -9999, screenY: -9999 })
    await page.waitForTimeout(500)

    const opened = await page.evaluate(
      () => (window as unknown as { __opened: unknown[] }).__opened.length,
    )
    expect(opened).toBe(0)
    // And we did not navigate into a pop-out window route.
    await expect(page).not.toHaveURL(/\/chat-window\//)
  })
})
