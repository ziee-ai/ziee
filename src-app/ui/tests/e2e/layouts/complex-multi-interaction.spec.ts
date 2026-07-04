import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { createRemoteProvider } from '../llm/helpers/provider-helpers'
import { clickProviderCard } from '../llm/helpers/navigation-helpers'

/**
 * E2E — a COMBINED multi-interaction sequence (no single spec chained these):
 * toggle the sidebar → open a drawer → resize the drawer → navigate → restore
 * the sidebar. Guards that these app-shell interactions compose without leaving
 * the layout in a broken state.
 */

test.describe('App layout — complex multi-interaction sequence', () => {
  test('toggle sidebar → open + resize drawer → navigate → restore sidebar', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    const providerName = `multi-${Date.now()}`

    await loginAsAdmin(page, baseURL)
    await createRemoteProvider(
      page,
      baseURL,
      providerName,
      'https://api.openai.com/v1',
      'sk-test-key',
    )
    await clickProviderCard(page, providerName)

    // 1) Collapse the sidebar.
    const toggleBtn = page.getByTestId('layout-sidebar-toggle-button')
    await expect(toggleBtn).toBeVisible({ timeout: 30000 })
    await expect(toggleBtn).toHaveAttribute('aria-expanded', 'true')
    await toggleBtn.click()
    await expect(toggleBtn).toHaveAttribute('aria-expanded', 'false', {
      timeout: 10000,
    })

    // 2) Open the Add Remote Model drawer (app-layout custom Drawer).
    const modelsCard = page.getByTestId('llm-models-section-card')
    await modelsCard.getByTestId('llm-models-add-remote-btn').click()
    await expect(
      page.getByTestId('llm-add-remote-model-form'),
    ).toBeVisible({ timeout: 15000 })

    // 3) Resize the drawer wider via its handle.
    const wrapper = page.getByTestId('layout-drawer-content').last()

    // Let the slide-in animation settle first — mid-animation the Content
    // carries a `translateX(...)` transform that puts the (absolute) resize
    // handle off-screen, so a drag would land on empty space. Ends at `none`.
    await expect
      .poll(async () =>
        wrapper.evaluate(el => getComputedStyle(el as HTMLElement).transform),
      )
      .toBe('none')

    const before = (await wrapper.boundingBox())!.width
    const handle = page.getByTestId('drawer-resize-handle').last()
    const hb = (await handle.boundingBox())!
    await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2)
    await page.mouse.down()
    await page.mouse.move(hb.x - 180, hb.y + hb.height / 2, { steps: 8 })
    await page.mouse.up()
    await expect
      .poll(async () => (await wrapper.boundingBox())!.width, { timeout: 5000 })
      .toBeGreaterThan(before)

    // 4) Close the drawer + navigate to another settings section.
    await page.keyboard.press('Escape')
    await page.goto(`${baseURL}/settings/hardware`)
    await expect(
      page.getByTestId('hardware-settings-connection-card'),
    ).toBeAttached({ timeout: 30000 })

    // 5) The collapsed sidebar persisted across navigation; restore it.
    const expandBtn = page.getByTestId('layout-sidebar-toggle-button')
    await expect(expandBtn).toBeVisible({ timeout: 10000 })
    await expect(expandBtn).toHaveAttribute('aria-expanded', 'false')
    await expandBtn.click()
    await expect(expandBtn).toHaveAttribute('aria-expanded', 'true', {
      timeout: 10000,
    })
  })
})
