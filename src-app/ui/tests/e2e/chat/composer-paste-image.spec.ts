import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'

/**
 * ITEM-8 / TEST-14 — paste an image from the clipboard onto the composer.
 *
 * Dispatches a synthetic `paste` carrying an image File onto the composer and
 * asserts it becomes a pending attachment (the real upload path runs). Pasting
 * plain text adds no attachment.
 */

// 1x1 transparent PNG.
const PNG_B64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=='

async function pasteImage(page: Page) {
  await page.evaluate((b64) => {
    const bin = atob(b64)
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
    const file = new File([bytes], 'pasted.png', { type: 'image/png' })
    const dt = new DataTransfer()
    dt.items.add(file)
    const el = document.querySelector('[data-chat-composer]')!
    const event = new Event('paste', { bubbles: true, cancelable: true })
    Object.defineProperty(event, 'clipboardData', { value: dt })
    el.dispatchEvent(event)
  }, PNG_B64)
}

async function pasteText(page: Page, text: string) {
  await page.evaluate((t) => {
    const dt = new DataTransfer()
    dt.setData('text/plain', t)
    const el = document.querySelector('[data-chat-composer]')!
    const event = new Event('paste', { bubbles: true, cancelable: true })
    Object.defineProperty(event, 'clipboardData', { value: dt })
    el.dispatchEvent(event)
  }, text)
}

test.describe('Chat — paste image from clipboard', () => {
  test('a pasted image becomes an attachment; pasted text does not', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToNewChatPage(page, baseURL)
    await expect(page.locator('[data-chat-composer]')).toBeVisible({ timeout: 30000 })

    const attachedList = page.getByRole('list', { name: 'Attached files' })

    // Plain-text paste: no attachment appears.
    await pasteText(page, 'just some text')
    await expect(attachedList).toHaveCount(0)

    // Image paste: an attachment card shows up (uploading → completed).
    await pasteImage(page)
    await expect(attachedList).toBeVisible({ timeout: 15000 })
    await expect(
      page.getByTestId('file-card').or(page.getByTestId('file-card-uploading')),
    ).toHaveCount(1, { timeout: 15000 })
  })
})
