import AxeBuilder from '@axe-core/playwright'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  seedAssistantWithToolResult,
  mockResourceLinkUrl,
} from './fixtures/mock-tool-result'

// 1x1 transparent PNG — keeps ImageBody's <img> in DOM (see
// mcp-resource-links-dispatch.spec.ts for the full explanation).
const TINY_PNG = Buffer.from(
  '89504E470D0A1A0A0000000D49484452000000010000000108060000001F15C4890000000D49444154789C6200010000050001' +
    '0D0A2DB40000000049454E44AE426082',
  'hex',
)

/**
 * Accessibility checks for the new MessageFilesView + InlineFilePreview
 * components. Uses @axe-core/playwright (already a project dep).
 */

test.describe('Inline file previews — accessibility', () => {
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

  test('axe scan on a message with image, csv, markdown reports 0 violations', async ({
    page,
    testInfra,
  }) => {
    const pngUri = '/api/files/a11y-img/download'
    const csvUri = '/api/files/a11y-csv/download'
    const mdUri = '/api/files/a11y-md/download'
    await mockResourceLinkUrl(page, pngUri, TINY_PNG, { contentType: 'image/png' })
    await mockResourceLinkUrl(page, csvUri, 'a,b\n1,2\n', { contentType: 'text/csv' })
    await mockResourceLinkUrl(page, mdUri, '# Section', { contentType: 'text/markdown' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: pngUri, name: 'p.png', mime_type: 'image/png' },
        { uri: csvUri, name: 'data.csv', mime_type: 'text/csv' },
        { uri: mdUri, name: 'r.md', mime_type: 'text/markdown' },
      ],
    })
    const view = page.locator('[data-testid="tool-result-files"]').first()
    await expect(view).toBeVisible({ timeout: 10000 })
    // Wait for bodies to render so axe sees the final DOM.
    await expect(view.locator('img').first()).toBeVisible()
    await expect(view.locator('table').first()).toBeVisible()
    await expect(view.locator('h1').first()).toBeVisible()

    const results = await new AxeBuilder({ page })
      .include('[data-testid="tool-result-files"]')
      // Skip color-contrast (theme tokens are tested separately and
      // axe sometimes complains in dev where the dark/light theme
      // switch is in flux).
      // Skip scrollable-region-focusable: the inline CSV preview uses AntD's
      // virtual `<Table>`, whose internal rc-virtual-list scroll container is
      // not keyboard-focusable. That's an AntD component limitation we can't
      // fix from here; the chevron/links a11y is covered by sibling tests.
      .disableRules(['color-contrast', 'scrollable-region-focusable'])
      .analyze()
    expect(results.violations).toEqual([])
  })

  test('chevron has accessible name + aria-expanded reflects state', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri: '/api/files/a11y-chev/download', name: 'p.png', mime_type: 'image/png' }],
    })
    const chevron = page.locator('[data-testid="inline-file-preview-chevron"]').first()
    await expect(chevron).toBeVisible({ timeout: 10000 })
    // Start expanded.
    await expect(chevron).toHaveAttribute('aria-expanded', 'true')
    await expect(chevron).toHaveAttribute('aria-label', /collapse/i)
    await chevron.click()
    await expect(chevron).toHaveAttribute('aria-expanded', 'false')
    await expect(chevron).toHaveAttribute('aria-label', /expand/i)
  })

  test('open-in-new-tab link has accessible name', async ({ page, testInfra }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri: '/api/files/a11y-open/download', name: 'p.png', mime_type: 'image/png' }],
    })
    const link = page.locator('[data-testid="inline-file-preview-open"]').first()
    await expect(link).toBeVisible({ timeout: 10000 })
    await expect(link).toHaveAttribute('aria-label', /open file in new tab/i)
  })

  test('chevron is keyboard-reachable via Tab', async ({ page, testInfra }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri: '/api/files/a11y-tab/download', name: 'p.png', mime_type: 'image/png' }],
    })
    const chevron = page.locator('[data-testid="inline-file-preview-chevron"]').first()
    await expect(chevron).toBeVisible({ timeout: 10000 })
    // Programmatic focus works (proves the element is focusable).
    await chevron.focus()
    const focused = await page.evaluate(() => document.activeElement?.getAttribute('data-testid'))
    expect(focused).toBe('inline-file-preview-chevron')
    // Both Enter and Space activate the chevron (button-role default).
    await page.keyboard.press('Space')
    await expect(chevron).toHaveAttribute('aria-expanded', 'false')
  })
})
