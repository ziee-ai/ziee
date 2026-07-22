import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the left sidebar's three section captions share ONE left edge.
 *
 * The bug: "Navigation" and "Tools" sat further right than "Recent chats",
 * breaking the vertical scan line down the rail. They were produced two
 * different ways, so their insets never had a reason to agree:
 *   - Navigation / Tools were kit `Menu` GROUP titles, inset by the Menu
 *     wrapper's `px-2` (8px) PLUS the kit's own hardcoded group-title `px-3`
 *     (12px) = 20px;
 *   - Recent chats was a hand-rolled div at a flat `px-3` = 12px.
 * The fix routes all three through one `SidebarSectionTitle`, and drops the kit
 * group wrapper so the padding no longer stacks.
 *
 * Why a full e2e rather than a gallery visual spec: the gallery's `PageFrame`
 * renders a route's element WITHOUT its layout, so the real `LeftSidebar` — and
 * the module slots that populate Navigation / Tools / the Recent-chats widget —
 * never render there. Measuring the real shell is the only way this assertion
 * means anything (B7).
 *
 * These specs assert POSITION, not classes: any equivalent re-implementation
 * should keep them green, and a class assertion would not have caught the
 * original bug (both class strings looked reasonable in isolation).
 */

/** Sub-pixel tolerance — these are computed layout positions, not integers. */
const EPS = 0.5

interface Edges {
  captions: { name: string; left: number }[]
  rows: { name: string; left: number }[]
}

/**
 * Left edges of every section caption and every menu ROW in the sidebar.
 *
 * Rows are measured from the row BUTTON's text-content edge (its border box +
 * its own left padding), which is what the eye actually aligns on — not the
 * full-width highlight pill, whose edge is the rail's padding and would report
 * the same number no matter what the rows did.
 */
async function readEdges(page: import('@playwright/test').Page): Promise<Edges> {
  return page.evaluate(() => {
    const sidebar = document.querySelector<HTMLElement>('#app-sidebar')
    if (!sidebar) throw new Error('no #app-sidebar in the DOM')

    const px = (v: string) => parseFloat(v) || 0
    const contentLeft = (el: Element): number => {
      const s = getComputedStyle(el)
      return +(
        el.getBoundingClientRect().left +
        px(s.borderLeftWidth) +
        px(s.paddingLeft)
      ).toFixed(2)
    }

    const captionOf = (testid: string) => {
      const el = sidebar.querySelector(`[data-testid="${testid}"]`)
      return el ? { name: testid, left: contentLeft(el) } : null
    }
    const captions = [
      captionOf('layout-sidebar-nav-title'),
      captionOf('layout-sidebar-tools-title'),
      captionOf('chat-recent-title'),
    ].filter((c): c is { name: string; left: number } => c !== null)

    // Every menu row button across all four sections (primary actions,
    // navigation, tools, and the virtualized recent-chat rows).
    const rows = Array.from(
      sidebar.querySelectorAll<HTMLElement>('nav ul > li > button, [data-testid^="chat-recent-conversations-menu-item-"]'),
    ).map(el => ({
      name: el.getAttribute('data-testid') ?? el.textContent?.trim() ?? 'row',
      left: contentLeft(el),
    }))

    return { captions, rows }
  })
}

test.describe('App layout — sidebar section title alignment', () => {
  test('TEST-6: Navigation, Tools and Recent chats share one left edge', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await expect(page.getByTestId('layout-sidebar-nav-title')).toBeVisible({
      timeout: 30000,
    })
    // The recent-chats widget renders its caption in every state (loading /
    // empty / error / loaded), so no seeded conversation is needed — but wait
    // for it so we never measure a frame where the widget hasn't mounted.
    await expect(page.getByTestId('chat-recent-title')).toBeVisible({
      timeout: 30000,
    })

    const { captions } = await readEdges(page)

    // Guard: all three must be present, or "they agree" is vacuous.
    expect(
      captions.map(c => c.name).sort(),
      'not all three sidebar captions rendered — the comparison would be vacuous',
    ).toEqual([
      'chat-recent-title',
      'layout-sidebar-nav-title',
      'layout-sidebar-tools-title',
    ])

    const [first, ...rest] = captions
    for (const c of rest) {
      expect(
        Math.abs(c.left - first.left),
        `"${c.name}" is at ${c.left}px but "${first.name}" is at ${first.left}px — ` +
          `the sidebar captions do not share a left edge`,
      ).toBeLessThanOrEqual(EPS)
    }
  })

  test('TEST-7: the menu rows keep their own shared edge, outdented from the captions', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await expect(page.getByTestId('layout-sidebar-nav-title')).toBeVisible({
      timeout: 30000,
    })

    const { captions, rows } = await readEdges(page)

    // This is the anti-cheat control for TEST-6: the captions could trivially be
    // "aligned" by dragging the ROWS around instead. Pin the rows to one shared
    // edge of their own so that fails.
    expect(
      rows.length,
      'no sidebar menu rows found — this control would prove nothing',
    ).toBeGreaterThanOrEqual(2)
    const rowLeft = rows[0].left
    for (const r of rows) {
      expect(
        Math.abs(r.left - rowLeft),
        `sidebar row "${r.name}" is at ${r.left}px but the first row is at ` +
          `${rowLeft}px — the rows no longer share one edge`,
      ).toBeLessThanOrEqual(EPS)
    }

    // And pin the intended RELATIONSHIP: captions hang to the left of their
    // rows (the arrangement "Recent chats" already had). Asserting strict
    // inequality also catches the opposite over-correction — pushing the
    // captions right to meet the rows instead of moving them left.
    expect(
      captions[0].left,
      `the captions (${captions[0].left}px) are not outdented from the rows ` +
        `(${rowLeft}px)`,
    ).toBeLessThan(rowLeft)
  })
})
