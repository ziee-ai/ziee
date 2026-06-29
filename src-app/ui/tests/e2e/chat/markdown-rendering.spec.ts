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
 * Streamdown lock-in spec.
 *
 * TextContent.tsx already uses `<Streamdown shikiTheme isAnimating
 * components>`, but zero E2Es assert that any of its built-in features
 * (mermaid, GFM tables, Shiki-themed code, footnotes) actually render.
 * This spec adds that coverage so a future refactor can't silently
 * regress what users see today.
 *
 * Strategy: mock the SSE stream + the post-stream /messages reload so
 * each test seeds an assistant message with deterministic markdown
 * content. No real LLM cost. The chat-extension stream parser routes
 * `text_delta` events into the existing text content block, which
 * TextContent.tsx renders via Streamdown — same code path as production.
 *
 * Per project directive ([[no-katex-remark-rehype]]) math is NOT
 * supported; one negative test pins that decision so a stray katex
 * import doesn't slip in.
 */

const assistantTextMessage = (id: string, text: string): MockMessageWithContent => ({
  id,
  role: 'assistant',
  contents: [
    {
      content_type: 'text',
      content: { type: 'text', text },
    },
  ],
})

async function seedAssistantWithText(
  page: Page,
  baseURL: string,
  markdown: string,
) {
  // Two-message conversation: the user's "anything" prompt + the
  // canned assistant response containing the markdown under test.
  await mockChatStream(page, [
    [
      startedEvent({ userMessageId: 'umsg_md_1' }),
      textDeltaEvent({ delta: markdown, messageId: 'amsg_md_1' }),
      completeEvent(),
    ],
  ])
  await mockGetMessages(page, [
    mockUserMessage({ id: 'umsg_md_1', text: 'render markdown please' }),
    assistantTextMessage('amsg_md_1', markdown),
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')

  const textarea = byTestId(page, 'chat-message-textarea').first()
  await textarea.fill('render markdown please')
  await byTestId(page, 'chat-input-send-btn').click()

  // Wait for the canned assistant bubble to mount. The complete event
  // triggers loadMessages → renders the persisted bubble.
  await expect(
    page.locator('[data-testid="chat-message"][data-role="assistant"]').last(),
  ).toBeVisible({ timeout: 15000 })
}

const assistantBubble = (page: Page) =>
  page.locator('[data-testid="chat-message"][data-role="assistant"]').last()

test.describe('Tier 1 — streamdown lock-in (chat assistant markdown rendering)', () => {
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

  test(
    'mermaid block renders as a styled code-block (no SVG: plugin not installed)',
    async ({ page, testInfra }) => {
      // FIXME: vite cold-start + streamdown 2's dynamic-import of
      // `dist/highlighted-body-*.js` (Shiki) interact badly in this
      // test infra. When the SPEC starts with a test that triggers
      // any code-block render, vite hits a 504 "Outdated Optimize
      // Dep" on the lazy chunk, the React error boundary fires, and
      // the assistant bubble never mounts in time. Tried
      // optimizeDeps.include / .exclude / .entries — none reliably
      // pre-bundle the hashed internal chunk.
      // The underlying behavior IS verified manually + via the
      // shiki test below (which catches code-block rendering more
      // directly). Re-enable when a fix lands for the test infra
      // (see [[streamdown-v2-unbundled-plugins]]).
    // Streamdown 2 unbundled mermaid into the `@streamdown/mermaid`
    // plugin package, which this project intentionally does NOT install
    // (per [[no-katex-remark-rehype]] — keep dep surface small, no
    // markdown plugin packages). The mermaid fence therefore renders as
    // a normal code block (`data-language="mermaid"`) with an EMPTY
    // body — no SVG. Pin this behavior so a future "let's add mermaid
    // back" PR has to update this test (and the plan) deliberately.
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      '```mermaid\ngraph LR\n  A-->B\n```',
    )
    const bubble = assistantBubble(page)
    const codeBlock = bubble.locator(
      '[data-streamdown="code-block"][data-language="mermaid"]',
    )
    await expect(codeBlock).toBeVisible({ timeout: 10000 })
    // The mermaid plugin would inject an <svg> into code-block-body.
    // Without the plugin, the body is empty. Scope strictly to the
    // body — the surrounding code-block wrapper has header chrome
    // icons (copy, expand, etc.) that ARE svgs.
    const body = codeBlock.locator(
      '[data-streamdown="code-block-body"]',
    )
    expect(await body.locator('svg').count()).toBe(0)
  },
  )

  test('renders GFM table as <table>', async ({ page, testInfra }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      '| h1 | h2 |\n|----|----|\n| a  | b  |\n| c  | d  |',
    )
    const bubble = assistantBubble(page)
    await expect(bubble.locator('table thead tr th').first()).toHaveText('h1')
    expect(await bubble.locator('table tbody tr').count()).toBe(2)
  })

  test(
    'renders fenced code with Shiki highlighting',
    async ({ page, testInfra }) => {
    // A fenced ```rust block is routed through Streamdown's shikiTheme
    // (wired in TextContent.tsx). Assert the SAME proven structure the
    // mermaid test relies on — the streamdown code-block wrapper tagged
    // with the fence language — then assert GENUINE Shiki highlighting:
    // the highlighted body carries token <span>s with INLINE `color:`
    // styles (Shiki's hallmark). Plain, unhighlighted text would have
    // zero inline-colored spans, so this catches a silent regression of
    // highlighting back to a bare <pre>.
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      '```rust\nfn foo() -> u32 { 42 }\n```',
    )
    const bubble = assistantBubble(page)
    const codeBlock = bubble.locator(
      '[data-streamdown="code-block"][data-language="rust"]',
    )
    await expect(codeBlock).toBeVisible({ timeout: 15000 })
    const body = codeBlock.locator('[data-streamdown="code-block-body"]')
    // The code text survived into the rendered block.
    await expect(body).toContainText('fn foo')
    // Shiki applies per-token colors via inline `style="color:..."` on
    // <span>s inside the <pre>. At least one such colored token must
    // exist — its absence means highlighting silently regressed to
    // plain text.
    const coloredTokens = body.locator('pre span[style*="color"]')
    await expect(coloredTokens.first()).toBeVisible({ timeout: 10000 })
    expect(await coloredTokens.count()).toBeGreaterThan(0)
  },
  )

  test('does NOT render math with KaTeX styling', async ({ page, testInfra }) => {
    // Per [[no-katex-remark-rehype]] — math is intentionally not
    // wired. This test pins that decision so a future stray katex
    // import doesn't slip in unnoticed.
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'Math here: $$x^2 + y^2 = z^2$$',
    )
    const bubble = assistantBubble(page)
    // Wait for the message text to render before asserting the absence.
    await expect(bubble).toContainText('Math here')
    // No .katex class anywhere — would be present if rehype-katex were active.
    expect(await bubble.evaluate(el => el.querySelectorAll('.katex').length)).toBe(0)
  })

  test('renders footnotes with collapsed References section', async ({
    page,
    testInfra,
  }) => {
    // `useStreamdownComponents` transforms the auto-generated GFM footnotes
    // section into a `<details><summary>References</summary>...` block.
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'See here[^1] for context.\n\n[^1]: A reference body.',
    )
    const bubble = assistantBubble(page)
    const details = bubble.locator('details.footnote-section')
    await expect(details).toBeVisible({ timeout: 5000 })
    await expect(details.locator('summary')).toHaveText(/References/i)
    // Collapsed by default (no `open` attribute on <details>).
    expect(await details.evaluate(el => (el as HTMLDetailsElement).open)).toBe(false)
  })

  test('raw <script> tags in markdown do not execute', async ({
    page,
    testInfra,
  }) => {
    // Streamdown's defaults do NOT include rehype-raw, so HTML in markdown
    // should render as escaped text — not as live DOM. Pin this so a future
    // contributor doesn't accidentally enable rehype-raw.
    await page.addInitScript(() => {
      ;(window as any).XSS_PWNED = false
    })
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'Before\n\n<script>window.XSS_PWNED = true</script>\n\nAfter',
    )
    const bubble = assistantBubble(page)
    await expect(bubble).toContainText('Before')
    await expect(bubble).toContainText('After')
    const pwned = await page.evaluate(() => (window as any).XSS_PWNED)
    expect(pwned).toBe(false)
  })

  test('incremental streaming: half-rendered table does not throw', async ({
    page,
    testInfra,
  }) => {
    // The point of streamdown over plain react-markdown is graceful
    // handling of half-finished syntax during the stream. Feed the table
    // a piece at a time and assert (a) no page error, (b) the final
    // render shows the complete table.
    const finalText = '| a | b |\n|----|----|\n| 1 | 2 |\n| 3 | 4 |'
    const chunks = [
      '|',
      ' a',
      ' |',
      ' b |',
      '\n|--',
      '--|--',
      '--|',
      '\n| 1 |',
      ' 2 |',
      '\n| 3 |',
      ' 4 |',
    ]
    await mockChatStream(page, [
      [
        startedEvent({ userMessageId: 'umsg_stream_1' }),
        ...chunks.map(c => textDeltaEvent({ delta: c, messageId: 'amsg_stream_1' })),
        completeEvent(),
      ],
    ])
    await mockGetMessages(page, [
      mockUserMessage({ id: 'umsg_stream_1', text: 'stream a table' }),
      assistantTextMessage('amsg_stream_1', finalText),
    ])

    // Capture any page errors during the stream — streamdown should
    // tolerate every intermediate state.
    const pageErrors: string[] = []
    page.on('pageerror', e => pageErrors.push(e.message))

    await goToNewChatPage(page, testInfra.baseURL)
    await selectModelInDropdown(page, 'GPT-4o Mini')
    const textarea = byTestId(page, 'chat-message-textarea').first()
    await textarea.fill('stream a table')
    await byTestId(page, 'chat-input-send-btn').click()

    const bubble = assistantBubble(page)
    await expect(bubble).toBeVisible({ timeout: 15000 })
    // Final shape: a complete table.
    await expect(bubble.locator('table tbody tr')).toHaveCount(2, { timeout: 5000 })
    expect(pageErrors).toEqual([])
  })
})
