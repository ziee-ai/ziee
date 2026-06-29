import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// ---------------------------------------------------------------------------
// ServerUpdateBanner "Release notes" link (ServerUpdateBanner.tsx:53-58).
// The banner renders an external anchor to the GitHub release page ONLY when
// the status payload carries a `release_url` (→ store `releaseUrl`). The
// existing server-update.spec covers the banner text + "How to update" route +
// dismiss, but never asserts the Release-notes link's href/attributes, nor the
// branch where it is absent. The /status endpoint is the external boundary and
// is mocked to drive the update-available state deterministically.
// ---------------------------------------------------------------------------

const RELEASE_URL =
  'https://github.com/phibya/ziee-chat-new/releases/tag/v0.2.0'

const STATUS_AVAILABLE = {
  current_version: '0.1.0',
  latest_version: '0.2.0',
  update_available: true,
  release_url: RELEASE_URL,
  notes: 'New release',
  checked_at: '2026-06-12T00:00:00Z',
  enabled: true,
}

async function mockStatus(page: Page, body: unknown) {
  await page.route(/\/api\/server-update\/status$/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(body),
    })
  })
}

test.describe('Server update banner — release notes link', () => {
  test('renders the Release notes link to the release URL (new tab, noreferrer)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await mockStatus(page, STATUS_AVAILABLE)
    await loginAsAdmin(page, baseURL)

    // The banner lives in AppLayout on every authenticated page. '0.2.0' is
    // dynamic data from the mocked status payload.
    await expect(byTestId(page, 'serverupd-banner-alert')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'serverupd-banner-alert')).toContainText('0.2.0')

    const releaseLink = byTestId(page, 'serverupd-banner-release-notes-link')
    await expect(releaseLink).toBeVisible()
    await expect(releaseLink).toHaveAttribute('href', RELEASE_URL)
    // Opens in a new tab without leaking the referrer (external GitHub page).
    await expect(releaseLink).toHaveAttribute('target', '_blank')
    await expect(releaseLink).toHaveAttribute('rel', /noreferrer/)
  })

  test('omits the Release notes link when the status carries no release_url', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // Same update-available banner, but no release_url → the `releaseUrl &&`
    // guard at ServerUpdateBanner.tsx:53 drops the link entirely.
    await mockStatus(page, { ...STATUS_AVAILABLE, release_url: null })
    await loginAsAdmin(page, baseURL)

    await expect(byTestId(page, 'serverupd-banner-alert')).toBeVisible({
      timeout: 30000,
    })
    await expect(byTestId(page, 'serverupd-banner-alert')).toContainText('0.2.0')
    // The banner still shows "How to update" but not the external link.
    await expect(byTestId(page, 'serverupd-banner-howto-btn')).toBeVisible()
    await expect(byTestId(page, 'serverupd-banner-release-notes-link')).toHaveCount(0)
  })
})
