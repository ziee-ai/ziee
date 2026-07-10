import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile } from '../file/helpers'

// TEST-25 (ITEM-15) / TEST-26 (ITEM-16): selecting text in the markdown canvas
// raises the selection popover with "Ask about this" (quotes the excerpt into the
// composer, non-mutating) and "Edit this section" (scoped edit). The message
// SHAPING is unit-tested (selectionEdit.test.ts); this spec drives the real
// popover UI on the canvas.
test.describe('Artifacts — selection popover (ask / edit)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('selecting text raises the ask/edit popover and fires the ask action', async ({
    page,
    testInfra,
  }) => {
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Select ${Date.now()}`,
      filename: 'prose.md',
      content: 'The unique selectable sentence lives here alone.\n',
      mime: 'text/markdown',
    })

    await page.goto(`${testInfra.baseURL}/files/${fileId}`)
    await page.getByTestId('canvas-edit-toggle').click()
    const editable = page
      .getByTestId('canvas-edit-body')
      .locator('[contenteditable="true"]')
      .first()
    await expect(editable).toBeVisible()

    // Select the whole first paragraph (triple-click), then verify the popover.
    const para = editable.locator('p, div').first()
    await para.click({ clickCount: 3 })

    const popover = page.getByTestId('canvas-selection-popover')
    await expect(popover).toBeVisible({ timeout: 10000 })
    await expect(page.getByTestId('canvas-selection-ask')).toBeVisible()
    await expect(page.getByTestId('canvas-selection-edit')).toBeVisible()

    // "Ask about this" fires the handler (builds the quoted-excerpt message +
    // injects it into the composer); the confirmation toast proves the action ran.
    await page.getByTestId('canvas-selection-ask').click()
    await expect(page.getByText(/added the selection to the chat/i).first()).toBeVisible({
      timeout: 10000,
    })
  })
})
