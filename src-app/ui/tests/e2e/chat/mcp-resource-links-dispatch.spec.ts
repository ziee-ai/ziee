import { readFile } from 'node:fs/promises'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
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

// 1x1 transparent PNG — keeps ImageBody's <img> in the DOM (it
// transitions to a "Couldn't load image" placeholder on `onerror`,
// which removes the <img> the test asserts against).
const TINY_PNG = Buffer.from(
  '89504E470D0A1A0A0000000D49484452000000010000000108060000001F15C4890000000D49444154789C6200010000050001' +
    '0D0A2DB40000000049454E44AE426082',
  'hex',
)

/**
 * Pin the modular dispatch contract: ALL MIME-to-renderer dispatch
 * happens through `getViewer(name, mimeType)` and the viewer's
 * `entry.inline` declaration. The chat-side `MessageFilesView` /
 * `InlineFilePreview` never inspect MIME literals.
 *
 * If a future contributor sneaks a `mimeType.startsWith('image/')`
 * into the chat code, the last test in this file (grep-style
 * assertion) breaks.
 *
 * Per project [[file-viewer-modular-system]].
 */

test.describe('Inline file previews — modular dispatch contract', () => {
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

  test('image viewer (inline:true) renders body for image MIME', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/disp-img/download'
    await mockResourceLinkUrl(page, uri, TINY_PNG, { contentType: 'image/png' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'plot.png', mime_type: 'image/png' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('[data-testid="inline-file-preview-body"] img'))
      .toHaveAttribute('src', uri)
  })

  test('pdf viewer (no inline flag) falls back to header-only file card', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/disp-pdf/download', name: 'doc.pdf', mime_type: 'application/pdf' },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    // No expanded body since pdf viewer doesn't opt in.
    await expect(preview.locator('[data-testid="inline-file-preview-body"]'))
      .toHaveCount(0)
    // No chevron either — only render the chevron when the viewer has
    // an inline body to toggle.
    await expect(preview.locator('[data-testid="inline-file-preview-chevron"]'))
      .toHaveCount(0)
    // External URL-only link → open-in-new-tab is the fallback access path.
    await expect(preview.locator('[data-testid="inline-file-preview-open"]'))
      .toBeVisible()
  })

  test('tabular viewer (inline: csv/tsv subset) inlines csv, falls back for xlsx', async ({
    page,
    testInfra,
  }) => {
    const csvUri = '/api/files/disp-csv/download'
    const xlsxUri = '/api/files/disp-xlsx/download'
    await mockResourceLinkUrl(page, csvUri, 'a,b\n1,2\n', { contentType: 'text/csv' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: csvUri, name: 'data.csv', mime_type: 'text/csv' },
        { uri: xlsxUri, name: 'sheet.xlsx', mime_type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' },
      ],
    })
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(2, { timeout: 10000 })
    // CSV opted in via FileSupportEntry[] subset → body renders.
    // data-file-uri is on the preview element itself (not a child),
    // so combine the attributes in one selector rather than using
    // .filter({has}) which only sees descendants.
    const csvPreview = page.locator(`[data-testid="inline-file-preview"][data-file-uri="${csvUri}"]`)
    await expect(csvPreview.locator('[data-testid="inline-file-preview-body"]')).toBeVisible({ timeout: 10000 })
    // XLSX NOT in the inline subset → no body.
    const xlsxPreview = page.locator(`[data-testid="inline-file-preview"][data-file-uri="${xlsxUri}"]`)
    await expect(xlsxPreview.locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
  })

  test('viewer matched by ext (no mime_type) renders inline', async ({
    page,
    testInfra,
  }) => {
    // No mime_type — registry must dispatch via ext rules. CSV viewer's
    // supportedTypes includes `{ext:'csv'}` so it should match.
    const uri = '/api/files/disp-ext/download'
    await mockResourceLinkUrl(page, uri, 'a,b\n1,2\n', { contentType: 'text/plain' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'no-mime.csv' /* no mime_type */ }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('[data-testid="inline-file-preview-body"] [data-testid^="file-delimited-table-row-"]').first()).toBeVisible({ timeout: 10000 })
  })

  test('viewer matched by mime wildcard (image/heic) renders inline', async ({
    page,
    testInfra,
  }) => {
    // image/heic isn't a specific viewer, but the image viewer registers
    // `image/*` wildcard → should match.
    const uri = '/api/files/disp-heic/download'
    await mockResourceLinkUrl(page, uri, TINY_PNG, { contentType: 'image/heic' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'photo.heic', mime_type: 'image/heic' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('[data-testid="inline-file-preview-body"] img'))
      .toHaveAttribute('src', uri)
  })

  test('MIME with no matching viewer falls back to file card', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/disp-unknown/download', name: 'blob.xyz', mime_type: 'application/x-completely-unknown' },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
    await expect(preview.locator('[data-testid="inline-file-preview-open"]')).toBeVisible()
  })

  test("viewer's icon and label are rendered in the inline header", async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri: '/api/files/disp-img-2/download', name: 'p.png', mime_type: 'image/png' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    // Image viewer's label is "Image".
    await expect(preview).toContainText('Image')
    // The viewer's icon is rendered (AntD's PictureOutlined). Match by
    // presence of an aria-label or a known antd icon class anywhere
    // in the header.
    await expect(preview.locator('[data-testid="inline-file-preview-icon"]').first()).toBeVisible()
  })

  test('body in inline context receives the {source} variant of FileViewerSlotProps', async ({
    page,
    testInfra,
  }) => {
    // Verified indirectly: the image viewer's body renders <img src={url}>
    // ONLY in the source branch (file-mode uses thumbnailUrls from
    // FileStore). If body were called with {file} in inline context,
    // it would be missing from the cache → render <Spin/>, not <img>.
    const uri = '/api/files/disp-src/download'
    await mockResourceLinkUrl(page, uri, TINY_PNG, { contentType: 'image/png' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'p.png', mime_type: 'image/png' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    const img = preview.locator('[data-testid="inline-file-preview-body"] img')
    await expect(img).toHaveAttribute('src', uri)
    // No spinner — confirms the source branch fired (not the file
    // branch's thumbnailUrls path which would be empty).
    await expect(preview.getByRole('status')).toHaveCount(0)
  })

  test('chat code contains no hardcoded MIME literals', async () => {
    // Lock-in for the modular contract: the chat-side components must
    // never inspect MIME types directly. ALL dispatch goes through
    // `getViewer()` and each viewer module's `inline` declaration.
    const __filename = fileURLToPath(import.meta.url)
    const __dirname = dirname(__filename)
    // File chat-extension components moved to modules/file/chat-extension/
    // (commit ced3fcf, frontend extraction series). Keep the meta-test
    // pointing at the live source files so it continues to gate hardcoded
    // MIME literals at the right boundary.
    const root = resolve(
      __dirname,
      '..',
      '..',
      '..',
      'src',
      'modules',
      'file',
      'chat-extension',
      'components',
    )
    const files = ['MessageFilesView.tsx', 'InlineFilePreview.tsx']
    const banned = [
      "startsWith('image/",
      'startsWith("image/',
      "startsWith('text/",
      'startsWith("text/',
      "startsWith('application/",
      'startsWith("application/',
    ]
    for (const file of files) {
      const path = resolve(root, file)
      const contents = await readFile(path, 'utf-8')
      for (const needle of banned) {
        expect(
          contents.includes(needle),
          `${file} must not hardcode MIME prefixes — found "${needle}". Dispatch belongs in viewer modules, not chat code.`,
        ).toBe(false)
      }
    }
  })
})
