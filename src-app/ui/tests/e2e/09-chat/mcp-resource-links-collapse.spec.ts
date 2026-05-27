import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedAssistantWithToolResult } from './fixtures/mock-tool-result'

/**
 * InlineFilePreview collapse UX:
 *   - chevron is the ONLY collapse toggle
 *   - body click is inert
 *   - keyboard accessibility (Tab + Enter/Space on chevron)
 *   - aria-expanded mirrors state
 */

test.describe('Inline file previews — collapse UX', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('preview starts expanded by default', async ({ page, testInfra }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri: '/api/files/coll-1/download', name: 'p.png', mime_type: 'image/png' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toBeVisible()
    await expect(preview.locator('[data-testid="inline-file-preview-chevron"]'))
      .toHaveAttribute('aria-expanded', 'true')
  })

  test('clicking the chevron collapses the body', async ({ page, testInfra }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri: '/api/files/coll-2/download', name: 'p.png', mime_type: 'image/png' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await preview.locator('[data-testid="inline-file-preview-chevron"]').click()
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
    await expect(preview.locator('[data-testid="inline-file-preview-chevron"]'))
      .toHaveAttribute('aria-expanded', 'false')
    // Header still visible.
    await expect(preview).toContainText('p.png')
  })

  test('clicking the body does NOT toggle collapse', async ({ page, testInfra }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri: '/api/files/coll-3/download', name: 'p.png', mime_type: 'image/png' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    const body = preview.locator('[data-testid="inline-file-preview-body"]')
    await expect(body).toBeVisible({ timeout: 10000 })
    await body.click() // click the body
    // Still visible; chevron aria-expanded still true.
    await expect(body).toBeVisible()
    await expect(preview.locator('[data-testid="inline-file-preview-chevron"]'))
      .toHaveAttribute('aria-expanded', 'true')
  })

  test('clicking chevron again re-expands', async ({ page, testInfra }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri: '/api/files/coll-4/download', name: 'p.png', mime_type: 'image/png' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    const chevron = preview.locator('[data-testid="inline-file-preview-chevron"]')
    await expect(preview).toBeVisible({ timeout: 10000 })
    await chevron.click() // collapse
    await chevron.click() // re-expand
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toBeVisible()
    await expect(chevron).toHaveAttribute('aria-expanded', 'true')
  })

  test('keyboard: focus chevron then Enter toggles collapse', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri: '/api/files/coll-kb/download', name: 'p.png', mime_type: 'image/png' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    const chevron = preview.locator('[data-testid="inline-file-preview-chevron"]')
    await expect(preview).toBeVisible({ timeout: 10000 })
    await chevron.focus()
    await page.keyboard.press('Enter')
    await expect(chevron).toHaveAttribute('aria-expanded', 'false')
    await page.keyboard.press('Enter')
    await expect(chevron).toHaveAttribute('aria-expanded', 'true')
  })

  test('each preview collapses independently', async ({ page, testInfra }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/coll-indep-1/download', name: 'a.png', mime_type: 'image/png' },
        { uri: '/api/files/coll-indep-2/download', name: 'b.png', mime_type: 'image/png' },
        { uri: '/api/files/coll-indep-3/download', name: 'c.png', mime_type: 'image/png' },
      ],
    })
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(3, { timeout: 10000 })
    // Collapse only the middle one.
    await previews.nth(1).locator('[data-testid="inline-file-preview-chevron"]').click()
    await expect(previews.nth(0).locator('[data-testid="inline-file-preview-body"]')).toBeVisible()
    await expect(previews.nth(1).locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
    await expect(previews.nth(2).locator('[data-testid="inline-file-preview-body"]')).toBeVisible()
  })

  test('header always shows filename, label, icon, open-in-new-tab', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/coll-hdr/download', name: 'plot.png', mime_type: 'image/png', size: 12345 },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview).toContainText('plot.png')
    await expect(preview).toContainText('Image') // viewer label
    await expect(preview.locator('.anticon').first()).toBeVisible() // icon
    await expect(preview.locator('[data-testid="inline-file-preview-open"]')).toBeVisible()
    // Header still present after collapsing.
    await preview.locator('[data-testid="inline-file-preview-chevron"]').click()
    await expect(preview).toContainText('plot.png')
  })

  test('open-in-new-tab link has target=_blank and rel=noopener', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/coll-open/download'
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'p.png', mime_type: 'image/png' }],
    })
    const link = page.locator('[data-testid="inline-file-preview-open"]').first()
    await expect(link).toBeVisible({ timeout: 10000 })
    await expect(link).toHaveAttribute('target', '_blank')
    const rel = await link.getAttribute('rel')
    expect(rel ?? '').toContain('noopener')
    await expect(link).toHaveAttribute('href', uri)
  })
})
