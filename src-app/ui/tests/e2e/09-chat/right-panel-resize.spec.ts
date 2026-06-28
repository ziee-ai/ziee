import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedLiteratureResult, sampleResult } from '../19-literature/fixtures/mock-literature-result'

/**
 * E2E — chat right-panel resize handle (ChatRightPanel.tsx:197). The panel only
 * exists when a right-panel tab is open, so we open one via a seeded literature
 * result, then drag the left-edge ResizeHandle and assert the panel widens. This
 * exercises the drag→setRightPanelWidth path that no E2E covered.
 */

test.describe('Chat — right panel resize', () => {
  test('dragging the right-panel resize handle widens the panel', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')

    // Seed a literature_search tool_result → its inline card opens the screening
    // right panel.
    await seedLiteratureResult(page, baseURL, sampleResult())
    await page.getByRole('button', { name: /Open in screening/ }).click()
    await expect(page.getByRole('heading', { name: 'Screening' })).toBeVisible({
      timeout: 15000,
    })

    const panel = page.locator('[data-testid="chat-right-panel"]')
    await expect(panel).toBeVisible({ timeout: 10000 })
    const before = (await panel.boundingBox())!.width

    // Drag the left-edge resize handle further LEFT to widen the panel.
    const handle = panel.locator('.cursor-col-resize').first()
    const hb = (await handle.boundingBox())!
    await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2)
    await page.mouse.down()
    await page.mouse.move(hb.x - 150, hb.y + hb.height / 2, { steps: 10 })
    await page.mouse.up()

    await expect
      .poll(async () => (await panel.boundingBox())!.width, { timeout: 5000 })
      .toBeGreaterThan(before)
  })
})
