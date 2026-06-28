import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { createRemoteProvider } from '../05-llm/helpers/provider-helpers'
import { clickProviderCard } from '../05-llm/helpers/navigation-helpers'

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
    const collapseBtn = page.getByRole('button', { name: 'Close navigation menu' })
    await expect(collapseBtn).toBeVisible({ timeout: 30000 })
    await collapseBtn.click()
    await expect(
      page.getByRole('button', { name: 'Open navigation menu' }),
    ).toBeVisible({ timeout: 10000 })

    // 2) Open the Add Remote Model drawer (app-layout custom Drawer).
    const modelsCard = page.locator(
      '.ant-card:has(.ant-card-head-title:has-text("Models"))',
    )
    await modelsCard.getByRole('button', { name: 'Add model' }).click()
    await expect(
      page.locator('.ant-drawer-title:has-text("Add Remote Model")'),
    ).toBeVisible({ timeout: 15000 })

    // 3) Resize the drawer wider via its handle.
    const wrapper = page.locator('.ant-drawer-content-wrapper').last()
    const before = (await wrapper.boundingBox())!.width
    const handle = page.locator('[data-testid="drawer-resize-handle"]').last()
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
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    // 5) The collapsed sidebar persisted across navigation; restore it.
    const expandBtn = page.getByRole('button', { name: 'Open navigation menu' })
    await expect(expandBtn).toBeVisible({ timeout: 10000 })
    await expandBtn.click()
    await expect(
      page.getByRole('button', { name: 'Close navigation menu' }),
    ).toBeVisible({ timeout: 10000 })
  })
})
