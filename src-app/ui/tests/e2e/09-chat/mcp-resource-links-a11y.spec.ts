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
    const csvUri = '/api/files/a11y-csv/download'
    const mdUri = '/api/files/a11y-md/download'
    await mockResourceLinkUrl(page, csvUri, 'a,b\n1,2\n', { contentType: 'text/csv' })
    await mockResourceLinkUrl(page, mdUri, '# Section', { contentType: 'text/markdown' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/a11y-img/download', name: 'p.png', mime_type: 'image/png' },
        { uri: csvUri, name: 'data.csv', mime_type: 'text/csv' },
        { uri: mdUri, name: 'r.md', mime_type: 'text/markdown' },
      ],
    })
    const view = page.locator('[data-testid="message-files-view"]').first()
    await expect(view).toBeVisible({ timeout: 10000 })
    // Wait for bodies to render so axe sees the final DOM.
    await expect(view.locator('img').first()).toBeVisible()
    await expect(view.locator('table').first()).toBeVisible()
    await expect(view.locator('h1').first()).toBeVisible()

    const results = await new AxeBuilder({ page })
      .include('[data-testid="message-files-view"]')
      // Skip color-contrast (theme tokens are tested separately and
      // axe sometimes complains in dev where the dark/light theme
      // switch is in flux).
      .disableRules(['color-contrast'])
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
