import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  getAdminToken,
  createTestUser,
  login,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'

// ---------------------------------------------------------------------------
// Server self-update NOTIFICATION UI (admin-only). The server polls GitHub and
// caches the result; the web UI shows a dismissible banner (AppLayout, via the
// `appBanners` slot) + a Settings → About page. Notification only — no install.
// The real /api/server-update/status reflects a daily background poll, so it's
// mocked here to drive the "update available" state deterministically.
// ---------------------------------------------------------------------------

const STATUS_AVAILABLE = {
  current_version: '0.1.0',
  latest_version: '0.2.0',
  update_available: true,
  release_url: 'https://github.com/phibya/ziee-chat-new/releases/tag/v0.2.0',
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

test.describe('Server update notification', () => {
  test('About page shows current + latest version and the upgrade command', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await mockStatus(page, STATUS_AVAILABLE)
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/about`)

    // Auto-loads via the store __init__ (no Refresh click needed). The version
    // strings are dynamic data from the mocked /status payload — asserted inside
    // the About descriptions block by testid scope.
    await expect(byTestId(page, 'serverupd-about-descriptions')).toContainText('0.1.0', { timeout: 30000 })
    await expect(byTestId(page, 'serverupd-about-descriptions')).toContainText('0.2.0')
    await expect(byTestId(page, 'serverupd-update-available-tag')).toBeVisible()
    // The copy-paste upgrade command is present.
    await expect(byTestId(page, 'serverupd-upgrade-command')).toBeVisible()
  })

  test('copies the upgrade command to the clipboard', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await mockStatus(page, STATUS_AVAILABLE)
    // Grant clipboard write so antd's copyable can populate it headlessly.
    await page
      .context()
      .grantPermissions(['clipboard-read', 'clipboard-write'])
    await loginAsAdmin(page, baseURL)

    await page.goto(`${baseURL}/settings/about`)
    await expect(byTestId(page, 'serverupd-upgrade-command')).toBeVisible({
      timeout: 30000,
    })

    // Click the kit copy affordance on the copyable Paragraph.
    await byTestId(page, 'serverupd-copy-cmd-btn').first().click()

    const clip = await page.evaluate(() => navigator.clipboard.readText())
    expect(clip).toContain('install.sh | sh')
    expect(clip).toContain('curl -fsSL')
  })

  test('admin banner appears, links to About, and dismisses', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await mockStatus(page, STATUS_AVAILABLE)
    await loginAsAdmin(page, baseURL)

    // The banner lives in AppLayout (every authenticated page). '0.2.0' is
    // dynamic data from the mocked status payload.
    const banner = byTestId(page, 'serverupd-banner-alert')
    await expect(banner).toBeVisible({ timeout: 30000 })
    await expect(banner).toContainText('0.2.0')

    // "How to update" routes to the About page.
    await byTestId(page, 'serverupd-banner-howto-btn').click()
    await expect(page).toHaveURL(/\/settings\/about$/)

    // Dismiss hides it for the session.
    await page.goto(`${baseURL}/`)
    await expect(byTestId(page, 'serverupd-banner-alert')).toBeVisible()
    await byTestId(page, 'serverupd-banner-alert-close').first().click()
    await expect(byTestId(page, 'serverupd-banner-alert')).toHaveCount(0)
  })

  test('no banner when up to date', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await mockStatus(page, { ...STATUS_AVAILABLE, update_available: false, latest_version: '0.1.0' })
    // The admin banner mounts on the authed shell and fetches status once; wait
    // for that (mocked) response so the absence assertion is meaningful. NOT
    // `networkidle` — the app's SSE streams (chat/sync) keep the network busy,
    // so it never fires on an authed page (see the chat/projects helpers).
    const sawStatus = page.waitForResponse(r =>
      /\/api\/server-update\/status$/.test(r.url()),
    )
    await loginAsAdmin(page, baseURL)
    await sawStatus
    await expect(byTestId(page, 'serverupd-banner-alert')).toHaveCount(0)
  })

  test('Refresh re-fetches the status', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    let calls = 0
    await page.route(/\/api\/server-update\/status$/, async route => {
      calls += 1
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(STATUS_AVAILABLE),
      })
    })
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/about`)
    await expect(byTestId(page, 'serverupd-about-descriptions')).toContainText('0.2.0', { timeout: 30000 })
    const before = calls
    await byTestId(page, 'serverupd-refresh-btn').click()
    await expect.poll(() => calls).toBeGreaterThan(before)
  })

  test('surfaces an error when the status endpoint fails', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await page.route(/\/api\/server-update\/status$/, async route => {
      await route.fulfill({ status: 500, contentType: 'application/json', body: '{}' })
    })
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/about`)
    // Refresh deterministically triggers loadStatus (which sets `error`).
    await byTestId(page, 'serverupd-refresh-btn').click()
    await expect(byTestId(page, 'serverupd-error-alert')).toBeVisible({ timeout: 30000 })
  })

  test('About page shows the green "up to date" tag when current', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await mockStatus(page, {
      ...STATUS_AVAILABLE,
      update_available: false,
      latest_version: '0.1.0',
    })
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/about`)

    // Latest == current and no update → the "up to date" Tag renders
    // (NOT the "update available" one).
    await expect(byTestId(page, 'serverupd-uptodate-tag')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'serverupd-update-available-tag')).toHaveCount(0)
  })

  test('About page shows "not checked yet" before the first poll', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // Enabled but never polled → no latest_version yet.
    await mockStatus(page, {
      ...STATUS_AVAILABLE,
      enabled: true,
      update_available: false,
      latest_version: null,
      checked_at: null,
    })
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/about`)

    await expect(byTestId(page, 'serverupd-not-checked')).toBeVisible({
      timeout: 30000,
    })
  })

  test('shows the disabled (air-gapped) notice', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await mockStatus(page, {
      ...STATUS_AVAILABLE,
      enabled: false,
      update_available: false,
      latest_version: null,
      checked_at: null,
    })
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/about`)
    await expect(byTestId(page, 'serverupd-disabled-alert')).toBeVisible({ timeout: 30000 })
  })

  test('non-admin sees no banner and never calls the status endpoint', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    // The banner is <Can>-gated and the About route is permission-gated, so a
    // permission-less user must never even fetch /api/server-update/status
    // (the documented "no-403" gate).
    let statusCalls = 0
    page.on('request', req => {
      if (/\/api\/server-update\/status$/.test(req.url())) statusCalls += 1
    })

    const adminToken = await getAdminToken(baseURL)
    await createTestUser(
      baseURL,
      adminToken,
      'noupd_user',
      'noupd@example.com',
      'password123',
      [],
    )
    await login(page, baseURL, 'noupd_user', 'password123')

    await page.goto(`${baseURL}/`)
    // Wait for the authed shell to be interactive (the new-chat composer) so the
    // admin-only banner has had its full mount chance. NOT `networkidle` (SSE
    // streams never idle). The banner is <Can>-gated, so for a permission-less
    // user it must never mount and therefore never fetch /status.
    await expect(byTestId(page, 'chat-input-send-btn')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'serverupd-banner-alert')).toHaveCount(0)
    expect(statusCalls).toBe(0)
  })
})
