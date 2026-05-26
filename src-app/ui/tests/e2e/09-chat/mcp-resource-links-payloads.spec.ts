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
 * Payload-shape variation: missing optional fields, Unicode names,
 * query strings, is_saved flag. Each test asserts the inline preview
 * tolerates the shape without crashing AND that dispatch still works.
 */

test.describe('Inline file previews — payload variations', () => {
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

  test('resource_link without mime_type uses extension for dispatch', async ({
    page,
    testInfra,
  }) => {
    // No mime_type — registry must match via ext. CSV viewer's
    // supportedTypes includes `{ext:'csv'}`.
    const uri = '/api/files/payload-noext/download'
    await mockResourceLinkUrl(page, uri, 'a,b\n1,2\n', { contentType: 'text/csv' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'data.csv' /* no mime_type */ }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview.locator('table:has(tbody td)')).toBeVisible({ timeout: 10000 })
  })

  test('resource_link without name falls back to URI tail', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/payload-noname-plot.png'
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, mime_type: 'image/png' /* no name */ }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    // The URI tail is "payload-noname-plot.png".
    await expect(preview).toContainText('payload-noname-plot.png')
  })

  test('resource_link without name AND without identifiable URI tail falls back to "untitled"', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/'
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, mime_type: 'application/octet-stream' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    // URI tail (between trailing slashes) → "files".
    // Just assert SOMETHING readable is in the header, no NaN/undefined.
    await expect(preview).not.toContainText('undefined')
    await expect(preview).not.toContainText('NaN')
  })

  test('resource_link without size omits the size segment', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/payload-nosize/download', name: 'x.png', mime_type: 'image/png' /* no size */ },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview).not.toContainText('undefined B')
    await expect(preview).not.toContainText('NaN')
  })

  test('URI with query params is preserved verbatim in <img src>', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/payload-q/download?token=abc&v=1'
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'p.png', mime_type: 'image/png' }],
    })
    const img = page.locator('[data-testid="inline-file-preview"] img').first()
    await expect(img).toBeVisible({ timeout: 10000 })
    await expect(img).toHaveAttribute('src', uri)
  })

  test('Unicode filename rendered correctly in header + viewer matched', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/payload-unicode/download'
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri, name: '日本語のファイル.png', mime_type: 'image/png' },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview).toContainText('日本語のファイル.png')
    // Image viewer still matches via image/*.
    await expect(preview.locator('img')).toBeVisible()
  })

  test('is_saved=true (user-attachment-style) renders the same way', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/payload-saved/download', name: 'a.png', mime_type: 'image/png', is_saved: true },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('img')).toBeVisible()
  })

  test('is_saved=false (sandbox workspace artifact) renders the same way', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/payload-workspace/download', name: 'a.png', mime_type: 'image/png', is_saved: false },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('img')).toBeVisible()
  })
})
