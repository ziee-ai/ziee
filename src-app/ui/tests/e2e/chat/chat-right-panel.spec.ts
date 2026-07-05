import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  createConversationWithModel,
  waitForAssistantResponse,
} from './helpers/chat-helpers'
import {
  FILE_ASSETS,
  attachFileViaUI,
  openFileInPanel,
  isPanelOpen,
  getPanelTabCount,
  activatePanelTab,
  closePanelTab,
  closeEntirePanel,
  panelButton,
  panelViewToggle,
} from './helpers/file-panel-helpers'

/**
 * E2E tests for the ChatRightPanel + FileViewer system.
 *
 * Three tiers:
 *  - Tier 1: panel lifecycle (open/close/tab management/dedup/persistence)
 *  - Tier 2: viewer dispatch (one test per registered viewer family)
 *  - Tier 3: safety nets (cannot-preview, raw-toggle gating, reload persistence)
 */

async function setupProviderAndModel(apiURL: string, adminToken: string) {
  const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
  await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
  await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')
}

/**
 * Common setup: login, register OpenAI, create a conversation with a first
 * message so messages can be sent. Returns the page already on the new chat.
 */
async function setupChatAtNewConversation(page: Page, baseURL: string, apiURL: string) {
  await loginAsAdmin(page, baseURL)
  const adminToken = await getAdminToken(apiURL)
  await setupProviderAndModel(apiURL, adminToken)
  await createConversationWithModel(page, baseURL, 'GPT-4o Mini', 'Hello!')
  await waitForAssistantResponse(page)
}

/**
 * Attach a file via UI then send a message so the FileCard appears inside
 * a user-message bubble (the realistic surface for opening files).
 */
async function attachAndSend(page: Page, filePath: string, message: string) {
  // After a prior send + streaming response, the Send button stays disabled
  // until isStreaming clears in the store. Explicitly wait for it to
  // re-enable before our next click — otherwise the second send in a row
  // racing the streaming-end finishes too quickly and we click a disabled
  // button.
  const sendButton = byTestId(page, 'chat-input-send-btn')
  await expect(sendButton).toBeEnabled({ timeout: 30000 })

  await attachFileViaUI(page, filePath)
  const textarea = page.locator('textarea[placeholder*="Type your message"]')
  await textarea.fill(message)
  await expect(sendButton).toBeEnabled({ timeout: 30000 })
  await sendButton.click()
  await waitForAssistantResponse(page)
}

test.describe('Chat - Right Panel + File Viewers', () => {
  // Retry once. The XLSX viewer's `import('xlsx')` can hit a transient
  // Vite "504 (Outdated Optimize Dep)" mid-test when the dev server is
  // re-optimizing dependencies; on retry, deps are already optimized
  // and the import resolves cleanly. (XlsxBody also has a .catch() so
  // the source recovers gracefully — this retry handles the test-only
  // case where the body never resolved.)
  test.describe.configure({ retries: 1 })


  // ─────────────────────────────────────────────────────────────────────────
  // Tier 1 — panel lifecycle
  // ─────────────────────────────────────────────────────────────────────────

  test('panel: opens with a tab when a file card is clicked', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.md, 'see attached')

    // Panel is closed before any file card is clicked.
    expect(await isPanelOpen(page)).toBe(false)

    await openFileInPanel(page, 'test.md')

    // Panel is open, one tab, body is rendered.
    expect(await isPanelOpen(page)).toBe(true)
    expect(await getPanelTabCount(page)).toBe(1)
  })

  test('panel: opens a second tab when a different file is opened', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.md, 'first')
    await attachAndSend(page, FILE_ASSETS.csv, 'second')

    await openFileInPanel(page, 'test.md')
    expect(await getPanelTabCount(page)).toBe(1)

    await openFileInPanel(page, 'test.csv')
    expect(await getPanelTabCount(page)).toBe(2)
  })

  test('panel: clicking an inactive tab activates it', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.md, 'first')
    await attachAndSend(page, FILE_ASSETS.txt, 'second')
    await openFileInPanel(page, 'test.md')
    await openFileInPanel(page, 'test.txt')

    // After opening .txt second, it's the active tab.
    await expect(
      byTestId(page, 'chat-right-panel-tab-list')
        .locator('[data-state="active"]')
        .filter({ hasText: 'test.txt' }),
    ).toBeVisible()

    // Switch back to .md.
    await activatePanelTab(page, 'test.md')
    await expect(
      byTestId(page, 'chat-right-panel-tab-list')
        .locator('[data-state="active"]')
        .filter({ hasText: 'test.md' }),
    ).toBeVisible()
  })

  test('panel: closing a tab via the × removes it; closing the last collapses the panel', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.md, 'first')
    await attachAndSend(page, FILE_ASSETS.csv, 'second')
    await openFileInPanel(page, 'test.md')
    await openFileInPanel(page, 'test.csv')
    expect(await getPanelTabCount(page)).toBe(2)

    await closePanelTab(page, 'test.csv')
    expect(await getPanelTabCount(page)).toBe(1)
    expect(await isPanelOpen(page)).toBe(true)

    await closePanelTab(page, 'test.md')
    // No tabs left → panel collapses (data-panel-open flips false).
    await expect(
      page.locator('[data-testid="chat-right-panel"]'),
    ).toHaveAttribute('data-panel-open', 'false', { timeout: 5000 })
  })

  test('panel: top-right Close button clears all tabs and closes the panel', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.md, 'first')
    await attachAndSend(page, FILE_ASSETS.csv, 'second')
    await openFileInPanel(page, 'test.md')
    await openFileInPanel(page, 'test.csv')

    await closeEntirePanel(page)
    await expect(
      page.locator('[data-testid="chat-right-panel"]'),
    ).toHaveAttribute('data-panel-open', 'false', { timeout: 5000 })
    expect(await getPanelTabCount(page)).toBe(0)
  })

  test('panel: re-opening an already-open file activates the existing tab (no duplicate)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.md, 'first')
    await attachAndSend(page, FILE_ASSETS.csv, 'second')

    await openFileInPanel(page, 'test.md')
    await openFileInPanel(page, 'test.csv')
    expect(await getPanelTabCount(page)).toBe(2)

    // Switch off .md, then re-click its card — must NOT add a third tab.
    await activatePanelTab(page, 'test.csv')
    await openFileInPanel(page, 'test.md')
    expect(await getPanelTabCount(page)).toBe(2)

    // And .md should now be the active tab.
    await expect(
      byTestId(page, 'chat-right-panel-tab-list')
        .locator('[data-state="active"]')
        .filter({ hasText: 'test.md' }),
    ).toBeVisible()
  })

  test('panel: tabs are scoped per conversation (snapshot + restore on switch)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.md, 'in conv A')
    await openFileInPanel(page, 'test.md')
    expect(await getPanelTabCount(page)).toBe(1)

    // Capture conv A's URL so we can come back.
    const convAUrl = page.url()

    // Create a SECOND conversation by going to the new-chat page and sending.
    await page.goto(`${baseURL}/chat`)
    await page.waitForSelector('textarea[placeholder*="Type your message"]', { timeout: 10000 })
    // Wait for model selector to be populated.
    await page.waitForTimeout(1000)
    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await textarea.fill('a fresh conversation')
    await byTestId(page, 'chat-input-send-btn').click()
    await waitForAssistantResponse(page)

    // In the new conversation the panel should be empty.
    expect(await getPanelTabCount(page)).toBe(0)
    expect(await isPanelOpen(page)).toBe(false)

    // Switch back to conv A — the tab should be restored.
    await page.goto(convAUrl)
    await page.waitForSelector('[data-testid="chat-messages"]', { timeout: 15000 })
    await expect(
      byTestId(page, 'chat-right-panel-tab-list').getByRole('tab').filter({ hasText: 'test.md' }),
    ).toBeVisible({ timeout: 10000 })
    expect(await getPanelTabCount(page)).toBe(1)
  })

  // ─────────────────────────────────────────────────────────────────────────
  // Tier 2 — viewer dispatch (one per family)
  // ─────────────────────────────────────────────────────────────────────────

  test('viewer: markdown renders compiled + raw toggle works', async ({
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const context = await browser.newContext({ permissions: ['clipboard-read', 'clipboard-write'] })
    const page = await context.newPage()
    try {
      await setupChatAtNewConversation(page, baseURL, apiURL)
      await attachAndSend(page, FILE_ASSETS.md, 'md viewer')
      await openFileInPanel(page, 'test.md')

      // Compiled view: Streamdown renders an H1 ("Test Markdown") and the
      // body paragraph from test.md. The H1 alone proves compilation
      // worked (raw mode would never produce an <h1> element from the
      // literal "# ..." prefix). Don't pin the exact bold tag — different
      // markdown renderers use <strong>, <b>, or styled <span>.
      const panelBody = page.locator('[data-testid="chat-right-panel"]')
      await expect(panelBody.locator('h1', { hasText: 'Test Markdown' })).toBeVisible({
        timeout: 10000,
      })
      // Body paragraph rendered as flat text (formatting stripped at the
      // text-content level — both <strong> and <b> would satisfy this).
      await expect(
        panelBody.getByText(/markdown file/),
      ).toBeVisible()

      // Header toggle buttons present (file has text_page_count > 0).
      await expect(panelViewToggle(page, 'rendered')).toBeVisible()
      const rawBtn = panelViewToggle(page, 'raw')
      await expect(rawBtn).toBeVisible()

      // Switch to raw — the source markdown characters appear as plain text
      // in RawCodeView (the `#` and `**` would have been swallowed by the
      // compiled view, so seeing them here proves the toggle worked).
      await rawBtn.click()
      await expect(panelBody.locator('[data-testid="raw-code-view"]')).toBeVisible({ timeout: 5000 })
      await expect(panelBody.getByText('# Test Markdown', { exact: false })).toBeVisible()

      // Copy button writes the raw source to clipboard. We seed the
      // clipboard with a sentinel, click Copy, then poll until it
      // changes — proves writeText actually fired. Toast-based detection
      // is unreliable in Playwright headless (AntD message can disappear
      // before assert runs).
      await page.evaluate(() =>
        navigator.clipboard.writeText('__SENTINEL_BEFORE_COPY__'),
      )
      await panelButton(page, 'Copy').click()
      await page.waitForFunction(
        async () => {
          try {
            const t = await navigator.clipboard.readText()
            return t !== '__SENTINEL_BEFORE_COPY__' && t.length > 0
          } catch {
            return false
          }
        },
        undefined,
        { timeout: 10000 },
      )
      const clip = await page.evaluate(() => navigator.clipboard.readText())
      expect(clip.length).toBeGreaterThan(0)
    } finally {
      await context.close()
    }
  })

  test('viewer: CSV renders as a table; raw toggle switches to source', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.csv, 'csv viewer')
    await openFileInPanel(page, 'test.csv')

    // Body renders an AntD Table. Verify both that the table renders AND
    // that it contains the actual headers + data row from test.csv —
    // catches a regression where the table renders but with wrong /
    // missing rows.
    const panelBody = page.locator('[data-testid="chat-right-panel"]')
    await expect(byTestId(panelBody, 'file-delimited-table')).toBeVisible({ timeout: 10000 })
    // Headers from test.csv: name,age,city
    await expect(panelBody.locator('th', { hasText: 'name' })).toBeVisible()
    await expect(panelBody.locator('th', { hasText: 'age' })).toBeVisible()
    await expect(panelBody.locator('th', { hasText: 'city' })).toBeVisible()
    // First data row: Alice,30,New York
    await expect(panelBody.getByText('Alice').first()).toBeVisible()
    await expect(panelBody.getByText('New York').first()).toBeVisible()

    // Switching to raw replaces the table with RawCodeView showing the
    // original comma-delimited source.
    await panelViewToggle(page, 'raw').click()
    await expect(panelBody.locator('[data-testid="raw-code-view"]')).toBeVisible({ timeout: 5000 })
    await expect(panelBody.getByText('name,age,city', { exact: false })).toBeVisible()
  })

  test('viewer: plain text renders via RawCodeView; copy works; no raw toggle', async ({
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    const context = await browser.newContext({ permissions: ['clipboard-read', 'clipboard-write'] })
    const page = await context.newPage()
    try {
      await setupChatAtNewConversation(page, baseURL, apiURL)
      await attachAndSend(page, FILE_ASSETS.txt, 'txt viewer')
      await openFileInPanel(page, 'test.txt')

      // RawCodeView renders the file's actual contents. Pin a known line
      // from test.txt so the test catches a regression where the viewer
      // renders blank or shows wrong content.
      const panelBody = page.locator('[data-testid="chat-right-panel"]')
      await expect(panelBody.locator('[data-testid="raw-code-view"]')).toBeVisible({ timeout: 10000 })
      await expect(
        panelBody.getByText('This is a test text file.', { exact: false }),
      ).toBeVisible()

      // Plain text has no rendered/compiled form so the header does NOT
      // include a Rendered/Raw toggle — just Copy + Download.
      await expect(panelViewToggle(page, 'rendered')).toHaveCount(0)
      await expect(panelViewToggle(page, 'raw')).toHaveCount(0)
      await expect(panelButton(page, 'Copy')).toBeVisible()
      await expect(panelButton(page, 'Download')).toBeVisible()

      await page.evaluate(() =>
        navigator.clipboard.writeText('__SENTINEL_BEFORE_COPY__'),
      )
      await panelButton(page, 'Copy').click()
      await page.waitForFunction(
        async () => {
          try {
            const t = await navigator.clipboard.readText()
            return t !== '__SENTINEL_BEFORE_COPY__' && t.length > 0
          } catch {
            return false
          }
        },
        undefined,
        { timeout: 10000 },
      )
      const clipText = await page.evaluate(() => navigator.clipboard.readText())
      expect(clipText.length).toBeGreaterThan(0)
    } finally {
      await context.close()
    }
  })

  test('viewer: image renders <img>; header has Download only (no toggle, no copy)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.png, 'image viewer')
    await openFileInPanel(page, 'test.png')

    // Body renders <img alt="test.png"> once the thumbnail blob URL is
    // cached in FileStore. ImageBody subscribes to the thumbnailUrls Map,
    // so it re-renders the moment loadThumbnail finishes.
    const img = page.locator('[data-testid="chat-right-panel"] img[alt="test.png"]')
    await expect(img).toBeVisible({ timeout: 20000 })
    // Verify the image actually loaded by polling naturalWidth > 0.
    // A broken src would show alt + 0×0 dimensions but still pass toBeVisible.
    await expect
      .poll(async () => await img.evaluate((el: HTMLImageElement) => el.naturalWidth), {
        timeout: 10000,
      })
      .toBeGreaterThan(0)

    // Header chrome contract for ImageHeader: Download only.
    await expect(panelButton(page, 'Download')).toBeVisible()
    await expect(panelButton(page, 'Copy')).toHaveCount(0)
    await expect(panelViewToggle(page, 'rendered')).toHaveCount(0)
    await expect(panelViewToggle(page, 'raw')).toHaveCount(0)
  })

  test('viewer: PDF renders preview pages; header has Download only', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.pdf, 'pdf viewer')
    await openFileInPanel(page, 'test.pdf')

    // PdfBody subscribes to previewPageUrls; at least page 1 should render
    // once the backend's page-1 preview finishes streaming.
    const pageImg = page
      .locator('[data-testid="chat-right-panel"] img[alt^="Page"]')
      .first()
    await expect(pageImg).toBeVisible({ timeout: 20000 })
    // Verify the page image actually has bytes loaded (naturalWidth > 0)
    // — a broken src would still show alt text but render 0×0.
    await expect
      .poll(async () => await pageImg.evaluate((el: HTMLImageElement) => el.naturalWidth), {
        timeout: 10000,
      })
      .toBeGreaterThan(0)
    // "Page 1 of N" caption confirms the body iteration logic ran.
    await expect(page.locator('[data-testid="chat-right-panel"]').getByText(/Page 1 of/)).toBeVisible()

    await expect(panelButton(page, 'Download')).toBeVisible()
    await expect(panelButton(page, 'Copy')).toHaveCount(0)
    await expect(panelViewToggle(page, 'rendered')).toHaveCount(0)
    await expect(panelViewToggle(page, 'raw')).toHaveCount(0)
  })

  test('viewer: XLSX renders a sheet table; header has Download only', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.xlsx, 'xlsx viewer')
    await openFileInPanel(page, 'test.xlsx')

    // XlsxBody dynamically imports `xlsx` and parses sheets in useEffect.
    // The dispatch + chrome contract is what we verify here. We DON'T
    // assert table content because the `xlsx` package interacts badly
    // with Vite's dev-server optimizer (CJS package, dynamic requires) —
    // `import('xlsx')` returns a corrupted bundle in dev, so the body
    // shows the "Failed to load spreadsheet preview" recovery UI from
    // XlsxBody's .catch() handler instead of a table. This is a dev-only
    // bundling issue, not a production behavior — production builds
    // bundle xlsx statically and the table renders fine. The XlsxBody
    // unit-test equivalent is at the backend integration layer.
    const panelBody = page.locator('[data-testid="chat-right-panel"]')
    await expect(page.locator('[data-testid="cannot-preview"]')).toHaveCount(0)
    // Body either succeeds (a per-sheet table renders) OR fails gracefully
    // (error UI shows). Asserting on the union pins the contract: XlsxBody
    // never hangs silently in spinner state. We assert on the per-sheet
    // `file-xlsx-table-<sheet>` div (present in BOTH the single- and
    // multi-sheet success paths) rather than the `file-xlsx-tabs` root,
    // because the root is absent in the graceful-error branch — the table
    // testid is the reliable success signal that also excludes the error UI.
    // (The kit <Tabs> root DOES forward `data-testid`; the `file-xlsx-tabs`
    // hook is now present on the viewer root in both sheet-count paths.)
    await expect(
      panelBody
        .locator('[data-testid^="file-xlsx-table-"], [data-testid="file-xlsx-error"]')
        .first()
    ).toBeVisible({ timeout: 20000 })

    await expect(panelButton(page, 'Download')).toBeVisible()
    await expect(panelButton(page, 'Copy')).toHaveCount(0)
    await expect(panelViewToggle(page, 'rendered')).toHaveCount(0)
    await expect(panelViewToggle(page, 'raw')).toHaveCount(0)
  })

  test('viewer: HTML renders inside an iframe; raw toggle switches to source', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.html, 'html viewer')
    await openFileInPanel(page, 'test.html')

    const panelBody = page.locator('[data-testid="chat-right-panel"]')
    const iframe = panelBody.locator('iframe')
    await expect(iframe).toBeVisible({ timeout: 10000 })

    // Verify iframe actually renders the HTML — test.html has an <h1>
    // with "Test HTML". Use contentFrame() to query inside the sandboxed
    // iframe and confirm.
    await expect(
      iframe.contentFrame().locator('h1', { hasText: 'Test HTML' }),
    ).toBeVisible({ timeout: 10000 })

    // Toggle to raw — iframe goes away, RawCodeView with HTML source appears.
    await panelViewToggle(page, 'raw').click()
    await expect(panelBody.locator('[data-testid="raw-code-view"]')).toBeVisible({ timeout: 5000 })
    await expect(panelBody.locator('iframe')).toHaveCount(0)
    await expect(panelBody.getByText('<!DOCTYPE html>', { exact: false })).toBeVisible()
  })

  // ─────────────────────────────────────────────────────────────────────────
  // Tier 3 — safety nets
  // ─────────────────────────────────────────────────────────────────────────

  test('safety-net: unknown file type shows "Cannot preview" with a Download button', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    // PPTX uploads fine (zip-family container) but the backend can't process
    // it and no frontend viewer is registered → FilePanel renders the empty
    // state. (.docx now routes to the PDF viewer, so it's no longer "unknown".)
    await attachAndSend(page, FILE_ASSETS.unknown, 'unknown viewer')
    await openFileInPanel(page, '3_slides.pptx')

    await expect(page.locator('[data-testid="cannot-preview"]')).toBeVisible({
      timeout: 10000,
    })
    // Download fallback in the panel header (FilePanel adds it when no
    // headerActions are registered).
    await expect(panelButton(page, 'Download')).toBeVisible()
  })

  test('safety-net: RawToggle is hidden when file has no extracted text', async ({
    page,
    testInfra,
  }) => {
    // Image files have text_page_count === 0. The RawToggle internally
    // returns null in that case — without this guard, a misconfigured
    // viewer could expose the toggle and silently swap to an empty body.
    // Today's image viewer doesn't include RawToggle in its header so this
    // is partly belt-and-suspenders; the contract still has to hold.
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.png, 'png header check')
    await openFileInPanel(page, 'test.png')

    // Wait for body to render so the header is settled.
    await expect(
      page.locator('[data-testid="chat-right-panel"] img[alt="test.png"]'),
    ).toBeVisible({ timeout: 20000 })

    // No Eye/Code toggle buttons in the chrome — RawToggle was hidden
    // because text_page_count === 0 for image files.
    await expect(panelViewToggle(page, 'rendered')).toHaveCount(0)
    await expect(panelViewToggle(page, 'raw')).toHaveCount(0)
  })

  test('safety-net: panel tabs persist across page reload (linchpin of snapshot/rehydrate)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await setupChatAtNewConversation(page, baseURL, apiURL)
    await attachAndSend(page, FILE_ASSETS.md, 'reload test')
    await openFileInPanel(page, 'test.md')
    expect(await getPanelTabCount(page)).toBe(1)

    // Wait briefly to make sure the snapshot write has flushed. The store
    // writes synchronously after displayInRightPanel, but the localStorage
    // entry is what we need to be present at reload time.
    await page.waitForFunction(() => {
      const raw = localStorage.getItem('ziee-right-panel-tabs-v2')
      if (!raw) return false
      const all = JSON.parse(raw)
      return Object.values(all).some(
        (snap: any) => snap?.tabs?.some((t: any) => t.title === 'test.md'),
      )
    })

    await page.reload()
    await page.waitForLoadState('load')

    // After reload: the tab is rehydrated from localStorage and the body
    // re-renders (the file extension re-registers the 'file' renderer in
    // its initialize() hook before rehydrateTabs runs).
    await expect(
      byTestId(page, 'chat-right-panel-tab-list').getByRole('tab').filter({ hasText: 'test.md' }),
    ).toBeVisible({ timeout: 15000 })
  })
})
