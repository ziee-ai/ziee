import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the sidebar resize handle changes the sidebar width.
 *
 * Audit gap: AppLayout's drag-resize (handleMouseDown / ResizeHandle) was
 * untested. The handle (`role="separator" aria-label="Resize"`) also supports
 * keyboard actuation (Arrow keys nudge the parent width, mirroring the drag
 * path) — a deterministic way to exercise the same resize→width pipeline.
 */

async function sidebarWidth(page: import('@playwright/test').Page) {
  const box = await page.locator('#app-sidebar').boundingBox()
  if (!box) throw new Error('no #app-sidebar box')
  return box.width
}

test.describe('Layout — sidebar resize', () => {
  test('keyboard-actuating the resize handle changes the sidebar width', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/profile`)
    await expect(
      page.getByRole('heading', { name: 'Profile' }),
    ).toBeVisible({ timeout: 30000 })

    const handle = page.getByRole('separator', { name: 'Resize' }).first()
    await expect(handle).toBeVisible()

    const before = await sidebarWidth(page)

    // Nudge wider; if this handle's orientation shrinks on ArrowRight, the
    // opposite arrow grows — either way the width must MOVE.
    await handle.focus()
    for (let i = 0; i < 3; i++) await handle.press('ArrowRight')
    let after = await sidebarWidth(page)
    if (Math.abs(after - before) < 1) {
      for (let i = 0; i < 3; i++) await handle.press('ArrowLeft')
      after = await sidebarWidth(page)
    }

    expect(Math.abs(after - before)).toBeGreaterThan(1)
  })
})
