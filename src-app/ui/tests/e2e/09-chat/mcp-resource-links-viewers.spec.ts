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
 * Per-viewer rendering tests. Each section asserts the production
 * viewer (image, markdown, tabular, text) actually renders correctly
 * via the chat-inline dispatch path. PDF / web / unknown verified as
 * file-card fallback.
 */

test.describe('Inline file previews — per-viewer rendering', () => {
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

  // ── image ─────────────────────────────────────────────────────────────────

  test('image: renders <img> with the resource_link URL', async ({ page, testInfra }) => {
    const uri = '/api/files/img-basic/download'
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'plot.png', mime_type: 'image/png' }],
    })
    const img = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"] img')
      .first()
    await expect(img).toBeVisible({ timeout: 10000 })
    await expect(img).toHaveAttribute('src', uri)
  })

  test('image: handles image/jpeg, image/webp, image/gif', async ({
    page,
    testInfra,
  }) => {
    // Note: image/svg+xml is intentionally claimed by the `web/`
    // viewer (priority-0 exact match overrides the `image/*`
    // wildcard at priority 10). The web viewer doesn't opt in to
    // inline rendering → SVG falls back to a file-card. So SVG is
    // excluded from this loop.
    const cases = [
      { name: 'a.jpg', mime_type: 'image/jpeg' },
      { name: 'b.webp', mime_type: 'image/webp' },
      { name: 'c.gif', mime_type: 'image/gif' },
    ]
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: cases.map((c, i) => ({
        uri: `/api/files/img-multi-${i}/download`,
        ...c,
      })),
    })
    const previews = page.locator('[data-testid="inline-file-preview"]')
    await expect(previews).toHaveCount(cases.length, { timeout: 10000 })
    for (let i = 0; i < cases.length; i++) {
      const uri = `/api/files/img-multi-${i}/download`
      await expect(previews.nth(i).locator('img')).toHaveAttribute('src', uri)
    }
  })

  test('image: applies max-width clamp via CSS', async ({ page, testInfra }) => {
    const uri = '/api/files/img-clamp/download'
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'big.png', mime_type: 'image/png' }],
    })
    const img = page.locator('[data-testid="inline-file-preview"] img').first()
    await expect(img).toBeVisible({ timeout: 10000 })
    // Inline body wraps <img> with `max-w-full max-h-[400px]` tailwind.
    // Assert via computed style.
    const maxHeight = await img.evaluate(el => window.getComputedStyle(el).maxHeight)
    expect(maxHeight).toBe('400px')
  })

  test('image: 404 URL does not crash the page; <img> renders broken', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/img-404/download'
    await mockResourceLinkUrl(page, uri, 'not found', { status: 404 })
    const pageErrors: string[] = []
    page.on('pageerror', e => pageErrors.push(e.message))
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'gone.png', mime_type: 'image/png' }],
    })
    const img = page.locator('[data-testid="inline-file-preview"] img').first()
    await expect(img).toBeVisible({ timeout: 10000 })
    expect(pageErrors).toEqual([])
  })

  // ── markdown ──────────────────────────────────────────────────────────────

  test('markdown: renders headings/bold/lists from fetched markdown', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/md-basic/download'
    await mockResourceLinkUrl(
      page,
      uri,
      '# Title\n\nThis is **bold** text.\n\n- item one\n- item two\n',
      { contentType: 'text/markdown' },
    )
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'README.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    // Streamdown 2 wraps **bold** as <span data-streamdown="strong">
    // (not <strong>), unordered list as <ul data-streamdown="unordered-list">.
    await expect(body.locator('h1')).toHaveText('Title', { timeout: 10000 })
    await expect(body.locator('[data-streamdown="strong"]')).toHaveText('bold')
    await expect(body.locator('[data-streamdown="list-item"]')).toHaveCount(2)
  })

  test('markdown: mermaid block inside fetched markdown renders without SVG', async ({
    page,
    testInfra,
  }) => {
    // Streamdown 2 unbundled mermaid; we don't install
    // `@streamdown/mermaid` (per [[project_no_katex_remark_rehype]]).
    // Without the plugin, Streamdown throws on the `mermaid` fence and
    // the StreamdownErrorBoundary catches it, falling back to a plain
    // `<pre data-testid="streamdown-fallback">`. Either path is
    // acceptable for the file-viewer — the contract this test pins is
    // (a) the body still renders (no crash leaks past the boundary),
    // (b) the raw markdown content is visible to the user, and
    // (c) no SVG is rendered (mermaid stays unrendered).
    const uri = '/api/files/md-mermaid/download'
    await mockResourceLinkUrl(
      page,
      uri,
      '```mermaid\ngraph LR\n  A-->B\n```\n',
      { contentType: 'text/markdown' },
    )
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'diagram.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toBeVisible({ timeout: 10000 })
    // Raw fence content is shown (either rendered as a code-block or
    // as the streamdown-fallback <pre>, depending on whether the lazy
    // plugin chunk crashed or not).
    await expect(body).toContainText('graph LR')
    expect(await body.locator('svg').count()).toBe(0)
  })

  test('markdown: raw <script> in fetched content does not execute', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/md-xss/download'
    await mockResourceLinkUrl(
      page,
      uri,
      'Before\n\n<script>window.MD_PWNED = true</script>\n\nAfter\n',
      { contentType: 'text/markdown' },
    )
    await page.addInitScript(() => {
      ;(window as any).MD_PWNED = false
    })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'xss.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText('Before')
    const pwned = await page.evaluate(() => (window as any).MD_PWNED)
    expect(pwned).toBe(false)
  })

  test('markdown: 404 URL shows inline error, not infinite spinner', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/md-404/download'
    await mockResourceLinkUrl(page, uri, 'not found', { status: 404 })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'missing.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText(/failed to load/i, { timeout: 10000 })
    await expect(body.locator('.ant-spin')).toHaveCount(0)
  })

  // ── tabular (CSV / TSV) ───────────────────────────────────────────────────

  test('csv: renders as table with correct row count', async ({ page, testInfra }) => {
    const uri = '/api/files/csv-basic/download'
    await mockResourceLinkUrl(page, uri, 'a,b\n1,2\n3,4\n', { contentType: 'text/csv' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'data.csv', mime_type: 'text/csv' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body.locator('table:has(tbody td)')).toBeVisible({ timeout: 10000 })
    // AntD Table v6 renders a measure row + dual <table> elements
    // for fixed-header scroll. Use AntD's `.ant-table-row` class
    // which only marks actual data rows.
    expect(await body.locator('.ant-table-row').count()).toBe(2)
  })

  test('tsv: renders as table', async ({ page, testInfra }) => {
    const uri = '/api/files/tsv-basic/download'
    await mockResourceLinkUrl(page, uri, 'a\tb\n1\t2\n', { contentType: 'text/tab-separated-values' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'data.tsv', mime_type: 'text/tab-separated-values' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body.locator('table:has(tbody td)')).toBeVisible({ timeout: 10000 })
    expect(await body.locator('.ant-table-row').count()).toBe(1)
  })

  test('csv with quoted commas inside fields preserves them', async ({
    page,
    testInfra,
  }) => {
    // Note: DelimitedTable renders the header row as the FIRST <tbody> row
    // (the column headers go in the antd <thead>; data rows go in <tbody>).
    // For `name,desc\n"Smith, J.",bio` that means 1 tbody row, with the
    // "Smith, J." cell preserving the embedded comma.
    const uri = '/api/files/csv-quoted/download'
    await mockResourceLinkUrl(
      page,
      uri,
      'name,desc\n"Smith, J.",bio\n',
      { contentType: 'text/csv' },
    )
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'q.csv', mime_type: 'text/csv' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    const rows = body.locator('.ant-table-row')
    await expect(rows).toHaveCount(1, { timeout: 10000 })
    // First column of the first data row contains the quoted-comma name.
    await expect(rows.first().locator('td').first()).toHaveText('Smith, J.')
  })

  test('xlsx mimetype does NOT inline render', async ({ page, testInfra }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        {
          uri: '/api/files/xlsx-1/download',
          name: 'sheet.xlsx',
          mime_type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
        },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
  })

  // ── text ──────────────────────────────────────────────────────────────────

  test('text: plain text renders as line-based preformatted view', async ({
    page,
    testInfra,
  }) => {
    // RawCodeView splits into per-line `<div>` elements inside a
    // `whitespace-pre` container (NOT a `<pre>` element). Assert via
    // the container's class and on visible text content.
    const uri = '/api/files/txt-basic/download'
    await mockResourceLinkUrl(page, uri, 'line 1\nline 2\n', { contentType: 'text/plain' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'notes.txt', mime_type: 'text/plain' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText('line 1', { timeout: 10000 })
    await expect(body).toContainText('line 2')
    await expect(body.locator('.whitespace-pre').first()).toBeVisible()
  })

  test('text: code MIME (text/javascript via .js ext) renders via text viewer', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/txt-code/download'
    await mockResourceLinkUrl(
      page,
      uri,
      'function foo() { return 42 }\n',
      { contentType: 'text/javascript' },
    )
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'foo.js', mime_type: 'text/javascript' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText('function foo', { timeout: 10000 })
    await expect(body.locator('.whitespace-pre').first()).toBeVisible()
  })

  // ── pdf / web / unknown (fallback) ────────────────────────────────────────

  test('pdf: falls back to header-only file card', async ({ page, testInfra }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/pdf-1/download', name: 'doc.pdf', mime_type: 'application/pdf' },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
    await expect(preview.locator('iframe')).toHaveCount(0)
    await expect(preview.locator('embed')).toHaveCount(0)
    await expect(preview.locator('[data-testid="inline-file-preview-open"]')).toBeVisible()
  })

  test('html (web viewer): falls back to header-only file card', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/html-1/download', name: 'page.html', mime_type: 'text/html' },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
    await expect(preview.locator('iframe')).toHaveCount(0)
  })

  test('unknown MIME: falls back to header-only file card', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [
        { uri: '/api/files/unk-1/download', name: 'blob.bin', mime_type: 'application/octet-stream' },
      ],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
    await expect(preview.locator('[data-testid="inline-file-preview-open"]')).toBeVisible()
  })
})
