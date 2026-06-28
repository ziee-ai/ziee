import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — RebuildStatusSection live progress polling on /settings/memory-admin.
 * audit id c02b08c19d0a — the section self-hides unless a re-embed is in flight
 * and polls the status endpoint every 2s; that polling/progress was untested.
 *
 * We mock ONLY the rebuild-status endpoint (the external boundary) to report an
 * in-progress rebuild whose pending_count decreases on each poll, and assert the
 * progress card appears and its "N memories remaining" text updates WITHOUT a
 * reload — proving the 2s polling refetch + re-render path works.
 */

test.describe('Memory — rebuild status polling', () => {
  test.describe.configure({ retries: 2 })

  test('re-embed progress card appears and updates via polling', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Decreasing pending_count across polls; in_progress stays true.
    let pending = 10
    await page.route(
      /\/api\/memory\/admin-settings\/rebuild-status$/,
      async route => {
        const body = {
          in_progress: true,
          model_name: 'nomic-embed-text',
          pending_count: pending,
        }
        if (pending > 2) pending -= 4
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(body),
        })
      },
    )

    await page.goto(`${baseURL}/settings/memory-admin`)

    // The progress card renders because a rebuild is in flight.
    await expect(page.getByText('Re-embedding memories')).toBeVisible({
      timeout: 15000,
    })
    // First poll snapshot.
    await expect(page.getByText('10 memories remaining.')).toBeVisible({
      timeout: 10000,
    })
    // Polling (every 2s) refetches the decreasing count → the text updates
    // without a manual reload.
    await expect(page.getByText('6 memories remaining.')).toBeVisible({
      timeout: 10000,
    })
    await expect(page.getByText('2 memories remaining.')).toBeVisible({
      timeout: 10000,
    })
  })
})
