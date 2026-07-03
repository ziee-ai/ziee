import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { createRemoteProvider } from '../llm/helpers/provider-helpers'
import { clickProviderCard } from '../llm/helpers/navigation-helpers'

/**
 * E2E — app-layout Drawer resize handle (`Drawer.tsx:166` →
 * `ResizeHandle placement="left"`). Dragging the handle resizes the drawer; no
 * E2E exercised it. We open a drawer that uses the custom app-layout Drawer
 * (the "Add Remote Model" drawer) and drag its `drawer-resize-handle`,
 * asserting the drawer width changes.
 */

test.describe('App layout — drawer resize handle', () => {
  test('dragging the resize handle widens the drawer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const providerName = `resize-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createRemoteProvider(
      page,
      baseURL,
      providerName,
      'https://api.openai.com/v1',
      'sk-test-key',
    )
    await clickProviderCard(page, providerName)

    // Open the Add Remote Model drawer (uses the app-layout Drawer wrapper).
    const modelsCard = page.getByTestId('llm-models-section-card')
    await modelsCard.getByTestId('llm-models-add-remote-btn').click()
    await expect(
      page.getByTestId('llm-add-remote-model-form'),
    ).toBeVisible({ timeout: 15000 })

    const wrapper = page.getByTestId('layout-drawer-content').last()

    // Wait for the drawer's slide-in animation to fully settle before measuring
    // or dragging. Mid-animation the Content still carries a `translateX(...)`
    // transform, so the (position:absolute) resize handle is off-screen — a
    // drag started then lands on empty space and the width never changes.
    // The enter animation ends at `transform: none`.
    await expect
      .poll(async () =>
        wrapper.evaluate(el => getComputedStyle(el as HTMLElement).transform),
      )
      .toBe('none')

    const before = (await wrapper.boundingBox())!.width

    // Drag the resize handle leftward to widen the (right-side) drawer.
    const handle = page.getByTestId('drawer-resize-handle').last()
    const hb = (await handle.boundingBox())!
    await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2)
    await page.mouse.down()
    await page.mouse.move(hb.x - 200, hb.y + hb.height / 2, { steps: 10 })
    await page.mouse.up()

    // The drawer got wider (or at least changed width) after the drag.
    await expect
      .poll(async () => (await wrapper.boundingBox())!.width, { timeout: 5000 })
      .toBeGreaterThan(before)
  })
})
