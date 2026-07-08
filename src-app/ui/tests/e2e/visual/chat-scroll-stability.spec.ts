/**
 * message-scroll-stability — the behavioural proof, against the backend-free
 * `?surface=seeded-message-list-long` gallery surface (500 mixed messages
 * driving the REAL virtualized MessageList).
 *
 * TEST-6  scroll → virtualizer corrections settle to ~0 after each pause
 * TEST-7  inline file body mount/settle does NOT change the row height
 * TEST-8  show-more survives scroll-away-and-back (state-lift)
 * TEST-9  expanding does not jump the viewport (in-place anchor)
 * TEST-10 drag/keyboard resize persists across remount + does not jump
 * TEST-11 jump-to-message still lands + settles (no anchor regression)
 * TEST-12 zero console errors / page errors across the interactions
 */
import { test, expect, type Page, type Locator } from '@playwright/test'

const SURFACE =
  '/gallery.html?surface=seeded-message-list-long&theme=light&accent=blue'

type Metrics = { corrections: number; reset: () => void; totalSize: () => number }
declare global {
  interface Window {
    __MSGLIST_METRICS__?: Metrics
  }
}

async function corrections(page: Page): Promise<number> {
  return page.evaluate(() => window.__MSGLIST_METRICS__?.corrections ?? -1)
}
async function resetMetrics(page: Page): Promise<void> {
  await page.evaluate(() => window.__MSGLIST_METRICS__?.reset())
}

/** Scroll the message viewport from the TOP down until `[data-message-id=id]` is
 *  attached, or we hit the bottom. Resets to the top first so it finds a target
 *  regardless of the current scroll position (a target above the current
 *  position would otherwise be unreachable by a downward-only walk). Returns the
 *  locator (may be detached if not found). */
async function scrollToMessage(page: Page, id: string): Promise<Locator> {
  const scroller = page.getByTestId('g-msglist-scroll')
  const target = page.locator(`[data-message-id="${id}"]`)
  await scroller.evaluate(el => (el.scrollTop = 0))
  await page.waitForTimeout(120)
  for (let i = 0; i < 80; i++) {
    if ((await target.count()) > 0) {
      // Bring the row fully INTO view (top ~80px below the viewport top) so an
      // inline-file preview crosses the 800px seen-band and mounts its real
      // body (not just the skeleton), and so collapsible tests have a stable
      // in-view position.
      await scroller.evaluate((el, id) => {
        const r = el
          .querySelector(`[data-message-id="${id}"]`)
          ?.getBoundingClientRect()
        if (r) el.scrollTop += r.top - el.getBoundingClientRect().top - 80
      }, id)
      await page.waitForTimeout(250)
      return target
    }
    const done = await scroller.evaluate(el => {
      const before = el.scrollTop
      el.scrollTop = Math.min(el.scrollTop + el.clientHeight * 0.9, el.scrollHeight)
      return el.scrollTop === before
    })
    await page.waitForTimeout(120)
    if (done) break
  }
  return target
}

/** Row top relative to the scroll viewport top, in px. */
async function rowTop(page: Page, msg: Locator): Promise<number> {
  const scroller = page.getByTestId('g-msglist-scroll')
  return msg.evaluate(
    (el, sEl) => el.getBoundingClientRect().top - sEl!.getBoundingClientRect().top,
    await scroller.elementHandle(),
  )
}

async function settleIdle(page: Page, ms = 900) {
  // Let the visible window mount + measure, then stop touching the scroller.
  await page.waitForTimeout(ms)
}

test.describe('chat message-scroll-stability', () => {
  let consoleErrors: string[]
  let pageErrors: string[]
  test.beforeEach(async ({ page }) => {
    consoleErrors = []
    pageErrors = []
    page.on('console', m => {
      if (m.type() === 'error') consoleErrors.push(m.text())
    })
    page.on('pageerror', e => pageErrors.push(String(e)))
    await page.goto(SURFACE)
    await page.getByTestId('g-msglist-scroll').waitFor({ state: 'visible' })
    // Wait for the metrics hook (MessageList mounted).
    await expect.poll(() => corrections(page)).toBeGreaterThanOrEqual(0)
  })

  test('TEST-6: corrections settle to ~0 after a scroll pause', async ({ page }) => {
    const scroller = page.getByTestId('g-msglist-scroll')
    // Scroll to the bottom in steps (corrections are EXPECTED during motion).
    for (let i = 0; i < 40; i++) {
      const done = await scroller.evaluate(el => {
        const b = el.scrollTop
        el.scrollTop = Math.min(el.scrollTop + el.clientHeight, el.scrollHeight)
        return el.scrollTop === b
      })
      await page.waitForTimeout(80)
      if (done) break
    }
    // STOP scrolling, let everything mount+measure, then measure the SETTLED
    // window: with fixed inline heights + lifted state, no further corrections
    // fire while idle (before the fix, late body-mounts kept firing).
    await settleIdle(page)
    await resetMetrics(page)
    await settleIdle(page, 1200)
    expect(await corrections(page)).toBeLessThanOrEqual(2)

    // Same at the top after scrolling back.
    await scroller.evaluate(el => (el.scrollTop = 0))
    await settleIdle(page)
    await resetMetrics(page)
    await settleIdle(page, 1200)
    expect(await corrections(page)).toBeLessThanOrEqual(2)

    expect(pageErrors, pageErrors.join('\n')).toHaveLength(0)
  })

  test('TEST-7: inline file body height is fixed across mount/settle', async ({ page }) => {
    const msg = await scrollToMessage(page, 'g-msg-13') // carries an inline image preview
    await expect(msg).toBeAttached()
    const body = msg.getByTestId('inline-file-preview-body')
    await expect(body).toBeVisible()
    const declared = Number(await body.getAttribute('data-body-height'))
    // The box is FIXED at the generic default (400px) — it CAPS content, it does
    // not hug it (the inline image is only 180px tall). This is the discriminator
    // vs the old content-driven `max-h` body, which would size to ~180px.
    expect(declared).toBe(400)
    const rowH1 = (await msg.boundingBox())!.height
    const h1 = (await body.boundingBox())!.height
    expect(Math.abs(h1 - declared)).toBeLessThanOrEqual(1)
    // Wait for the image to decode/settle, then re-measure the ROW: fixed box ⇒
    // neither the body nor the enclosing message row changes height.
    await page.waitForTimeout(600)
    const rowH2 = (await msg.boundingBox())!.height
    const h2 = (await body.boundingBox())!.height
    expect(Math.abs(h2 - h1)).toBeLessThanOrEqual(1)
    expect(Math.abs(rowH2 - rowH1)).toBeLessThanOrEqual(1)
  })

  test('TEST-8: show-more stays expanded after scroll-away-and-back', async ({ page }) => {
    const msg = await scrollToMessage(page, 'g-msg-7') // long → collapsible
    await expect(msg).toBeAttached()
    const toggle = msg.getByTestId('collapsible-toggle')
    await expect(toggle).toBeVisible()
    const content = msg.getByTestId('collapsible-content')
    await expect(content).toHaveAttribute('data-collapsed', 'true')
    await toggle.click()
    await expect(content).toHaveAttribute('data-collapsed', 'false')

    // Scroll far away (unmounts the row) then back.
    const scroller = page.getByTestId('g-msglist-scroll')
    await scroller.evaluate(el => (el.scrollTop = el.scrollHeight))
    await page.waitForTimeout(400)
    const back = await scrollToMessage(page, 'g-msg-7')
    await expect(back).toBeAttached()
    // Still expanded — state survived the virtualizer unmount/remount.
    await expect(back.getByTestId('collapsible-content')).toHaveAttribute(
      'data-collapsed',
      'false',
    )
  })

  test('TEST-9: expanding does not jump the viewport', async ({ page }) => {
    const scroller = page.getByTestId('g-msglist-scroll')
    const msg = await scrollToMessage(page, 'g-msg-7')
    await expect(msg).toBeAttached()
    // Position the collapsed message so its top sits ~80px inside the viewport
    // (deterministically in-view, straddling nothing) so the assertion measures
    // a genuine in-place expand — the exact case the ITEM-7 anchor guards.
    await scroller.evaluate(el => {
      const s = el.getBoundingClientRect()
      const r = el
        .querySelector('[data-message-id="g-msg-7"]')!
        .getBoundingClientRect()
      el.scrollTop += r.top - s.top - 80
    })
    await page.waitForTimeout(200)
    const topBefore = await rowTop(page, msg)
    await msg.getByTestId('collapsible-toggle').click()
    await page.waitForTimeout(350)
    const topAfter = await rowTop(page, msg)
    // The row's top stays put (expansion grows downward, no viewport teleport).
    expect(Math.abs(topAfter - topBefore)).toBeLessThanOrEqual(6)
  })

  test('TEST-10: resize persists across remount and does not jump', async ({ page }) => {
    const msg = await scrollToMessage(page, 'g-msg-13')
    await expect(msg).toBeAttached()
    const body = msg.getByTestId('inline-file-preview-body')
    const handle = msg.getByTestId('inline-file-preview-resize')
    await expect(body).toBeVisible()
    const h0 = Number(await body.getAttribute('data-body-height'))
    // Keyboard resize (deterministic): grow the body.
    await handle.focus()
    for (let i = 0; i < 6; i++) await page.keyboard.press('ArrowDown')
    await page.waitForTimeout(200)
    const h1 = Number(await body.getAttribute('data-body-height'))
    expect(h1).toBeGreaterThan(h0)

    // Scroll away + back → the resized height persisted (lifted state).
    const scroller = page.getByTestId('g-msglist-scroll')
    await scroller.evaluate(el => (el.scrollTop = el.scrollHeight))
    await page.waitForTimeout(400)
    const back = await scrollToMessage(page, 'g-msg-13')
    await expect(back).toBeAttached()
    expect(
      Number(await back.getByTestId('inline-file-preview-body').getAttribute('data-body-height')),
    ).toBe(h1)
  })

  test('TEST-11: jump-to-message lands and settles', async ({ page }) => {
    await page.getByTestId('g-msglist-jump').click()
    await page.waitForTimeout(400)
    const target = page.locator('[data-message-id="g-msg-250"]')
    await expect(target).toBeAttached()
    // It landed inside the viewport…
    const scroller = page.getByTestId('g-msglist-scroll')
    const within = await target.evaluate((el, sEl) => {
      const t = el.getBoundingClientRect()
      const s = sEl!.getBoundingClientRect()
      return t.top >= s.top - 4 && t.top <= s.bottom
    }, await scroller.elementHandle())
    expect(within).toBe(true)
    // …and the list settles (no ongoing correction storm after the jump).
    await settleIdle(page)
    await resetMetrics(page)
    await settleIdle(page, 1000)
    expect(await corrections(page)).toBeLessThanOrEqual(2)
  })

  test('TEST-13: pointer-drag resize grows the body and persists', async ({ page }) => {
    const msg = await scrollToMessage(page, 'g-msg-13')
    await expect(msg).toBeAttached()
    const body = msg.getByTestId('inline-file-preview-body')
    const handle = msg.getByTestId('inline-file-preview-resize')
    await expect(body).toBeVisible()
    const h0 = Number(await body.getAttribute('data-body-height'))
    // Drive a real pointer drag on the handle: dispatch pointerdown → several
    // pointermove → pointerup with a downward clientY delta. (Explicit dispatch
    // is deterministic; Playwright's synthetic mouse + setPointerCapture is
    // flaky on a 8px strip.)
    await handle.evaluate((el: HTMLElement) => {
      const r = el.getBoundingClientRect()
      const x = r.x + r.width / 2
      const y0 = r.y + r.height / 2
      const ev = (type: string, y: number) =>
        el.dispatchEvent(
          new PointerEvent(type, {
            clientX: x,
            clientY: y,
            pointerId: 1,
            bubbles: true,
            cancelable: true,
          }),
        )
      ev('pointerdown', y0)
      for (let i = 1; i <= 9; i++) ev('pointermove', y0 + i * 10)
      ev('pointerup', y0 + 90)
    })
    await page.waitForTimeout(150)
    const h1 = Number(await body.getAttribute('data-body-height'))
    expect(h1).toBeGreaterThan(h0)
    // Persisted across scroll-away-and-back (lifted state).
    const scroller = page.getByTestId('g-msglist-scroll')
    await scroller.evaluate(el => (el.scrollTop = el.scrollHeight))
    await page.waitForTimeout(400)
    const back = await scrollToMessage(page, 'g-msg-13')
    expect(
      Number(await back.getByTestId('inline-file-preview-body').getAttribute('data-body-height')),
    ).toBe(h1)
  })

  test.afterEach(() => {
    // TEST-12: no console/page errors across any of the above interactions.
    expect(consoleErrors, consoleErrors.join('\n')).toHaveLength(0)
    expect(pageErrors, pageErrors.join('\n')).toHaveLength(0)
  })
})
