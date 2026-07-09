/**
 * Mermaid code⇄render toggle — behavioral e2e against the backend-free gallery.
 *
 * Two surfaces:
 *  - the isolated `deep-chat-rendering-showcase` deep-state (a REAL
 *    ConversationPage that renders a mermaid fence through the production chat
 *    path: TextContent → Streamdown → our `plugins.renderers` MermaidBlock). This
 *    focused single-surface render is the stable click target, so the interactive
 *    tests (render / toggle / copy / download) run here and thus prove the real
 *    chat integration.
 *  - the `mermaid-block` component story (render / source / error / streaming
 *    cases), used for the visibility-only state assertions (both-modes +
 *    edge-state exercise); the browse-all-stories view mounts every story at once
 *    so it is only asserted against, never clicked.
 *
 * Closes AFFORDANCE_MATRIX G1 (source⇄render toggle) + G2 (copy source) + the
 * download-SVG rider. Lifecycle: .lifecycle/mermaid-toggle (TEST-1..6).
 */
import { readFile } from 'node:fs/promises'
import { test, expect, type Page } from '@playwright/test'
import { openGallery } from './_gallery'

const SHOWCASE = '/gallery.html?surface=deep-chat-rendering-showcase&theme=light'
const SECTION = 'gallery-section-mermaid-block'
const caseId = (k: string) => `gallery-case-mermaid-block-${k}`
// Async mermaid render (lazy import + parse + layout) can take a beat on a cold
// Vite server; the deep-state also has to load its conversation first.
const RENDER_TIMEOUT = 25_000

/**
 * Open the real-chat showcase surface and return its (single) rendered mermaid
 * block, once the diagram SVG is on screen.
 */
async function openShowcaseBlock(page: Page) {
  await page.goto(SHOWCASE)
  const block = page.locator('[data-streamdown="mermaid-block"]').first()
  await block
    .locator('[data-testid="mermaid-diagram"] svg')
    .waitFor({ state: 'visible', timeout: RENDER_TIMEOUT })
  return block
}

test.describe('Mermaid code⇄render toggle', () => {
  let pageErrors: string[]

  test.beforeEach(async ({ page }) => {
    pageErrors = []
    page.on('pageerror', e => pageErrors.push(String(e)))
  })

  test('TEST-1: renders the diagram by default via the real chat path', async ({ page }) => {
    const block = await openShowcaseBlock(page)
    await expect(block).toBeVisible()
    await expect(block.locator('[data-testid="mermaid-diagram"] svg')).toBeVisible()
  })

  test('TEST-2: toggle flips render↔source', async ({ page }) => {
    const block = await openShowcaseBlock(page)

    // Default = Diagram selected.
    await expect(block.getByTestId('mermaid-source-toggle-opt-render')).toHaveAttribute(
      'data-state',
      'on',
    )

    // Flip to Source → raw source shows, diagram gone.
    await block.getByTestId('mermaid-source-toggle-opt-source').click()
    await expect(block.getByTestId('mermaid-source-view')).toBeVisible()
    await expect(block.getByTestId('mermaid-source-view')).toContainText('graph TD')
    await expect(block.locator('[data-testid="mermaid-diagram"]')).toHaveCount(0)

    // Flip back to Diagram → svg returns.
    await block.getByTestId('mermaid-source-toggle-opt-render').click()
    await expect(block.locator('[data-testid="mermaid-diagram"] svg')).toBeVisible()
  })

  test('TEST-3: invalid diagram → inline error; streaming → placeholder', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    await page.getByTestId(SECTION).scrollIntoViewIfNeeded()

    const err = page.getByTestId(caseId('error'))
    await expect(err.getByTestId('mermaid-error')).toBeVisible({ timeout: RENDER_TIMEOUT })
    await expect(err.locator('[data-testid="mermaid-diagram"]')).toHaveCount(0)

    const streaming = page.getByTestId(caseId('streaming'))
    await expect(streaming.getByTestId('mermaid-rendering')).toBeVisible()
    await expect(streaming.locator('[data-testid="mermaid-diagram"]')).toHaveCount(0)

    // A bad diagram must NOT blow up the surface.
    expect(pageErrors, pageErrors.join('\n')).toHaveLength(0)
  })

  test('TEST-4: copy-source writes the exact source to the clipboard', async ({
    page,
    context,
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write'])
    const block = await openShowcaseBlock(page)
    await block.getByTestId('mermaid-copy-source-btn').click()
    const copied = await page.evaluate(() => navigator.clipboard.readText())
    expect(copied).toContain('graph TD')
    expect(copied).toContain('B{Decision}')
  })

  test('TEST-5: download-svg disabled while streaming, enabled + downloads real SVG', async ({
    page,
  }) => {
    // Disabled state — read on the story's streaming case (an attribute check,
    // no click, so the browse view's story density doesn't matter).
    await openGallery(page, 'light', 'blue')
    await expect(
      page.getByTestId(caseId('streaming')).getByTestId('mermaid-download-svg-btn'),
    ).toBeDisabled()

    // Enabled + real download — on the clickable showcase surface.
    const block = await openShowcaseBlock(page)
    const dlBtn = block.getByTestId('mermaid-download-svg-btn')
    await expect(dlBtn).toBeEnabled()
    const [download] = await Promise.all([page.waitForEvent('download'), dlBtn.click()])
    expect(download.suggestedFilename()).toMatch(/\.svg$/)
    // Prove the download carries the real rendered SVG, not an empty/broken blob.
    const path = await download.path()
    expect(await readFile(path, 'utf8')).toContain('<svg')
  })

  test('TEST-6: all four story cases reach their expected terminal state', async ({ page }) => {
    await openGallery(page, 'light', 'blue')
    await page.getByTestId(SECTION).scrollIntoViewIfNeeded()
    await expect(
      page.getByTestId(caseId('render')).locator('[data-testid="mermaid-diagram"] svg'),
    ).toBeVisible({ timeout: RENDER_TIMEOUT })
    await expect(
      page.getByTestId(caseId('source')).getByTestId('mermaid-source-view'),
    ).toBeVisible()
    await expect(
      page.getByTestId(caseId('error')).getByTestId('mermaid-error'),
    ).toBeVisible({ timeout: RENDER_TIMEOUT })
    await expect(
      page.getByTestId(caseId('streaming')).getByTestId('mermaid-rendering'),
    ).toBeVisible()
    expect(pageErrors, pageErrors.join('\n')).toHaveLength(0)
  })
})
