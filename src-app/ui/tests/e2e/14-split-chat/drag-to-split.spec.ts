import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * Split-chat E2E — drag-and-drop workspace edits (TEST-28, ITEM-31). Dragging a
 * conversation onto a pane header REPLACES that pane; onto the inter-pane seam
 * OPENS a new pane; dragging a pane grip onto another pane's header REORDERS; and
 * an OS **file** dropped on a pane's workspace zone is IGNORED (the file-vs-
 * conversation disambiguation — a `Files`-typed drag never triggers a pane
 * replace/reorder). Driven via synthetic HTML5 DnD (a shared DataTransfer handle),
 * since the handlers only read `dataTransfer` (`paneDnd.ts`). No LLM.
 */
test.describe('Split chat — drag-to-split', () => {
  const CONV_MIME = 'application/x-ziee-conversation'

  const mkConv = async (
    page: import('@playwright/test').Page,
    apiURL: string,
    token: string,
    title: string,
  ): Promise<string> => {
    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title },
    })
    expect(res.status()).toBeLessThan(300)
    return (await res.json()).id as string
  }

  // Fire a conversation drag onto a target (dragover+drop) with a synthesized
  // DataTransfer carrying the conversation MIME — the drop handler reads getData.
  const dropConversation = async (
    page: import('@playwright/test').Page,
    target: import('@playwright/test').Locator,
    convId: string,
  ) => {
    const dt = await page.evaluateHandle(
      ({ mime, id }) => {
        const d = new DataTransfer()
        d.setData(mime, id)
        return d
      },
      { mime: CONV_MIME, id: convId },
    )
    await target.dispatchEvent('dragover', { dataTransfer: dt })
    await target.dispatchEvent('drop', { dataTransfer: dt })
    await dt.dispose()
  }

  // Build a [A | B] split via the picker.
  const openAB = async (
    page: import('@playwright/test').Page,
    baseURL: string,
    convA: string,
    convB: string,
  ) => {
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(pane1.locator('textarea[placeholder*="Type your message"]')).toBeVisible({
      timeout: 15000,
    })
  }

  test('conversation→pane-header replaces; conversation→seam opens a new pane; a file is ignored', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Drag Alpha')
    const convB = await mkConv(page, apiURL, token, 'Drag Bravo')
    const convC = await mkConv(page, apiURL, token, 'Drag Charlie')
    await openAB(page, baseURL, convA, convB)

    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')

    // Drop conversation C onto pane 0's header → REPLACE pane 0 with C.
    await dropConversation(page, pane0.getByTestId('chat-pane-header'), convC)
    await expect(pane0.getByTestId('conversation-title')).toContainText('Charlie', {
      timeout: 15000,
    })
    // Pane 1 (B) is untouched — the replace was scoped to pane 0.
    await expect(pane1.getByTestId('conversation-title')).toContainText('Bravo')

    // Drop conversation A (not currently open) onto the seam → OPEN a new pane.
    await dropConversation(page, byTestId(page, 'split-divider-0'), convA)
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible({ timeout: 15000 })

    // Negative: an OS file dropped on a pane header is IGNORED (dragKind→'file'),
    // so pane 1's conversation does not change and no pane is replaced/added.
    const fileDt = await page.evaluateHandle(() => {
      const d = new DataTransfer()
      d.items.add(new File(['x'], 'note.txt', { type: 'text/plain' }))
      return d
    })
    await pane1.getByTestId('chat-pane-header').dispatchEvent('drop', { dataTransfer: fileDt })
    await fileDt.dispose()
    await expect(pane1.getByTestId('conversation-title')).toContainText('Bravo')
    await expect(byTestId(page, 'chat-pane-3')).toHaveCount(0)
  })

  test('dragging a pane grip onto another pane header reorders the panes', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convA = await mkConv(page, apiURL, token, 'Order Alpha')
    const convB = await mkConv(page, apiURL, token, 'Order Bravo')
    await openAB(page, baseURL, convA, convB)

    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0.getByTestId('conversation-title')).toContainText('Alpha')
    await expect(pane1.getByTestId('conversation-title')).toContainText('Bravo')

    // Faithful pane-drag: dispatch dragstart on pane 0's grip (the app writes the
    // pane MIME into the DataTransfer), then drop on pane 1's header → reorder.
    const dt = await page.evaluateHandle(() => new DataTransfer())
    await pane0.getByTestId('chat-pane-grip').dispatchEvent('dragstart', { dataTransfer: dt })
    await pane1.getByTestId('chat-pane-header').dispatchEvent('dragover', { dataTransfer: dt })
    await pane1.getByTestId('chat-pane-header').dispatchEvent('drop', { dataTransfer: dt })
    await dt.dispose()

    // Order swapped: pane 0 now shows Bravo, pane 1 shows Alpha.
    await expect(pane0.getByTestId('conversation-title')).toContainText('Bravo', {
      timeout: 15000,
    })
    await expect(pane1.getByTestId('conversation-title')).toContainText('Alpha')
  })
})
