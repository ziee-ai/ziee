import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// TEST-39 (ITEM-32,33): the chat composer KB-attach affordance — the "+" menu
// exposes a "Knowledge bases" picker; toggling a KB shows a status-row chip; the
// chip's × detaches it. (Grounding is resolved server-side from the attachment.)
test.describe('Knowledge Base — chat composer attach', () => {
  test('attach a KB from the + picker → chip → detach', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const kb = await page.request.post(`${apiURL}/api/knowledge-bases`, {
      headers: { Authorization: `Bearer ${await getAdminToken(apiURL)}` },
      data: { name: 'Grounding KB' },
    })
    const kbId: string = (await kb.json()).id

    await page.goto(`${baseURL}/chat`)

    // Open the composer "+" dropdown → the Knowledge-bases picker.
    await byTestId(page, 'chat-input-add-btn').click()
    await byTestId(page, 'kb-menu-trigger').click()

    // Toggle the KB on → its status-row chip appears.
    await byTestId(page, `kb-option-${kbId}`).click()
    const chip = byTestId(page, `kb-chip-${kbId}`)
    await expect(chip).toBeVisible()
    await expect(chip).toContainText('Grounding KB')

    // Detach via the chip's close affordance → the chip disappears.
    await chip.getByRole('button').first().click()
    await expect(byTestId(page, `kb-chip-${kbId}`)).toHaveCount(0)
  })
})
