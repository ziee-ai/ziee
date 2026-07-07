import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile, openPreviewDrawer } from './helpers'

// TEST-5 (ITEM-1, ITEM-2, ITEM-3): image zoom / fit-mode / pan.
// A 2×2 red PNG (base64) — small but real, so the backend thumbnails it.
const PNG_BASE64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEUlEQVR4nGP8z8Dwn4EIwDiqEAAlwQMFvA0kCwAAAABJRU5ErkJggg=='

test.describe('File viewer — image zoom', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('zoom controls flip fit-mode and scale the body', async ({ page, testInfra }) => {
    const content = atob(PNG_BASE64)
    await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Img ${Date.now()}`,
      filename: 'pixel.png',
      content,
      mime: 'image/png',
    })
    const drawer = await openPreviewDrawer(page, 'pixel.png')

    // Zoom controls render in the header for the right-panel image viewer.
    const zoomIn = drawer.getByTestId('file-viewer-zoom-in-btn')
    await expect(zoomIn).toBeVisible()
    const fit = drawer.getByTestId('file-viewer-image-fit-segmented')
    await expect(fit).toBeVisible()

    // The body starts in fit mode.
    const body = drawer.getByTestId('image-viewer-body')
    await body.waitFor({ state: 'visible', timeout: 15000 })
    await expect(body).toHaveAttribute('data-view-mode', 'fit')

    // Zooming in switches to actual mode and grows the scale > 1.
    await zoomIn.click()
    // Zoom in several more times so the (tiny) image overflows the container and
    // becomes pannable.
    for (let i = 0; i < 5; i++) await zoomIn.click()
    await expect(body).toHaveAttribute('data-view-mode', 'actual')
    await expect
      .poll(async () =>
        Number(await drawer.getByTestId('image-viewer-body').getAttribute('data-scale')),
      )
      .toBeGreaterThan(1)

    // Drag-to-pan: the applied translate changes after a pointer drag across the
    // zoomed image (exercises pointer capture + overflow geometry + clampTranslate).
    const img = body.locator('img')
    const transformBefore = await img.evaluate(el => getComputedStyle(el).transform)
    const box = await body.boundingBox()
    if (box) {
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
      await page.mouse.down()
      await page.mouse.move(box.x + box.width / 2 - 60, box.y + box.height / 2 - 40, { steps: 8 })
      await page.mouse.up()
    }
    await expect
      .poll(() => img.evaluate(el => getComputedStyle(el).transform))
      .not.toBe(transformBefore)

    // Keyboard pan: focusing the body and pressing an arrow also moves it.
    await body.focus()
    const beforeKey = await img.evaluate(el => getComputedStyle(el).transform)
    await page.keyboard.press('ArrowRight')
    await expect
      .poll(() => img.evaluate(el => getComputedStyle(el).transform))
      .not.toBe(beforeKey)

    // Fit returns to the fit render (and resets pan).
    await fit.getByText('Fit', { exact: true }).click()
    await expect(body).toHaveAttribute('data-view-mode', 'fit')
  })
})
