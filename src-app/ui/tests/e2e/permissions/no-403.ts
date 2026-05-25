import { test as base } from '../../fixtures/test-context'

/**
 * Test fixture that fails the running test if any /api/* response
 * returns 403 during execution. Run the suite under this fixture as
 * a non-admin user and missing UI gates surface as test failures —
 * the highest-leverage way to keep gating coverage complete across
 * the codebase.
 *
 * Negative tests that intentionally probe 403s (e.g. asserting the
 * backend rejects an unauthorized API call) can opt out by setting
 * `test.use({ allow403: true })` at the describe level.
 */

interface No403Options {
  /** Set true to disable the 403 check (for tests that intentionally probe 403s). */
  allow403?: boolean
}

export const test = base.extend<No403Options>({
  allow403: [false, { option: true }],

  page: async ({ page, allow403 }, use, testInfo) => {
    const unexpected403s: string[] = []

    page.on('response', resp => {
      if (allow403) return
      if (!resp.url().includes('/api/')) return
      if (resp.status() === 403) {
        unexpected403s.push(`${resp.request().method()} ${resp.url()}`)
      }
    })

    await use(page)

    if (unexpected403s.length > 0 && testInfo.status === testInfo.expectedStatus) {
      throw new Error(
        `Unexpected 403 response(s) from /api during test — likely a missing UI permission gate:\n` +
          unexpected403s.map(e => `  - ${e}`).join('\n'),
      )
    }
  },
})

export { expect } from '@playwright/test'
