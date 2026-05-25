import { Page, expect } from '@playwright/test'
import { fillDownloadForm, fillUploadForm, submitUploadForm, type DownloadFormData, type UploadFormData } from './form-helpers'

/**
 * LLM Model CRUD helpers
 *
 * IMPORTANT: These helpers should NOT handle drawer cleanup.
 * Each test is responsible for explicitly closing any drawers it opens.
 */

// =====================================================
// Model Upload
// =====================================================

export async function openAddModelDropdown(page: Page) {
  // Click the + button in the Models card header. Scope to the
  // Models card by its aria-label="Add model" so we don't collide
  // with "Add provider" on the Providers card or with the
  // "Downloading Models" card (whose title text would also match a
  // loose "Models" substring).
  const addButton = page.locator('button[aria-label="Add model"]').first()

  // Wait for button to be ready and visible
  await addButton.waitFor({ state: 'visible', timeout: 10000 })

  // Ensure button is enabled and stable before clicking
  await expect(addButton).toBeEnabled()
  await page.waitForTimeout(300) // Small delay to ensure button is fully interactive

  await addButton.click()

  // Wait for dropdown menu to appear and be stable — multiple dropdowns
  // may exist in the DOM (e.g. closed drawers leaving their menus
  // behind). Filter to the one carrying the menuitem we care about.
  await page
    .getByRole('menuitem', { name: /Upload from Files|Download from Repository|Add Remote Model/ })
    .first()
    .waitFor({ state: 'visible', timeout: 10000 })
}

export async function selectAddModelOption(page: Page, option: 'upload' | 'download' | 'remote') {
  const optionMap = {
    upload: 'Upload from Files',
    download: 'Download from Repository',
    remote: 'Add Remote Model',
  }

  // Use semantic selector; `.first()` to disambiguate when multiple
  // dropdown menus are present in DOM.
  const menuItem = page
    .getByRole('menuitem', { name: optionMap[option] })
    .first()

  await menuItem.waitFor({ state: 'visible', timeout: 10000 })
  await menuItem.click()

  // Wait for network to settle after clicking
  await page.waitForLoadState('domcontentloaded')
  await page.waitForTimeout(500) // Additional delay for drawer animation
}

export async function uploadModelFolder(page: Page, folderPath: string) {
  // For folder uploads, we need to use the directory input
  const fileInput = page.locator('input[type="file"][webkitdirectory]')
  await fileInput.setInputFiles(folderPath)

  // Wait for file processing to complete
  await page.waitForLoadState('domcontentloaded')

  // Wait for the file list to appear (indicates files were processed)
  await page.getByText('Selected Files').or(page.locator('.ant-card-head-title:has-text("Selected Files")')).waitFor({ timeout: 10000 })

  // Additional delay to ensure file classification completes
  await page.waitForTimeout(500)
}

export async function uploadModelFile(page: Page, filePath: string) {
  const fileInput = page.locator('input[type="file"]')
  await fileInput.setInputFiles(filePath)
  await page.waitForLoadState('networkidle')
}

export async function uploadModel(
  page: Page,
  data: UploadFormData
): Promise<void> {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'upload')

  // Wait for upload drawer to open. `.first()` because both branches
  // of `.or()` can match: AntD keeps closed drawers in DOM, so the
  // dialog role and the drawer-title may each resolve to 2 elements.
  await page.getByRole('dialog', { name: /upload.*model/i })
    .or(page.locator('.ant-drawer-title:has-text("Upload Local Model")'))
    .first()
    .waitFor({ timeout: 5000 })

  // Upload folder
  await uploadModelFolder(page, data.folderPath)

  // Fill form
  await fillUploadForm(page, data)

  // Submit
  await submitUploadForm(page)

  // Wait for success message
  await page.getByText('Model uploaded successfully').waitFor({ timeout: 30000 })
}

export async function waitForUploadProgress(page: Page): Promise<void> {
  // Wait for upload progress card to appear
  await page.getByText('Upload Progress').or(page.locator('.ant-card-head-title:has-text("Upload Progress")')).waitFor({ timeout: 5000 })

  // Wait for progress to complete (progress bar reaches 100% or upload completes)
  await page.locator('.ant-progress-status-success').waitFor({ timeout: 60000 })
}

export async function assertUploadProgressVisible(page: Page): Promise<void> {
  await expect(page.getByText('Upload Progress').or(page.locator('.ant-card-head-title:has-text("Upload Progress")')).first()).toBeVisible()
  await expect(page.locator('.ant-progress-line').first()).toBeVisible()
}

export async function openUploadDrawer(page: Page): Promise<void> {
  // Wait for any prior drawer's close-animation to settle so the
  // dropdown isn't intercepted and the new drawer fully opens.
  await page.locator('.ant-drawer.ant-drawer-open').first().waitFor({ state: 'hidden', timeout: 5000 }).catch(() => {})
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'upload')

  // Wait for drawer to appear. Use `.ant-drawer-open` directly so the
  // dialog is the currently-open one (not a stale closed drawer that
  // AntD leaves in DOM). `.first()` to dedupe across `.or()` branches.
  const uploadDrawer = page
    .locator('.ant-drawer.ant-drawer-open:has(.ant-drawer-title:has-text("Upload Local Model"))')
    .first()

  await uploadDrawer.waitFor({ state: 'visible', timeout: 10000 })

  // Wait for drawer to be fully loaded - check that form fields and buttons are present
  await expect(uploadDrawer.getByLabel(/display name/i).or(uploadDrawer.locator('label:has-text("Display Name")')).first()).toBeVisible()

  // The footer Upload button: while a previous upload's React state
  // is still being flushed it briefly carries `ant-btn-loading`, which
  // changes its accessible name from "Upload" to "loading Upload".
  // Wait on the structural footer-button selector instead, then wait
  // for `ant-btn-loading` to clear before asserting.
  const uploadButton = uploadDrawer
    .locator('.ant-drawer-footer button:has-text("Upload")')
    .or(uploadDrawer.getByRole('button', { name: /^(loading )?upload$/i }))
    .first()
  const cancelButton = uploadDrawer
    .locator('.ant-drawer-footer button:has-text("Cancel")')
    .first()
  await expect(uploadButton).toBeVisible()
  await expect(cancelButton).toBeVisible()
  await expect(uploadButton).not.toHaveClass(/ant-btn-loading/, { timeout: 10000 })
  await expect(uploadButton).toBeEnabled({ timeout: 5000 })
  await expect(cancelButton).toBeEnabled({ timeout: 5000 })

  // Small delay to ensure all animations are complete
  await page.waitForTimeout(300)
}

// =====================================================
// Model Download
// =====================================================

export async function startModelDownload(
  page: Page,
  data: DownloadFormData
): Promise<void> {
  await openAddModelDropdown(page)
  await selectAddModelOption(page, 'download')

  // Wait for drawer to be fully loaded. `.first()` to dedupe across
  // `.or()` branches.
  await page.getByRole('dialog', { name: /download.*repository/i })
    .or(page.locator('.ant-drawer-title:has-text("Download from Repository")'))
    .first()
    .waitFor({ timeout: 10000 })
  await page.waitForTimeout(500) // Allow drawer to fully render

  await fillDownloadForm(page, data)

  const downloadButton = page.getByRole('button', { name: /start download/i })
  await expect(downloadButton).toBeEnabled()
  await downloadButton.click()

  // Wait for success message
  await page.getByText('Download started successfully').waitFor({ timeout: 15000 })

  // Wait for drawer to close completely. The hidden state should be
  // unambiguous, but use `.first()` for consistency with the open wait.
  await page.getByRole('dialog', { name: /download.*repository/i })
    .or(page.locator('.ant-drawer-title:has-text("Download from Repository")'))
    .first()
    .waitFor({ state: 'hidden', timeout: 10000 })
}

// =====================================================
// Model Editing
// =====================================================

export async function openEditModelDrawer(page: Page, modelName: string) {
  // Use semantic selector for the edit button
  const editButton = page.getByRole('button', { name: new RegExp(`edit.*${modelName}`, 'i') })
    .or(page.locator(`text=${modelName}`).first().locator('button[aria-label="Edit"]'))

  await editButton.first().click()

  // Wait for edit drawer. `.first()` to dedupe across `.or()` branches.
  await page.getByRole('dialog', { name: /edit model/i })
    .or(page.getByText('Edit Model'))
    .first()
    .waitFor({ timeout: 30000 })
}

// =====================================================
// Model Deletion
// =====================================================

export async function deleteModel(page: Page, modelName: string): Promise<void> {
  // Find the delete button by its aria-label which includes the model display name
  // Use semantic selector first, fallback to aria-label
  const deleteButton = page.getByRole('button', { name: new RegExp(`delete.*${modelName}`, 'i') })
    .or(page.locator(`button[aria-label*="Delete"][aria-label*="${modelName}"]`))

  await deleteButton.first().click()

  // Wait for success message (no confirmation modal)
  await page.getByText('Model deleted').waitFor({ timeout: 15000 })
}

// =====================================================
// Model Assertions
// =====================================================

export async function assertModelExists(page: Page, modelName: string): Promise<void> {
  await expect(page.getByText(modelName).first()).toBeVisible()
}

export async function assertModelNotExists(page: Page, modelName: string): Promise<void> {
  await expect(page.getByText(modelName).first()).not.toBeVisible()
}
