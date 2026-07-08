import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  mockPaginatedMessages,
  type MockMessageWithContent,
} from '../helpers/sse-mock-helpers'

/**
 * message-scroll-perf ITEM-3 — inline images reserve their row height BEFORE the
 * bytes arrive, so an async image load doesn't thrash the virtualizer's row
 * measurement and jump the viewport (symptom 3, images).
 *
 * Regression this guards: markdown images rendered as a bare `<img>` (~0 height
 * until loaded); under virtualization the row measured short, the image loaded,
 * the row grew, the ResizeObserver fired and shifted scroll. `ReservedImage`
 * reserves a stable min-height up front (released on load), so the row height is
 * stable from first paint.
 */

async function seedConversation(apiURL: string, token: string, title: string) {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed failed: ${res.status}`)
  return (await res.json()).id as string
}

// A same-origin, 240×240 SVG so the img renderer's ALLOWED branch routes it
// through ReservedImage (root-relative src). Served with a delay so the
// pre-load reservation is observable.
const IMG_PATH = '/perf-reserved-image.svg'
const SVG =
  `<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240">` +
  `<rect width="240" height="240" fill="#4488cc"/></svg>`

function assistantWithImage(id: string): MockMessageWithContent {
  return {
    id,
    role: 'assistant',
    contents: [
      {
        content_type: 'text',
        content: {
          type: 'text',
          text: `Here is a chart:\n\n![chart](${IMG_PATH})\n\nDone.`,
        },
      },
    ],
  }
}

test.describe('message-scroll-perf — inline image height reservation', () => {
  test('row reserves height before load; no jump when the image arrives', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: image reserve')

    // Delay the image so we can observe the reserved (pre-load) state.
    let releaseImage: (() => void) | null = null
    const imageGate = new Promise<void>(r => (releaseImage = r))
    await page.route(`**${IMG_PATH}`, async route => {
      await imageGate
      await route.fulfill({
        status: 200,
        contentType: 'image/svg+xml',
        body: SVG,
      })
    })

    // A short reference message BELOW the image message, to detect any jump.
    const win: MockMessageWithContent[] = [
      {
        id: 'ref-top',
        role: 'user',
        contents: [{ content_type: 'text', content: { type: 'text', text: 'Show me a chart' } }],
      },
      assistantWithImage('img-msg'),
      {
        id: 'ref-below',
        role: 'user',
        contents: [{ content_type: 'text', content: { type: 'text', text: 'REFERENCE ROW' } }],
      },
    ]
    await mockPaginatedMessages(page, win)

    await page.goto(`${baseURL}/chat/${convId}`)
    await expect(page.getByTestId('chat-messages')).toBeVisible({
      timeout: 30000,
    })

    // The reserved-image placeholder is present and NOT yet loaded.
    const reserved = page.getByTestId('reserved-image').first()
    await expect(reserved).toBeVisible({ timeout: 10000 })
    await expect(reserved).not.toHaveAttribute('data-loaded', '')

    // The reservation gives the placeholder a real (non-tiny) height BEFORE the
    // bytes arrive — the row is not collapsed to ~0.
    const reservedH = (await reserved.boundingBox())?.height ?? 0
    expect(reservedH).toBeGreaterThan(200)

    // Reference row position before the image resolves.
    const beforeY = (await page
      .locator('[data-message-id="ref-below"]')
      .boundingBox())?.y ?? 0

    // Release the image; it loads into the already-reserved box.
    releaseImage?.()
    await expect(reserved).toHaveAttribute('data-loaded', '', { timeout: 10000 })
    await page.waitForTimeout(400)

    // The reference row barely moved — the reserved height (240) matched the
    // natural image height (240), so the load produced no meaningful jump.
    const afterY = (await page
      .locator('[data-message-id="ref-below"]')
      .boundingBox())?.y ?? 0
    expect(Math.abs(afterY - beforeY)).toBeLessThan(24)
  })
})
