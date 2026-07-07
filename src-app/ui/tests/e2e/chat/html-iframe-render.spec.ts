import type { Page } from '@playwright/test'
import { byTestId } from '../testid'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import {
  mockChatStream,
  startedEvent,
  textDeltaEvent,
  completeEvent,
  mockGetMessages,
  mockUserMessage,
  type MockMessageWithContent,
} from '../helpers/sse-mock-helpers'

/**
 * HTML-block sandboxed-iframe render spec (feature: html-iframe-render).
 *
 * A fenced ```html code block in an assistant message gets a Code | Preview
 * toggle. CODE is the DEFAULT (safety); PREVIEW renders the HTML inside a
 * strictly-sandboxed iframe. These specs pin the behavior AND the security
 * posture — most importantly that a script inside the preview cannot reach the
 * parent page (null-origin isolation).
 *
 * Seeding mirrors markdown-rendering.spec.ts: mock the SSE stream + the
 * post-stream /messages reload — no real LLM.
 */

const assistantTextMessage = (id: string, text: string): MockMessageWithContent => ({
  id,
  role: 'assistant',
  contents: [{ content_type: 'text', content: { type: 'text', text } }],
})

async function seedAssistantWithText(page: Page, baseURL: string, markdown: string) {
  await mockChatStream(page, [
    [
      startedEvent({ userMessageId: 'umsg_html_1' }),
      textDeltaEvent({ delta: markdown, messageId: 'amsg_html_1' }),
      completeEvent(),
    ],
  ])
  await mockGetMessages(page, [
    mockUserMessage({ id: 'umsg_html_1', text: 'render html please' }),
    assistantTextMessage('amsg_html_1', markdown),
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')

  const textarea = byTestId(page, 'chat-message-textarea').first()
  await textarea.fill('render html please')
  await byTestId(page, 'chat-input-send-btn').click()

  await expect(
    page.locator('[data-testid="chat-message"][data-role="assistant"]').last(),
  ).toBeVisible({ timeout: 15000 })
}

const assistantBubble = (page: Page) =>
  page.locator('[data-testid="chat-message"][data-role="assistant"]').last()

// A representative HTML doc whose inline <script> (a) writes a sentinel into its
// own DOM — a POSITIVE control proving the script actually EXECUTED (not merely
// that static markup painted), and (b) attempts to escape the sandbox. Used by
// the isolation test.
const PWN_HTML = [
  '<!doctype html>',
  '<html><head><title>t</title></head><body>',
  '<h1 id="hh">static-markup</h1>',
  '<script>',
  '  // positive control: only a RUNNING script can flip this text.',
  '  document.getElementById("hh").textContent = "SCRIPT_EXECUTED"',
  '  try { window.parent.__HTML_PWNED = true } catch (e) {}',
  '  try { top.__HTML_PWNED = true } catch (e) {}',
  '  try { top.location = "https://evil.test/" } catch (e) {}',
  '</script>',
  '</body></html>',
].join('\n')

const fence = (html: string) => '```html\n' + html + '\n```'

test.describe('Tier 3 — HTML block sandboxed-iframe render', () => {
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

  // TEST-1: defaults to CODE view (no iframe), toggle present, source visible.
  test('html fence defaults to CODE view (source shown, no iframe)', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      fence('<div>Hello <b>world</b></div>'),
    )
    const bubble = assistantBubble(page)
    const block = bubble.locator('[data-testid="html-block"]')
    await expect(block).toBeVisible({ timeout: 10000 })
    // Toggle present (the affordance).
    await expect(block.locator('[data-testid="html-block-toggle"]')).toBeVisible()
    // Default is the SOURCE view — no iframe rendered yet.
    await expect(block.locator('[data-testid="html-block-source"]')).toContainText(
      'Hello',
    )
    expect(await block.locator('iframe').count()).toBe(0)
  })

  // TEST-2: Preview mounts a sandboxed iframe with exactly `allow-scripts`.
  test('toggling Preview mounts a sandboxed iframe (allow-scripts only)', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      fence('<p>preview me</p>'),
    )
    const bubble = assistantBubble(page)
    const block = bubble.locator('[data-testid="html-block"]')
    await block.locator('[data-testid="html-block-toggle-opt-preview"]').click()

    const iframe = block.locator('[data-testid="html-block-preview"]')
    await expect(iframe).toBeVisible({ timeout: 5000 })
    const sandbox = await iframe.getAttribute('sandbox')
    expect(sandbox).toBe('allow-scripts')
    expect(sandbox).not.toContain('allow-same-origin')
    expect(sandbox).not.toContain('allow-top-navigation')
    expect(sandbox).not.toContain('allow-popups')
    expect(sandbox).not.toContain('allow-forms')
    // srcdoc populated with the HTML.
    const srcdoc = await iframe.getAttribute('srcdoc')
    expect(srcdoc).toContain('preview me')
  })

  // TEST-3: the injected CSP actually BLOCKS external network — proven by the
  // block observed FROM INSIDE the null-origin frame (the browser's own verdict
  // on whether the external image loaded), independent of Playwright internals.
  test('preview CSP blocks external network (external image blocked in-frame)', async ({
    page,
    testInfra,
  }) => {
    const SENTINEL = 'blocked-sentinel-xyz'
    // A valid 1×1 PNG. This route makes ANY request that actually reaches the
    // network succeed — so if the CSP FAILED to block, the img would LOAD
    // (IMG_LOADED). A CSP-blocked request never reaches the route (blocked in
    // the renderer before dispatch), so the img errors instead → IMG_BLOCKED
    // unambiguously means the CSP severed the external load (not a coincidental
    // network/DNS failure).
    const PNG = Buffer.from(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
      'base64',
    )
    await page.route(`**/*${SENTINEL}*`, route =>
      route.fulfill({ status: 200, contentType: 'image/png', body: PNG }),
    )
    const html = [
      '<!doctype html><html><body>',
      '<h1 id="img">pending</h1>',
      // External image — img-src is data: only, so the CSP must block this http
      // load. onerror running ALSO proves the frame executed (no vacuous pass).
      `<img onload="document.getElementById('img').textContent='IMG_LOADED'"`,
      `     onerror="document.getElementById('img').textContent='IMG_BLOCKED'"`,
      `     src="http://${SENTINEL}.example.com/beacon.png">`,
      '</body></html>',
    ].join('\n')
    await seedAssistantWithText(page, testInfra.baseURL, fence(html))
    const bubble = assistantBubble(page)
    const block = bubble.locator('[data-testid="html-block"]')
    await block.locator('[data-testid="html-block-toggle-opt-preview"]').click()
    const iframe = block.locator('[data-testid="html-block-preview"]')
    await expect(iframe).toBeVisible({ timeout: 5000 })

    // Mechanism present …
    const srcdoc = (await iframe.getAttribute('srcdoc')) || ''
    expect(srcdoc).toContain('Content-Security-Policy')
    expect(srcdoc).toContain("default-src 'none'")
    expect(await iframe.getAttribute('referrerpolicy')).toBe('no-referrer')

    // … and effect, observed from INSIDE the null-origin frame: the external
    // image was blocked by the CSP (would be IMG_LOADED if the CSP failed).
    const frame = page.frameLocator('[data-testid="html-block-preview"]')
    await expect(frame.locator('#img')).toHaveText('IMG_BLOCKED', { timeout: 5000 })
  })

  // TEST-4: SECURITY — sandbox script runs but cannot reach the parent.
  test('sandboxed script cannot reach the parent page', async ({
    page,
    testInfra,
  }) => {
    await page.addInitScript(() => {
      ;(window as any).__HTML_PWNED = false
    })
    await seedAssistantWithText(page, testInfra.baseURL, fence(PWN_HTML))
    const bubble = assistantBubble(page)
    const block = bubble.locator('[data-testid="html-block"]')
    await block.locator('[data-testid="html-block-toggle-opt-preview"]').click()
    const iframe = block.locator('[data-testid="html-block-preview"]')
    await expect(iframe).toBeVisible({ timeout: 5000 })

    // POSITIVE CONTROL — prove the inline <script> actually EXECUTED (not merely
    // that static markup painted): the script rewrites #hh to "SCRIPT_EXECUTED".
    // If a regression dropped `allow-scripts`, this stays "static-markup" and the
    // test FAILS here — so the isolation assertion below can't pass vacuously.
    const frame = page.frameLocator('[data-testid="html-block-preview"]')
    await expect(frame.locator('#hh')).toHaveText('SCRIPT_EXECUTED', {
      timeout: 5000,
    })

    // Give any escape attempt time to fire, then assert isolation held.
    await page.waitForTimeout(500)
    const pwned = await page.evaluate(() => (window as any).__HTML_PWNED)
    expect(pwned).toBe(false)
    // The top URL was NOT navigated by the sandbox's `top.location = ...`.
    expect(page.url()).not.toContain('evil.test')
  })

  // TEST-5: while the fence is genuinely mid-stream (isStreaming true + unclosed
  // fence ⇒ isIncomplete), the block stays CODE, Preview is disabled, no iframe.
  test('incomplete (streaming) html fence stays in CODE view, Preview disabled, no iframe', async ({
    page,
    testInfra,
  }) => {
    // Deliver started + partial deltas of an UNCLOSED ```html fence but NO
    // `complete` — so the stream stays open (isStreaming=true) and the fence is
    // genuinely incomplete. (A finalized unclosed fence would be auto-completed
    // and NOT incomplete — the disabled state only exists live.)
    const chunks = ['```html\n', '<div>', 'partial']
    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_inc_1' }),
        ...chunks.map(c => textDeltaEvent({ delta: c, messageId: 'amsg_inc_1' })),
        // intentionally no completeEvent()
      ],
    ])
    const pageErrors: string[] = []
    page.on('pageerror', e => pageErrors.push(e.message))

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    const textarea = byTestId(page, 'chat-message-textarea').first()
    await textarea.fill('stream html')
    await byTestId(page, 'chat-input-send-btn').click()

    // The live streaming bubble renders the partial (unclosed) html fence.
    const block = page.locator('[data-testid="html-block"]')
    await expect(block).toBeVisible({ timeout: 15000 })
    // Preview option is disabled while incomplete (Base UI reflects it in
    // aria-disabled); no iframe rendered.
    const previewOpt = block.locator('[data-testid="html-block-toggle-opt-preview"]')
    await expect(previewOpt).toHaveAttribute('aria-disabled', 'true')
    expect(await block.locator('iframe').count()).toBe(0)
    expect(pageErrors).toEqual([])
  })

  // TEST-6: copy-source copies the exact HTML; language label shown.
  test('copy button copies the HTML source; language label is shown', async ({
    page,
    context,
    testInfra,
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write'])
    const html = '<section>copy this exact html</section>'
    await seedAssistantWithText(page, testInfra.baseURL, fence(html))
    const bubble = assistantBubble(page)
    const block = bubble.locator('[data-testid="html-block"]')
    await expect(block).toContainText('html') // language label
    await block.locator('[data-testid="html-block-copy-btn"]').click()
    const clip = await page.evaluate(() => navigator.clipboard.readText())
    expect(clip).toContain('copy this exact html')
  })

  // TEST-8: Preview → Code is bidirectional (iframe unmounts, source returns).
  test('toggling Preview then Code returns to the source view', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      fence('<p>round trip</p>'),
    )
    const bubble = assistantBubble(page)
    const block = bubble.locator('[data-testid="html-block"]')
    await block.locator('[data-testid="html-block-toggle-opt-preview"]').click()
    await expect(block.locator('[data-testid="html-block-preview"]')).toBeVisible({
      timeout: 5000,
    })
    await block.locator('[data-testid="html-block-toggle-opt-code"]').click()
    await expect(block.locator('[data-testid="html-block-source"]')).toContainText(
      'round trip',
    )
    expect(await block.locator('iframe').count()).toBe(0)
  })
})
