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
 * Regression guarded: markdown images rendered as a bare `<img>` (~0 height
 * until loaded); under virtualization the row measured short, the image loaded,
 * the row grew, the ResizeObserver fired and shifted scroll. `ReservedImage`
 * reserves a stable min-height up front (released on load), so the row is not
 * collapsed and the post-load growth is bounded to |natural − reserved| instead
 * of the full natural height.
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

// A same-origin, 360px-tall SVG — DELIBERATELY taller than the 240px dimensionless
// reservation, so the "no meaningful jump" assertion is NOT trivially satisfied
// by matching sizes. Without the reservation the row would grow from ~0 → 360
// (a 360px jump); with it, from 240 → 360 (a bounded ~120px jump). A root-
// relative src routes through the ALLOWED → ReservedImage branch. Served with a
// gate so the pre-load reservation is observable.
const IMG_PATH = '/perf-reserved-image.svg'
const IMG_NATURAL = 360
const SVG =
  `<svg xmlns="http://www.w3.org/2000/svg" width="480" height="${IMG_NATURAL}">` +
  `<rect width="480" height="${IMG_NATURAL}" fill="#4488cc"/></svg>`

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
  test('row reserves height before load; post-load growth is bounded', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Perf: image reserve')

    // Delay the image so we can observe the reserved (pre-load) state.
    let releaseImage: () => void = () => {}
    const imageGate = new Promise<void>(resolve => {
      releaseImage = resolve
    })
    await page.route(`**${IMG_PATH}`, async route => {
      await imageGate
      await route.fulfill({
        status: 200,
        contentType: 'image/svg+xml',
        body: SVG,
      })
    })

    const win: MockMessageWithContent[] = [
      {
        id: 'ref-top',
        role: 'user',
        contents: [
          { content_type: 'text', content: { type: 'text', text: 'Show me a chart' } },
        ],
      },
      assistantWithImage('img-msg'),
      {
        id: 'ref-below',
        role: 'user',
        contents: [
          { content_type: 'text', content: { type: 'text', text: 'REFERENCE ROW' } },
        ],
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

    // PRIMARY assertion (the ITEM-3 fix): the row is NOT collapsed to ~0 before
    // the bytes arrive — it holds its reserved height.
    const reservedH = (await reserved.boundingBox())?.height ?? 0
    expect(reservedH).toBeGreaterThan(200)

    // Reference row position before the image resolves.
    const beforeY =
      (await page.locator('[data-message-id="ref-below"]').boundingBox())?.y ?? 0

    // Release the image; it loads into the already-reserved box.
    releaseImage()
    await expect(reserved).toHaveAttribute('data-loaded', '', { timeout: 10000 })
    await page.waitForTimeout(400)

    // The reference row moved by at most |natural − reserved| (~120px), FAR less
    // than the ~360px jump an unreserved (0-height) image would have produced.
    const afterY =
      (await page.locator('[data-message-id="ref-below"]').boundingBox())?.y ?? 0
    const shift = Math.abs(afterY - beforeY)
    expect(shift).toBeLessThan(IMG_NATURAL - 180) // < 180; unreserved would be ~360
  })
})
