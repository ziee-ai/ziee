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
 * Math IS supported: `streamdownPlugins.ts` wires `@streamdown/math`
 * (remark-math + rehype-katex) and `index.css` imports the KaTeX
 * stylesheet. The former `[[no-katex-remark-rehype]]` directive is
 * retired — it outlived the code it described. Because remark-math only
 * understands `$` delimiters, `preprocessMarkdown` normalizes LaTeX's
 * own delimiters first: display `\[ … \]` into `$$ … $$` (issue #177)
 * and inline `\( … \)` into `$ … $`. The inline pass is guarded against
 * regex syntax (`\(a\|b\)`), code (fences + inline spans), and an
 * unpaired `$` in the same paragraph — the specs below pin all three.
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

/**
 * Seed a conversation with TWO assistant messages (each carrying its own
 * markdown), so footnote per-message id-scoping can be exercised. The send flow
 * streams the first assistant turn; `mockGetMessages` then returns BOTH
 * assistant messages, so after the post-stream reload both bubbles mount, each
 * rendered by its own `<Streamdown>` with a distinct `content.id` (→ distinct
 * scoped footnote ids).
 */
async function seedTwoAssistantMessages(
  page: Page,
  baseURL: string,
  first: string,
  second: string,
) {
  await mockChatStream(page, [
    [
      startedEvent({ userMessageId: 'umsg_md_1' }),
      textDeltaEvent({ delta: first, messageId: 'amsg_md_1' }),
      completeEvent(),
    ],
  ])
  await mockGetMessages(page, [
    mockUserMessage({ id: 'umsg_md_1', text: 'render markdown please' }),
    assistantTextMessage('amsg_md_1', first),
    assistantTextMessage('amsg_md_2', second),
  ])

  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, 'GPT-4o Mini')
  const textarea = byTestId(page, 'chat-message-textarea').first()
  await textarea.fill('render markdown please')
  await byTestId(page, 'chat-input-send-btn').click()

  // Both assistant bubbles must mount after the post-stream reload.
  await expect(
    page.locator('[data-testid="chat-message"][data-role="assistant"]'),
  ).toHaveCount(2, { timeout: 15000 })
}

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
    // (mermaid is owned by the custom `renderers` entry in
    // `streamdownPlugins.ts` instead — see its docblock). The mermaid
    // fence therefore renders as
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

  // TEST-9 — the inverse of the former `does NOT render math` test. Math IS
  // wired (`streamdownPlugins.ts` passes `createMathPlugin`, `index.css` imports
  // the KaTeX stylesheet), so this pins that it stays wired.
  test('renders $$…$$ math with KaTeX styling', async ({ page, testInfra }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'Math here: $$x^2 + y^2 = z^2$$',
    )
    const bubble = assistantBubble(page)
    await expect(bubble).toContainText('Math here')
    await expect(bubble.locator('.katex').first()).toBeVisible({ timeout: 10000 })
    expect(
      await bubble.evaluate(el => el.querySelectorAll('.katex').length),
    ).toBeGreaterThan(0)
  })

  // TEST-7 — the literal reproduction of issue #177: the model wrote its display
  // equations with LaTeX's own `\[ … \]`, markdown ate the `\[` as a character
  // escape, and the equation surfaced as raw LaTeX. These are the exact strings
  // from the issue screenshot.
  test('renders \\[ … \\] display math (issue #177)', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'Steady state:\n\n' +
        '\\[ \\frac{d^2C(x)}{dx^2} - \\frac{k}{D}C(x) = 0 \\]\n\n' +
        'with solution\n\n' +
        '\\[ C(x) = C_0 \\, e^{-x/\\lambda}, \\quad \\lambda = \\sqrt{D/k} \\]',
    )
    const bubble = assistantBubble(page)
    await expect(bubble).toContainText('Steady state')

    // Both display equations render as KaTeX display blocks. The count IS the
    // proof the delimiters converted: if the raw `\[ … \]` had leaked through as
    // plain text (the #177 bug) it would be 0.
    //
    // We deliberately do NOT assert the raw TeX is ABSENT from the DOM. KaTeX
    // embeds the source in a hidden `<annotation encoding="application/x-tex">`
    // for screen readers (the a11y win of rendering as math), so `\frac{…}` is
    // legitimately present in the bubble's text content — a text-absence check
    // would fail on a correctly-rendered equation.
    await expect(bubble.locator('.katex-display').first()).toBeVisible({
      timeout: 10000,
    })
    expect(
      await bubble.evaluate(el => el.querySelectorAll('.katex-display').length),
    ).toBe(2)
    // ...and that hidden TeX annotation is the accessible-name source, so assert
    // it exists rather than that it's gone.
    expect(
      await bubble.evaluate(
        el => el.querySelectorAll('.katex-mathml annotation').length,
      ),
    ).toBe(2)
  })

  // TEST-15 — the inline counterpart of #177. `\( … \)` must reach the DOM as
  // INLINE KaTeX, not a display block: `.katex` present, `.katex-display` absent.
  //
  // This message contains no `[` at all, which is deliberate — it is the only
  // thing that exercises `preprocessMarkdown`'s early return. That guard used to
  // bail on any bracket-free document, which made the whole inline-math path a
  // silent no-op for exactly this, the most common real input.
  test('renders \\( … \\) as inline math, not a display block', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'The decay length is \\( \\lambda = \\sqrt{D/k} \\) at steady state.',
    )
    const bubble = assistantBubble(page)
    await expect(bubble).toContainText('The decay length is')
    await expect(bubble.locator('.katex').first()).toBeVisible({ timeout: 10000 })
    expect(
      await bubble.evaluate(el => el.querySelectorAll('.katex').length),
    ).toBeGreaterThan(0)
    // INLINE, not display — this is what separates it from the `\[ … \]` path.
    expect(
      await bubble.evaluate(el => el.querySelectorAll('.katex-display').length),
    ).toBe(0)
  })

  // TEST-16 — the prose tradeoff, pinned at the DOM level.
  //
  // Inline `\( … \)` in un-fenced prose NOW converts, so `sed -e 's/\(foo\)/bar/'`
  // renders with an italic math `foo` and no parens. That is a deliberate change,
  // not a regression: markdown's OWN character-escape rule already turned `\(`
  // into `(` before this feature existed, so the backslashes were lost either way
  // — the user previously saw `s/(foo)/bar/`, never the literal source. A real sed
  // command belongs in a code block, which stays literal (see the fence spec).
  //
  // Explicit regex syntax is the exception: `\(a\|b\)` carries a BRE signal, so it
  // is skipped and renders EXACTLY as it always has.
  test('converts inline \\( … \\) in prose but skips regex alternation', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      "Ran: sed -e 's/\\(foo\\)/bar/' on 3 files.\n\n" +
        'To escape use \\( and \\) in LaTeX.\n\n' +
        'Pattern \\(a\\|b\\) matched 4 lines.',
    )
    const bubble = assistantBubble(page)
    // The two prose cases became equations — exactly two of them.
    await expect(bubble.locator('.katex').first()).toBeVisible({ timeout: 10000 })
    expect(
      await bubble.evaluate(el => el.querySelectorAll('.katex').length),
    ).toBe(2)
    // ...and the sed line's parentheses are GONE, which is the discriminator: a
    // `.katex` count alone would also pass if some unrelated span had converted.
    // `s/(foo)/bar/` is what this rendered as BEFORE inline conversion existed.
    await expect(bubble).not.toContainText("s/(foo)/bar/")
    // ...but the regex line is untouched, parens and pipe intact, exactly as
    // markdown has always rendered it.
    await expect(bubble).toContainText('Pattern (a|b) matched 4 lines.')
  })

  // TEST-18 — the unpaired-`$` hijack guard, proved in the real renderer.
  //
  // Without it, `That costs $5 and the rate \( k \) is fixed.` would become
  // `…$5 and the rate $k$ is fixed.`, and remark-math would pair the price's `$`
  // with our injected opener — rendering `5 and the rate ` as an equation and
  // leaving a dangling literal `k$`. The guard skips the match instead, so the
  // sentence renders as plain text with the price intact.
  test('an unpaired $ in the paragraph suppresses inline conversion', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'That costs $5 and the rate \\( k \\) is fixed.',
    )
    const bubble = assistantBubble(page)
    await expect(bubble).toContainText('That costs $5 and the rate (k) is fixed.')
    // Nothing became an equation — in particular there is no mangled `5 and …`
    // span and no dangling `k$`.
    expect(await bubble.evaluate(el => el.querySelectorAll('.katex').length)).toBe(0)
  })

  // TEST-8 / TEST-17 — the main correctness risk: a delimiter inside code must
  // stay literal. This matters far more now that INLINE `\( … \)` converts too,
  // because a code fence is where a genuine sed/grep command actually lives — and
  // `preprocessMarkdown`'s fence + inline-code split is the only thing protecting
  // it (the guards inside `normalizeMathDelimiters` cannot see code structure).
  test('leaves \\[ … \\] and \\( … \\) inside a code block literal', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'Escaping in LaTeX source:\n\n' +
        "```tex\n\\[ x^2 \\]\nsed -e 's/\\(foo\\)/bar/'\n```\n\n" +
        'and inline `\\[ y \\]` plus `\\( y \\)` too.',
    )
    const bubble = assistantBubble(page)
    await expect(bubble).toContainText('Escaping in LaTeX source')
    const codeBlock = bubble.locator('[data-streamdown="code-block"]')
    await expect(codeBlock).toBeVisible({ timeout: 15000 })

    // The delimiters survive verbatim inside the fence — both forms...
    await expect(codeBlock).toContainText('\\[ x^2 \\]')
    await expect(codeBlock).toContainText("sed -e 's/\\(foo\\)/bar/'")
    // ...and inside the inline-code spans.
    await expect(bubble.locator('code', { hasText: '\\[ y \\]' }).first()).toBeVisible()
    await expect(bubble.locator('code', { hasText: '\\( y \\)' }).first()).toBeVisible()
    // ...and nothing in the bubble was turned into math.
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

  test('clicking a footnote reference expands References + cited excerpt and resolves the target', async ({
    page,
    testInfra,
  }) => {
    // Regression guard for the footnote-reference-click bug: Streamdown v2
    // double-prefixes footnote element ids (`user-content-user-content-fn-1`)
    // while the ref href stays single-prefixed, so the un-scoped definition id
    // used to break `getElementById` and the click no-oped. The prefix-agnostic
    // scoping (footnoteScope.ts) makes the ref href and the definition `<li>` id
    // resolve to the same message-scoped element. The 4-space indent keeps the
    // `>` blockquote INSIDE footnote 1's `<li>` (a multi-block footnote def).
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'See here[^1] for context.\n\n[^1]: A reference body.\n\n    > An excerpt from the cited source.',
    )
    const bubble = assistantBubble(page)
    const details = bubble.locator('details.footnote-section')
    await expect(details).toBeVisible({ timeout: 5000 })
    // Collapsed before the click.
    expect(await details.evaluate(el => (el as HTMLDetailsElement).open)).toBe(false)

    // The superscript reference link (backrefs are suppressed by the override).
    const ref = bubble.locator('sup a').first()
    await expect(ref).toBeVisible()
    const targetId = await ref.evaluate(
      el => (el as HTMLAnchorElement).getAttribute('href')?.slice(1) ?? '',
    )
    expect(targetId.length).toBeGreaterThan(0)

    await ref.click()

    // The References section is now expanded (the handler opened the enclosing
    // <details> — only possible if getElementById resolved the target).
    await expect(details).toHaveJSProperty('open', true)
    // The ref's href target actually exists in the DOM and is the footnote
    // definition <li> inside this bubble (the core fix — was null before).
    const resolved = await bubble.evaluate((el, id) => {
      const t = document.getElementById(id)
      return { found: !!t, tag: t?.tagName ?? null, inBubble: !!t && el.contains(t) }
    }, targetId)
    expect(resolved.found).toBe(true)
    expect(resolved.inBubble).toBe(true)
    expect(resolved.tag).toBe('LI')
    // The cited-excerpt <details> inside the footnote definition is expanded too.
    const quote = bubble.locator('details.footnote-quote')
    await expect(quote.first()).toHaveJSProperty('open', true)
    // No stray visible "Footnotes" heading leaked outside the <summary>
    // (isFootnoteLabel suppresses the double-prefixed sr-only label).
    expect(
      await bubble.evaluate(
        el =>
          Array.from(el.querySelectorAll('h2')).filter(
            h => /footnotes/i.test(h.textContent ?? ''),
          ).length,
      ),
    ).toBe(0)
  })

  test('footnote reference click is scoped per message', async ({
    page,
    testInfra,
  }) => {
    // Two assistant messages each contain `[^1]`. Clicking message 2's
    // reference must open message 2's References only — message 1's stays
    // collapsed. Guards the per-message `contentId` scoping (duplicate footnote
    // numbers across messages must not collide on a shared DOM id).
    await seedTwoAssistantMessages(
      page,
      testInfra.baseURL,
      'First message[^1].\n\n[^1]: First body.',
      'Second message[^1].\n\n[^1]: Second body.',
    )
    const bubbles = page.locator(
      '[data-testid="chat-message"][data-role="assistant"]',
    )
    const first = bubbles.nth(0)
    const second = bubbles.nth(1)
    const firstDetails = first.locator('details.footnote-section')
    const secondDetails = second.locator('details.footnote-section')
    await expect(firstDetails).toBeVisible({ timeout: 5000 })
    await expect(secondDetails).toBeVisible({ timeout: 5000 })

    // Click the reference in the SECOND message.
    await second.locator('sup a').first().click()

    await expect(secondDetails).toHaveJSProperty('open', true)
    // The first message's References must NOT have opened.
    expect(await firstDetails.evaluate(el => (el as HTMLDetailsElement).open)).toBe(false)
  })

  // --- paper-grouped references (ziee#167) ----------------------------------
  //
  // The BioGnosia RAG service cites one footnote per retrieved CHUNK and labels
  // same-paper chunks `P.1`, `P.2`. GFM renumbers footnotes sequentially and
  // keeps the label only in the anchor id/href, so what the READER sees is
  // decided entirely here — hence these assert rendered text, not markup.

  // Two chunks of one paper, then a second paper cited through a single chunk.
  // That second paper keeps a FLAT label — "2", not "2-1" — because the
  // hierarchy should only appear where it carries information. It is also the
  // sharpest case: GFM numbers by first reference, so this ref sits at ordinal
  // position 3 and would DISPLAY as "3" if the renderer trusted GFM.
  const GROUPED_ANSWER = [
    'Caspase-8 gates the switch[^1-1][^1-2] and PTEN modulates it[^2].',
    '',
    '[^1-1]: Short et al. (2024). Regulated cell death. *Cell Death Differ*. https://doi.org/10.1038/s41418-024-01346-x',
    '    > Caspase-8 acts as a molecular switch.',
    '',
    '[^1-2]: Short et al. (2024). Regulated cell death. *Cell Death Differ*. https://doi.org/10.1038/s41418-024-01346-x',
    '    > Loss of caspase-8 redirects to necroptosis.',
    '',
    '[^2]: Doe J, Roe A (2021). PTEN and HR repair. *Nature*.',
    '    > PTEN loss impairs homologous recombination.',
  ].join('\n')

  test('TEST-7: adjacent citations are comma-separated, lone refs and exponents are not', async ({
    page,
    testInfra,
  }) => {
    // The bug: `[^1][^2][^3]` rendered as one run-together blob, "123".
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'Three here[^1][^2][^3]. One here[^4]. An exponent: E = mc<sup>2</sup>.\n\n' +
        ['[^1]: One.', '[^2]: Two.', '[^3]: Three.', '[^4]: Four.'].join('\n\n'),
    )
    const bubble = assistantBubble(page)
    // `innerText` omits ::before content, so read the marks the CSS keys off.
    const marked = await bubble.evaluate(el =>
      [...el.querySelectorAll('sup')].map(s => ({
        ref: !!s.querySelector('a[data-footnote-ref]'),
        comma: s.classList.contains('footnote-ref-adjacent'),
        before: getComputedStyle(s, '::before').content,
      })),
    )
    const refs = marked.filter(m => m.ref)
    // The run of three: only the 2nd and 3rd get a comma.
    expect(refs.slice(0, 3).map(m => m.comma)).toEqual([false, true, true])
    expect(refs[1].before).toBe('", "')
    // The LATER, standalone citation must NOT gain a comma — it is a `sup + sup`
    // CSS match (text between is ignored by the combinator) but not a real run.
    expect(refs[3].comma).toBe(false)
    // A real exponent is never marked.
    expect(marked.filter(m => !m.ref).every(m => !m.comma)).toBe(true)
  })

  test('TEST-8: DEGRADATION — a plain sequential footnote set renders exactly as before', async ({
    page,
    testInfra,
  }) => {
    // The renderer is shared with the citations + knowledge_base modules. A
    // set with no hierarchical label must not be grouped, relabelled, or
    // renumbered — one flat <li> each, showing GFM's own ordinal.
    await seedAssistantWithText(
      page,
      testInfra.baseURL,
      'A[^1] and B[^2].\n\n[^1]: First body.\n\n[^2]: Second body.',
    )
    const bubble = assistantBubble(page)
    const details = bubble.locator('details.footnote-section')
    await expect(details).toBeVisible({ timeout: 5000 })

    // No grouping wrappers were introduced.
    await expect(bubble.locator('li.footnote-paper')).toHaveCount(0)
    await expect(bubble.locator('ol.footnote-excerpts')).toHaveCount(0)
    // Exactly two flat definition items, in the top-level list.
    await expect(details.locator('> ol > li')).toHaveCount(2)
    // Inline markers still display GFM's ordinals, not a P.C label.
    expect(await bubble.locator('sup a').first().innerText()).toBe('1')
    expect(await bubble.locator('sup a').nth(1).innerText()).toBe('2')
  })

  test('TEST-9: same-paper chunks merge into one entry with 1.1/1.2 excerpts', async ({
    page,
    testInfra,
  }) => {
    await seedAssistantWithText(page, testInfra.baseURL, GROUPED_ANSWER)
    const bubble = assistantBubble(page)

    // (c) inline markers show the LABEL, not GFM's sequential 1/2/3. The third
    // marker is the giveaway twice over: its ordinal position is 3, so seeing
    // "2" proves both that the label won AND that a FLAT label is left flat.
    const refs = bubble.locator('sup a')
    expect(await refs.nth(0).innerText()).toBe('1.1')
    expect(await refs.nth(1).innerText()).toBe('1.2')
    expect(await refs.nth(2).innerText()).toBe('2')

    const details = bubble.locator('details.footnote-section')
    await expect(details).toBeVisible({ timeout: 5000 })
    await details.evaluate(el => el.setAttribute('open', ''))

    // (b) one bibliographic entry per PAPER, not one per chunk — 2, not 3.
    // Scoped to the TOP-LEVEL list: a bare `ol > li` would also match the
    // nested excerpt items and count 4.
    const entries = details.locator('> ol > li')
    await expect(entries.first()).toBeVisible()
    await expect(entries).toHaveCount(2)
    // Only the MULTI-chunk paper becomes a nested entry; the single-chunk one
    // stays flat, exactly as it renders today.
    const paper = bubble.locator('li.footnote-paper')
    await expect(paper).toHaveCount(1)
    // The duplicated header appears ONCE, and the nesting is self-explanatory.
    expect((await paper.innerText()).match(/Regulated cell death/g)).toHaveLength(1)
    await expect(paper).toContainText('2 cited excerpts')
    // The flat entry has no sub-list and no count caption.
    const flat = entries.nth(1)
    await expect(flat).not.toContainText('cited excerpts')
    expect(await flat.locator('ol.footnote-excerpts').count()).toBe(0)

    // Both excerpts survive, labelled beneath their paper.
    const excerpts = paper.first().locator('ol.footnote-excerpts > li')
    await expect(excerpts).toHaveCount(2)
    await expect(excerpts.nth(0)).toContainText('1.1')
    await expect(excerpts.nth(1)).toContainText('1.2')

    // Clicking inline 1.2 expands THAT excerpt (the anchor id survived grouping).
    await refs.nth(1).click()
    await expect(
      excerpts.nth(1).locator('details.footnote-quote'),
    ).toHaveJSProperty('open', true)
    await expect(excerpts.nth(1)).toContainText('redirects to necroptosis')
  })

  test('TEST-10: sanitization survives the footnote grouper, grouped and sequential', async ({
    page,
    testInfra,
  }) => {
    // The grouper is appended to `rehypePlugins`, which REPLACES Streamdown's
    // default chain — so the defaults (raw → sanitize → harden) are spread back
    // in first. If that spread is ever dropped, this test is what catches it.
    const payload = '<script>window.XSS_PWNED = true</script>' +
      '<img src=x onerror="window.XSS_PWNED = true">'

    for (const [shape, markdown] of [
      // Grouped shape: plugin ACTIVE. Payload in the body, in a bib header, and
      // inside a grouped excerpt.
      [
        'grouped',
        [
          `Body ${payload} text[^1-1][^1-2].`,
          '',
          `[^1-1]: Short et al. (2024). ${payload} Journal.`,
          `    > Excerpt one ${payload}`,
          '',
          `[^1-2]: Short et al. (2024). ${payload} Journal.`,
          `    > Excerpt two ${payload}`,
        ].join('\n'),
      ],
      // Sequential shape: plugin BAILS. Same chain must still hold.
      ['sequential', `Body ${payload}[^1].\n\n[^1]: A body ${payload}\n\n    > ${payload}`],
    ] as const) {
      await page.addInitScript(() => {
        ;(window as any).XSS_PWNED = false
      })
      await seedAssistantWithText(page, testInfra.baseURL, markdown)
      const bubble = assistantBubble(page)
      await bubble
        .locator('details.footnote-section')
        .evaluate(el => el.setAttribute('open', ''))

      expect(
        await page.evaluate(() => (window as any).XSS_PWNED),
        `${shape}: no payload executed`,
      ).toBe(false)
      // No live script element and no event-handler attribute anywhere in the
      // rendered message — including inside the regrouped reference list.
      expect(
        await bubble.evaluate(el => el.querySelectorAll('script').length),
        `${shape}: no <script> element`,
      ).toBe(0)
      expect(
        await bubble.evaluate(el => el.querySelectorAll('[onerror]').length),
        `${shape}: no onerror attribute`,
      ).toBe(0)
    }
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
