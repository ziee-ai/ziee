import { test, expect } from '../../fixtures/test-context'
import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — small-screen pane manager (TEST-114/115/116, ITEM-79..83 / FB-26).
 *
 * Below the `md` breakpoint there is no room to tile columns. The old always-visible
 * tab strip + drag chrome are GONE; instead:
 *   • a focused split pane reads as a NORMAL single-pane conversation — no reorder
 *     grip, no per-pane ✕, normal header inset (the sidebar toggle is never clipped);
 *   • the header "Panes" button opens the `PaneManagerDrawer` (NOT a direct split);
 *   • the drawer lists the open panes (tap to focus, ✕ to close) and opens ANOTHER
 *     conversation into a new pane.
 * All panes stay MOUNTED (one visible), so a background pane keeps streaming. No LLM.
 */
test.describe('Split chat — small-screen pane manager', () => {
  test.use({ viewport: { width: 390, height: 844 } })

  const mkConv = async (
    page: Page,
    apiURL: string,
    token: string,
    title: string,
  ): Promise<string> => {
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title },
    })
    expect(res.status()).toBeLessThan(300)
    return (await res.json()).id as string
  }

  // Build the [A | B] split via the drawer flow (the ONLY way to split on mobile),
  // starting from single-pane conv A (already the current route) and leaving pane 1
  // (conv B) focused/visible.
  const openSplitAB = async (page: Page, convB: string) => {
    // Single-pane: the header "Panes" button opens the manager (does NOT split).
    await byTestId(page, 'chat-split-btn').click()
    const drawer = byTestId(page, 'pane-manager-drawer')
    await expect(drawer).toBeVisible({ timeout: 15000 })
    // "Open another" → conv B → creates the split [A | B] and focuses B.
    await drawer.getByTestId(`pane-manager-open-${convB}`).click()
    await expect(drawer).toBeHidden({ timeout: 15000 })
    await expect(byTestId(page, 'split-chat-view')).toBeVisible({ timeout: 15000 })
  }

  test('the Panes button opens the manager drawer; opening another conversation splits with normal single-pane chrome (no tab strip / grip / per-pane ✕)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Mobile Alpha')
    const convB = await mkConv(page, apiURL, token, 'Mobile Bravo')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    // Tapping the Panes button opens the DRAWER (not a split) and lists the current
    // conversation under "Open panes".
    await byTestId(page, 'chat-split-btn').click()
    const drawer = byTestId(page, 'pane-manager-drawer')
    await expect(drawer).toBeVisible({ timeout: 15000 })
    await expect(drawer.getByTestId('pane-manager-focus-current')).toContainText('Alpha')
    // No split has been created yet.
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0)

    // Open conv B into a new pane.
    await drawer.getByTestId(`pane-manager-open-${convB}`).click()
    await expect(drawer).toBeHidden({ timeout: 15000 })

    // Now a split exists, in single-visible-pane mode — but with NONE of the old
    // mobile chrome.
    const view = byTestId(page, 'split-chat-view')
    await expect(view).toBeVisible({ timeout: 15000 })
    await expect(view).toHaveAttribute('data-split-mode', 'tabs')
    await expect(byTestId(page, 'pane-tab-strip')).toHaveCount(0) // no tab strip
    await expect(byTestId(page, 'chat-pane-grip')).toHaveCount(0) // no reorder grip
    await expect(byTestId(page, 'chat-pane-close')).toHaveCount(0) // no per-pane ✕

    // Exactly ONE pane visible (the focused conv B); pane 0 (A) mounted-but-hidden.
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-0')).toBeHidden()
    await expect(
      byTestId(page, 'chat-pane-1').getByTestId('conversation-title'),
    ).toContainText('Bravo', { timeout: 15000 })
  })

  test('the manager drawer switches the visible pane and closes a pane', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Switch Alpha')
    const convB = await mkConv(page, apiURL, token, 'Switch Bravo')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await openSplitAB(page, convB) // [A | B], B focused/visible

    // Open the manager from the (visible) focused pane header → it lists BOTH panes.
    await byTestId(page, 'chat-pane-1').getByTestId('chat-split-btn').click()
    let drawer = byTestId(page, 'pane-manager-drawer')
    await expect(drawer).toBeVisible({ timeout: 15000 })
    const openList = drawer.getByTestId('pane-manager-open-list')
    await expect(openList.locator('> li')).toHaveCount(2)

    // Tap conv A's row → focuses pane 0 (A becomes the visible pane), drawer closes.
    await openList.getByRole('button', { name: 'Switch Alpha' }).click()
    await expect(drawer).toBeHidden({ timeout: 15000 })
    await expect(byTestId(page, 'chat-pane-0')).toBeVisible()
    await expect(byTestId(page, 'chat-pane-1')).toBeHidden()

    // Reopen the manager (now from pane 0's header) and CLOSE conv B's pane via its ✕.
    await byTestId(page, 'chat-pane-0').getByTestId('chat-split-btn').click()
    drawer = byTestId(page, 'pane-manager-drawer')
    await expect(drawer).toBeVisible({ timeout: 15000 })
    await drawer
      .getByTestId('pane-manager-open-list')
      .locator('li')
      .filter({ hasText: 'Switch Bravo' })
      .getByRole('button', { name: 'Close pane' })
      .click()

    // The workspace collapses to a single pane (A) — split view is gone.
    await expect(byTestId(page, 'split-chat-view')).toHaveCount(0, { timeout: 15000 })
    await expect(byTestId(page, 'conversation-title')).toContainText('Alpha', { timeout: 15000 })
  })

  // TEST-118 (ITEM-84 / FB-28): the focused mobile split pane behaves like a normal
  // single-pane conversation — native document-scroll + an auto-hiding header (slides
  // up on scroll-down, reveals on scroll-up). Also a regression guard: the header is
  // driven by an unconditional hook + an unconditional store read, so building the
  // split (which flips the focused pane's scroll mode) must not crash.
  test('the focused mobile pane has native scroll + an auto-hiding header', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Scroll Alpha')
    const convB = await mkConv(page, apiURL, token, 'Scroll Bravo')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await openSplitAB(page, convB) // [A | B], B focused/visible

    // The focused mobile pane renders the SAME HeaderBarContainer as single-pane
    // (`app-header-bar`), not the compact `chat-pane-header` div — so it inherits the
    // real auto-hide chrome (sticky top:5 + safe-area backdrop + relative-wipe).
    const header = byTestId(page, 'chat-pane-1').getByTestId('app-header-bar')
    await expect(header).toBeVisible()
    expect(
      await byTestId(page, 'chat-pane-1').getByTestId('chat-pane-header').count(),
    ).toBe(0) // the compact header is NOT used for the focused mobile pane
    // Native document-scroll is active (the auto-hide precondition).
    expect(
      await page.evaluate(() =>
        document.documentElement.classList.contains('scroll-native'),
      ),
    ).toBe(true)

    // Make the document scrollable (empty conversation isn't) by injecting a tall
    // spacer into the visible pane, then scroll and assert the header hides/reveals.
    await page.evaluate(() => {
      const pane = document.querySelector('[data-testid="chat-pane-1"]')
      const spacer = document.createElement('div')
      spacer.style.height = '2000px'
      pane?.appendChild(spacer)
    })
    expect(
      await page.evaluate(
        () => document.documentElement.scrollHeight > window.innerHeight + 100,
      ),
    ).toBe(true)

    await page.evaluate(() => window.scrollTo(0, 400))
    await expect
      .poll(async () => (await header.boundingBox())?.y ?? 0, { timeout: 5000 })
      .toBeLessThan(0) // slid up out of view on scroll-down

    // Scroll back to the TOP: the "at top" path always reveals instantly (no
    // direction-toggle debounce), so this is deterministic regardless of timing.
    await page.evaluate(() => window.scrollTo(0, 0))
    await expect
      .poll(async () => (await header.boundingBox())?.y ?? -1, { timeout: 5000 })
      .toBeGreaterThanOrEqual(0) // header revealed at the top
  })
})
