import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — the sidebar resize handle changes the sidebar width.
 *
 * Audit gap: AppLayout's drag-resize (handleMouseDown on the
 * `layout-sidebar-resize-handle`) was untested. Dragging the handle pins the
 * sidebar width to the pointer's clientX (clamped to MIN_WIDTH=200 ..
 * MAX_WIDTH=400), so dragging it to a new x must MOVE the rendered width.
 */

async function sidebarWidth(page: import('@playwright/test').Page) {
  const box = await byTestId(page, 'app-sidebar').boundingBox()
  if (!box) throw new Error('no app-sidebar box')
  return box.width
}

test.describe('Layout — sidebar resize', () => {
  test('dragging the resize handle changes the sidebar width', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/profile`)
    await expect(byTestId(page, 'settings-page-title')).toBeVisible({
      timeout: 30000,
    })

    const handle = byTestId(page, 'layout-sidebar-resize-handle')
    await expect(handle).toBeVisible()

    const before = await sidebarWidth(page)
    const box = await handle.boundingBox()
    if (!box) throw new Error('no resize-handle box')

    // Drag the handle to a target clientX that is clearly inside the allowed
    // [200, 400] range AND clearly different from the current width — the
    // sidebar width pins to the pointer's clientX.
    const targetX = before > 300 ? 230 : 360
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    await page.mouse.down()
    await page.mouse.move(targetX, box.y + box.height / 2, { steps: 10 })
    await page.mouse.up()

    const after = await sidebarWidth(page)
    expect(Math.abs(after - before)).toBeGreaterThan(1)
  })
})
