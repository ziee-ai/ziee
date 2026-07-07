/**
 * Mermaid code⇄render toggle — behavioral e2e against the component gallery
 * (backend-free `/gallery.html`). Drives the real `MermaidBlock` renderer through
 * the `mermaid-block` story cases (render / source / error / streaming) so the
 * real `mermaid` package renders a real `<svg>` in the browser.
 *
 * Closes AFFORDANCE_MATRIX G1 (source⇄render toggle) + G2 (copy source) + the
 * download-SVG rider. Lifecycle: .lifecycle/mermaid-toggle (TEST-1..6).
 */
import { test, expect } from '@playwright/test'
import { openGallery } from './_gallery'

const SECTION = 'gallery-section-mermaid-block'
const caseId = (k: string) => `gallery-case-mermaid-block-${k}`
// Async mermaid render (lazy import + parse + layout) can take a beat on a cold
// Vite server.
const RENDER_TIMEOUT = 20_000

test.describe('Mermaid code⇄render toggle', () => {
  let pageErrors: string[]

  test.beforeEach(async ({ page }) => {
    pageErrors = []
    page.on('pageerror', e => pageErrors.push(String(e)))
    await openGallery(page, 'light', 'blue')
    await page.getByTestId(SECTION).scrollIntoViewIfNeeded()
  })

  test('TEST-1: renders the diagram by default (real svg)', async ({ page }) => {
    const card = page
      .getByTestId(caseId('render'))
      .locator('[data-streamdown="mermaid-block"]')
    await expect(card).toBeVisible()
    await expect(
      card.locator('[data-testid="mermaid-diagram"] svg'),
    ).toBeVisible({ timeout: RENDER_TIMEOUT })
  })

  test('TEST-2: toggle flips render↔source', async ({ page }) => {
    const scope = page.getByTestId(caseId('render'))
    await expect(
      scope.locator('[data-testid="mermaid-diagram"] svg'),
    ).toBeVisible({ timeout: RENDER_TIMEOUT })

    // Default = Diagram selected.
    await expect(
      scope.getByTestId('mermaid-source-toggle-opt-render'),
    ).toHaveAttribute('data-state', 'on')

    // Flip to Source → raw source shows, diagram gone.
    await scope.getByTestId('mermaid-source-toggle-opt-source').click()
    await expect(scope.getByTestId('mermaid-source-view')).toBeVisible()
    await expect(scope.getByTestId('mermaid-source-view')).toContainText('graph TD')
    await expect(scope.locator('[data-testid="mermaid-diagram"]')).toHaveCount(0)

    // Flip back to Diagram → svg returns.
    await scope.getByTestId('mermaid-source-toggle-opt-render').click()
    await expect(
      scope.locator('[data-testid="mermaid-diagram"] svg'),
    ).toBeVisible()
  })

  test('TEST-3: invalid diagram → inline error; streaming → placeholder', async ({ page }) => {
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
    const scope = page.getByTestId(caseId('render'))
    await scope.getByTestId('mermaid-copy-source-btn').click()
    const copied = await page.evaluate(() => navigator.clipboard.readText())
    expect(copied).toContain('graph TD')
    expect(copied).toContain('B{Decision}')
  })

  test('TEST-5: download-svg disabled while streaming, enabled + downloads after render', async ({
    page,
  }) => {
    await expect(
      page.getByTestId(caseId('streaming')).getByTestId('mermaid-download-svg-btn'),
    ).toBeDisabled()

    const scope = page.getByTestId(caseId('render'))
    await expect(
      scope.locator('[data-testid="mermaid-diagram"] svg'),
    ).toBeVisible({ timeout: RENDER_TIMEOUT })
    const dlBtn = scope.getByTestId('mermaid-download-svg-btn')
    await expect(dlBtn).toBeEnabled()

    const [download] = await Promise.all([
      page.waitForEvent('download'),
      dlBtn.click(),
    ])
    expect(download.suggestedFilename()).toMatch(/\.svg$/)
  })

  test('TEST-6: all four cases reach their expected terminal state', async ({ page }) => {
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
