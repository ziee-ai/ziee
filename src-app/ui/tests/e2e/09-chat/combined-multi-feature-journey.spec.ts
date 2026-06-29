import path from 'path'
import { byTestId } from '../testid'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'
import { attachFileViaUI } from './helpers/file-panel-helpers'

/**
 * E2E — a complex multi-feature combined journey in ONE chat composer.
 *
 * Audit gap: the chat specs each exercise a single feature; none combined
 * several in one flow. This selects an assistant AND attaches a file in the
 * same new conversation, asserting BOTH the assistant chip and the file
 * preview coexist in the composer (the combined multi-feature state).
 * Deterministic — no LLM.
 */

const PPTX_FIXTURE = path.resolve(
  __dirname,
  '../../../../server/tests/file/test_data/10_slides.pptx',
)
const PPTX_FILENAME = '10_slides.pptx'

test.describe('Chat — combined multi-feature journey', () => {
  test('assistant selection AND file attachment coexist in the composer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const assistantName = `Combined Assistant ${Date.now().toString(36)}`
    const created = await fetch(`${apiURL}/api/assistants`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ name: assistantName, instructions: 'Be terse.' }),
    })
    expect(created.status).toBeLessThan(300)

    await goToNewChatPage(page, baseURL)

    // Feature 1: select the assistant via the "+" dropdown.
    await page.getByRole('button', { name: 'Add attachment' }).click()
    await page.getByText('Select assistant').click()
    await expect(page.getByText(assistantName)).toBeVisible({ timeout: 10000 })
    await page.getByText(assistantName).click()
    await expect(
      page.locator('.ant-tag').filter({ hasText: assistantName }),
    ).toBeVisible({ timeout: 10000 })

    // Feature 2: attach a file via the "+" dropdown.
    await attachFileViaUI(page, PPTX_FIXTURE)
    await expect(
      page.locator(`[data-testid="file-card"][data-filename="${PPTX_FILENAME}"]`),
    ).toBeVisible({ timeout: 30000 })

    // Combined state: BOTH the assistant chip AND the file preview are present
    // together in the composer.
    await expect(
      page.locator('.ant-tag').filter({ hasText: assistantName }),
    ).toBeVisible()
    await expect(
      page.locator(`[data-testid="file-card"][data-filename="${PPTX_FILENAME}"]`),
    ).toBeVisible()
  })
})
