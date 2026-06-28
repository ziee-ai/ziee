import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — tab-visibility permission refresh (`Auth.store.ts:319-340`).
 *
 * A `visibilitychange` listener re-fetches GET /api/auth/me when the tab
 * becomes visible again, so a permission change made elsewhere self-heals on
 * refocus. This drives the real listener (no mocking of the behavior) and
 * asserts a fresh /me request fires when the tab returns to "visible".
 */

test.describe('Authentication — visibility permission refresh', () => {
  test('refocusing the tab re-fetches /api/auth/me', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    // Let the initial bootstrap /me settle.
    await page.waitForTimeout(1000)

    let meCalls = 0
    page.on('request', req => {
      if (req.method() === 'GET' && req.url().includes('/api/auth/me')) {
        meCalls++
      }
    })

    // Simulate the tab going hidden then visible again, dispatching the real
    // visibilitychange event the store's listener is bound to.
    await page.evaluate(() => {
      const set = (v: string) =>
        Object.defineProperty(document, 'visibilityState', {
          configurable: true,
          get: () => v,
        })
      set('hidden')
      document.dispatchEvent(new Event('visibilitychange'))
      set('visible')
      document.dispatchEvent(new Event('visibilitychange'))
    })

    // The listener fires a fresh /me on the visible transition.
    await expect.poll(() => meCalls, { timeout: 10000 }).toBeGreaterThan(0)
  })
})
