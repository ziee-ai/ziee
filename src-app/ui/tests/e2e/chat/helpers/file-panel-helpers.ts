import { Page, expect } from '@playwright/test'
import path from 'path'
import { fileURLToPath } from 'url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

/**
 * Right-panel + file-viewer E2E helpers.
 *
 * The chat-right-panel.spec.ts suite tests the docking right panel, the
 * pluggable file-viewer system, and the empty-state safety nets. These
 * helpers cover the common flows: attach a file via the + dropdown, send
 * a message with attachments, then click the resulting file card to open
 * the right panel.
 */

// ─── Test asset paths ────────────────────────────────────────────────────────
//
// We reuse the backend integration test data — it already has every file
// type we need to exercise. Resolved relative to this helper file so the
// path works regardless of test runner CWD.

const TEST_DATA_DIR = path.resolve(
  __dirname,
  '../../../../../server/tests/file/test_data',
)

export const FILE_ASSETS = {
  md: path.join(TEST_DATA_DIR, 'test.md'),
  csv: path.join(TEST_DATA_DIR, 'test.csv'),
  txt: path.join(TEST_DATA_DIR, 'test.txt'),
  png: path.join(TEST_DATA_DIR, 'test.png'),
  pdf: path.join(TEST_DATA_DIR, 'test.pdf'),
  xlsx: path.join(TEST_DATA_DIR, 'test.xlsx'),
  html: path.join(TEST_DATA_DIR, 'test.html'),
  // PPTX is accepted by the backend (a zip-family OOXML container) but
  // intentionally NOT processed (OfficeProcessor.can_process excludes
  // PPT / PPTX — pandoc can't read PowerPoint, and the only pure-Rust
  // PPTX renderer is broken against quick-xml 0.38) and has NO
  // frontend viewer registered (pdf/module.tsx deliberately skips
  // PPTX). That makes it the canonical "uploads fine, nothing can
  // render it" type — used for the "Cannot preview this file"
  // empty-state test. (.docx no longer works here: it now routes to
  // the PDF viewer.)
  unknown: path.join(TEST_DATA_DIR, '3_slides.pptx'),
} as const

// ─── File attachment via the + dropdown ──────────────────────────────────────

/**
 * Attach a file through the user-facing flow: open the + dropdown, click
 * "Attach files or photos", pass the file path to the resulting file
 * chooser. Waits until the file appears in the input-area preview to
 * confirm the upload completed.
 */
export async function attachFileViaUI(page: Page, absoluteFilePath: string): Promise<void> {
  const filename = path.basename(absoluteFilePath)

  // The + dropdown (aria-label "Add tools & files", stable testid
  // `chat-input-add-btn`) contains the "Attach files or photos" menu item.
  // Clicking that item triggers the native file chooser; we capture it via
  // Playwright's fileChooser event.
  await page.locator('[data-testid="chat-input-add-btn"]').click()
  const [fileChooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByText('Attach files or photos').click(),
  ])
  await fileChooser.setFiles(absoluteFilePath)

  await waitForFileInPreview(page, filename)
}

/**
 * Wait for a file card with the given filename to appear in the chat
 * input's preview area (FilePreviewList). This confirms upload completion
 * via reactive store update — no polling needed.
 */
export async function waitForFileInPreview(page: Page, filename: string): Promise<void> {
  const card = page.locator(`[data-testid="file-card"][data-filename="${filename}"]`)
  await expect(card.first()).toBeVisible({ timeout: 30000 })
}

// ─── Opening the right panel ─────────────────────────────────────────────────

/**
 * Click the file card with the given filename to open it in the right panel.
 * Locates the LAST matching file card on the page — by default the most
 * recent message attachment, since both the input preview AND each sent
 * message render their own FileCards.
 */
export async function openFileInPanel(page: Page, filename: string): Promise<void> {
  const card = page
    .locator(`[data-testid="file-card"][data-filename="${filename}"]`)
    .last()
  await card.click()
  // The panel is open when its outer wrapper has data-panel-open="true".
  await expect(page.locator('[data-testid="chat-right-panel"]')).toHaveAttribute(
    'data-panel-open',
    'true',
    { timeout: 10000 },
  )
}

// ─── Panel queries ───────────────────────────────────────────────────────────

export async function isPanelOpen(page: Page): Promise<boolean> {
  const attr = await page
    .locator('[data-testid="chat-right-panel"]')
    .getAttribute('data-panel-open')
  return attr === 'true'
}

// The panel now uses the kit <Tabs> (shadcn/base-ui), not AntD: each tab is a
// [data-slot="tabs-trigger"] (role=tab) with data-state="active|inactive", and
// the per-tab × is a following-sibling <button> of the trigger.
export async function getPanelTabCount(page: Page): Promise<number> {
  return await page
    .locator('[data-testid="chat-right-panel-tabs"] [data-slot="tabs-trigger"]')
    .count()
}

export async function getActivePanelTabTitle(page: Page): Promise<string> {
  return (
    (await page
      .locator('[data-testid="chat-right-panel-tabs"] [data-slot="tabs-trigger"][data-state="active"]')
      .innerText()) ?? ''
  )
}

// ─── Panel actions ───────────────────────────────────────────────────────────

export async function activatePanelTab(page: Page, title: string): Promise<void> {
  await page
    .locator('[data-testid="chat-right-panel-tabs"] [data-slot="tabs-trigger"]')
    .filter({ hasText: title })
    .first()
    .click()
  await expect(
    page
      .locator('[data-testid="chat-right-panel-tabs"] [data-slot="tabs-trigger"][data-state="active"]')
      .filter({ hasText: title }),
  ).toBeVisible({ timeout: 5000 })
}

/**
 * Close a tab via its built-in × button (the kit Tabs renders it as a
 * following-sibling <button> of the tab trigger).
 */
export async function closePanelTab(page: Page, title: string): Promise<void> {
  await page
    .locator('[data-testid="chat-right-panel-tabs"] [data-slot="tabs-trigger"]')
    .filter({ hasText: title })
    .first()
    .locator('xpath=following-sibling::button')
    .first()
    .click()
}

/**
 * Click the panel-level Close button (the one in the tab strip's right
 * extra area, not the per-tab × buttons). Clears all tabs and collapses
 * the panel.
 */
export async function closeEntirePanel(page: Page): Promise<void> {
  await page.locator('[data-testid="chat-right-panel-close"]').click()
}

// ─── Scoped query helpers ────────────────────────────────────────────────────
//
// Always scope panel-chrome button lookups to inside the panel. Globally,
// "Copy" matches message-actions copy buttons and "Download" matches the
// chat-input Export extension's "download Export" button — both cause
// Playwright strict-mode violations otherwise.

/**
 * Return a Locator for a button (by accessible name) rendered inside the
 * right panel. Used to assert header actions like Copy / Download / the
 * raw-toggle Eye/Code buttons without clashing with the chat input or
 * message-action buttons elsewhere on the page.
 */
export function panelButton(page: Page, name: string) {
  return page.locator('[data-testid="chat-right-panel"]').getByRole('button', { name })
}
